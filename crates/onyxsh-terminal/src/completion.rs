//! Multi-source Intelligent Autocomplete Engine and Declarative Command Specs.
//! Full port of `src/onyxsh/terminal/completion/engine.py`, `models.py`, and `specs/*`
//! (`apt.py`, `dnf.py`, `pacman.py`, `flatpak.py`, `systemd.py`, `docker.py`, `git.py`,
//! `devops.py`, `common.py`, plus `rpm`, `dpkg`, `zypper`, `snap`, and `cargo`).

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionType {
    Command,
    Subcommand,
    OptionFlag,
    History,
    Snippet,
}

impl CompletionType {
    pub fn badge_label(&self) -> &'static str {
        match self {
            Self::Command => "CMD",
            Self::Subcommand => "SUB",
            Self::OptionFlag => "FLAG",
            Self::History => "HIST",
            Self::Snippet => "SNIP",
        }
    }

    pub fn icon_name(&self) -> &'static str {
        match self {
            Self::Command => "utilities-terminal-symbolic",
            Self::Subcommand => "media-playback-start-symbolic",
            Self::OptionFlag => "preferences-other-symbolic",
            Self::History => "document-open-recent-symbolic",
            Self::Snippet => "code-context-symbolic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionSource {
    Spec,
    History,
    Snippet,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompletionItem {
    pub text: String,
    pub description: String,
    pub completion_type: CompletionType,
    pub source: CompletionSource,
    pub score: f32,
    pub prefix_to_replace: String,
    pub suffix_to_insert: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionContext {
    pub full_line: String,
    pub cursor_position: usize,
    pub line_before_cursor: String,
    pub tokens: Vec<String>,
    pub current_word: String,
    pub token_index: usize,
    pub command_root: String,
    pub is_sudo: bool,
    pub cwd: String,
    pub host: String,
}

#[derive(Debug, Clone)]
pub struct OptionSpec {
    pub names: Vec<String>,
    pub description: String,
}

impl OptionSpec {
    pub fn new(names: &[&str], description: &str) -> Self {
        Self {
            names: names.iter().map(|s| (*s).to_string()).collect(),
            description: description.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubcommandSpec {
    pub name: String,
    pub description: String,
    pub aliases: Vec<String>,
    pub options: Vec<OptionSpec>,
    pub subcommands: Vec<SubcommandSpec>,
}

impl SubcommandSpec {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            aliases: Vec::new(),
            options: Vec::new(),
            subcommands: Vec::new(),
        }
    }

    pub fn with_aliases(mut self, aliases: &[&str]) -> Self {
        self.aliases = aliases.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn with_options(mut self, opts: Vec<OptionSpec>) -> Self {
        self.options = opts;
        self
    }

    pub fn with_subcommands(mut self, subs: Vec<SubcommandSpec>) -> Self {
        self.subcommands = subs;
        self
    }
}

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub name: String,
    pub description: String,
    pub aliases: Vec<String>,
    pub subcommands: Vec<SubcommandSpec>,
    pub global_options: Vec<OptionSpec>,
}

impl CommandSpec {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            aliases: Vec::new(),
            subcommands: Vec::new(),
            global_options: Vec::new(),
        }
    }

    pub fn with_aliases(mut self, aliases: &[&str]) -> Self {
        self.aliases = aliases.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn with_subcommands(mut self, subs: Vec<SubcommandSpec>) -> Self {
        self.subcommands = subs;
        self
    }

    pub fn with_options(mut self, opts: Vec<OptionSpec>) -> Self {
        self.global_options = opts;
        self
    }

    /// Generates relevant completions for this command given the parsed `CompletionContext`.
    pub fn get_completions(&self, ctx: &CompletionContext) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        let offset = if ctx.is_sudo { 2 } else { 1 };
        let sub_tokens = if ctx.tokens.len() > offset {
            &ctx.tokens[offset..]
        } else {
            &[]
        };
        let ends_space = ctx.line_before_cursor.ends_with(char::is_whitespace);
        let num_cmd_tokens = ctx.tokens.len().saturating_sub(if ctx.is_sudo { 1 } else { 0 });
        let current_word = ctx.current_word.to_lowercase();

        // Check if an active subcommand has already been completed in a previous token
        let completed_sub_tokens = if ends_space {
            sub_tokens
        } else if !sub_tokens.is_empty() {
            &sub_tokens[..sub_tokens.len() - 1]
        } else {
            &[]
        };

        let mut active_sub: Option<&SubcommandSpec> = None;
        let mut active_nested: Option<&SubcommandSpec> = None;
        for tok in completed_sub_tokens {
            let t_low = tok.to_lowercase();
            if let Some(sub) = active_sub {
                for nested in &sub.subcommands {
                    if nested.name.to_lowercase() == t_low
                        || nested.aliases.iter().any(|a| a.to_lowercase() == t_low)
                    {
                        active_nested = Some(nested);
                        break;
                    }
                }
            } else {
                for sub in &self.subcommands {
                    if sub.name.to_lowercase() == t_low
                        || sub.aliases.iter().any(|a| a.to_lowercase() == t_low)
                    {
                        active_sub = Some(sub);
                        break;
                    }
                }
            }
        }

        // Case 1: User is typing the first argument after the command (no active subcommand yet)
        if active_sub.is_none() && (num_cmd_tokens <= 1 || (num_cmd_tokens == 2 && !ends_space)) {
            for sub in &self.subcommands {
                if current_word.is_empty() || sub.name.to_lowercase().starts_with(&current_word) {
                    let score = if current_word.is_empty() {
                        2.6
                    } else if sub.name.to_lowercase() == current_word {
                        3.1
                    } else {
                        2.9
                    };
                    items.push(CompletionItem {
                        text: sub.name.clone(),
                        description: sub.description.clone(),
                        completion_type: CompletionType::Subcommand,
                        source: CompletionSource::Spec,
                        score,
                        prefix_to_replace: ctx.current_word.clone(),
                        suffix_to_insert: String::new(),
                    });
                }
            }

            for opt in &self.global_options {
                for opt_name in &opt.names {
                    if current_word.is_empty() || opt_name.to_lowercase().starts_with(&current_word)
                    {
                        let score = if current_word.starts_with('-') {
                            2.8
                        } else {
                            2.2
                        };
                        items.push(CompletionItem {
                            text: opt_name.clone(),
                            description: opt.description.clone(),
                            completion_type: CompletionType::OptionFlag,
                            source: CompletionSource::Spec,
                            score,
                            prefix_to_replace: ctx.current_word.clone(),
                            suffix_to_insert: String::new(),
                        });
                    }
                }
            }
            return items;
        }

        // Case 2: Active subcommand (or nested subcommand) is present
        let target_sub = active_nested.or(active_sub);
        if let Some(sub) = target_sub {
            // Nested subcommands (e.g., `docker compose up`, `git stash pop`)
            for nested in &sub.subcommands {
                if current_word.is_empty() || nested.name.to_lowercase().starts_with(&current_word)
                {
                    items.push(CompletionItem {
                        text: nested.name.clone(),
                        description: nested.description.clone(),
                        completion_type: CompletionType::Subcommand,
                        source: CompletionSource::Spec,
                        score: 2.9,
                        prefix_to_replace: ctx.current_word.clone(),
                        suffix_to_insert: String::new(),
                    });
                }
            }

            // Subcommand-specific options
            for opt in &sub.options {
                for opt_name in &opt.names {
                    if current_word.is_empty() || opt_name.to_lowercase().starts_with(&current_word)
                    {
                        items.push(CompletionItem {
                            text: opt_name.clone(),
                            description: opt.description.clone(),
                            completion_type: CompletionType::OptionFlag,
                            source: CompletionSource::Spec,
                            score: 2.7,
                            prefix_to_replace: ctx.current_word.clone(),
                            suffix_to_insert: String::new(),
                        });
                    }
                }
            }
        }

        // Global options as fallback
        for opt in &self.global_options {
            for opt_name in &opt.names {
                if (current_word.is_empty() && self.subcommands.is_empty())
                    || (!current_word.is_empty()
                        && opt_name.to_lowercase().starts_with(&current_word))
                {
                    items.push(CompletionItem {
                        text: opt_name.clone(),
                        description: opt.description.clone(),
                        completion_type: CompletionType::OptionFlag,
                        source: CompletionSource::Spec,
                        score: 2.1,
                        prefix_to_replace: ctx.current_word.clone(),
                        suffix_to_insert: String::new(),
                    });
                }
            }
        }

        items
    }
}

#[derive(Debug, Clone)]
pub struct SpecRegistry {
    specs: HashMap<String, CommandSpec>,
}

impl Default for SpecRegistry {
    fn default() -> Self {
        Self::new_builtin()
    }
}

impl SpecRegistry {
    pub fn new_builtin() -> Self {
        let mut specs = HashMap::new();
        let all_specs = build_all_builtin_specs();

        for spec in all_specs {
            specs.insert(spec.name.to_lowercase(), spec.clone());
            for alias in &spec.aliases {
                let mut aliased = spec.clone();
                aliased.name = alias.clone();
                specs.insert(alias.to_lowercase(), aliased);
            }
        }

        Self { specs }
    }

    pub fn get_spec(&self, name: &str) -> Option<&CommandSpec> {
        self.specs.get(&name.to_lowercase())
    }

    pub fn command_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.specs.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }
}

fn build_all_builtin_specs() -> Vec<CommandSpec> {
    let mut list = vec![
        build_apt_spec(),
        build_dpkg_spec(),
        build_dnf_spec(),
        build_rpm_spec(),
        build_pacman_spec(),
        build_flatpak_spec(),
        build_zypper_spec(),
        build_snap_spec(),
        build_systemctl_spec(),
        build_journalctl_spec(),
        build_docker_spec(),
        build_git_spec(),
        build_cargo_spec(),
    ];
    list.extend(build_devops_specs());
    list.extend(build_common_specs());
    list
}

fn build_apt_spec() -> CommandSpec {
    CommandSpec::new("apt", "Debian/Ubuntu package manager utility")
        .with_aliases(&["apt-get"])
        .with_subcommands(vec![
            SubcommandSpec::new("install", "Install one or more packages").with_options(vec![
                OptionSpec::new(&["-y", "--yes"], "Automatic yes to prompts"),
                OptionSpec::new(&["--no-install-recommends"], "Do not install recommended packages"),
                OptionSpec::new(&["--reinstall"], "Reinstall packages even if up to date"),
                OptionSpec::new(&["--dry-run", "-s"], "Simulate installation"),
            ]),
            SubcommandSpec::new("update", "Update list of available packages from repositories")
                .with_options(vec![OptionSpec::new(&["-q", "--quiet"], "Quiet mode")]),
            SubcommandSpec::new("upgrade", "Upgrade all installed packages to newest versions")
                .with_options(vec![
                    OptionSpec::new(&["-y", "--yes"], "Automatic yes to prompts"),
                    OptionSpec::new(&["--without-new-pkgs"], "Do not install new packages during upgrade"),
                ]),
            SubcommandSpec::new("full-upgrade", "Upgrade packages removing/installing dependencies if needed")
                .with_aliases(&["dist-upgrade"])
                .with_options(vec![OptionSpec::new(&["-y", "--yes"], "Automatic yes to prompts")]),
            SubcommandSpec::new("remove", "Remove one or more installed packages").with_options(vec![
                OptionSpec::new(&["--purge"], "Remove configuration files along with packages"),
                OptionSpec::new(&["-y", "--yes"], "Automatic yes to prompts"),
            ]),
            SubcommandSpec::new("purge", "Remove packages and all their configuration files")
                .with_options(vec![OptionSpec::new(&["-y", "--yes"], "Automatic yes to prompts")]),
            SubcommandSpec::new("autoremove", "Remove automatically installed dependencies no longer needed")
                .with_options(vec![
                    OptionSpec::new(&["--purge"], "Also remove configuration files of orphaned packages"),
                    OptionSpec::new(&["-y", "--yes"], "Automatic yes to prompts"),
                ]),
            SubcommandSpec::new("search", "Search package names and descriptions"),
            SubcommandSpec::new("show", "Display detailed package information"),
            SubcommandSpec::new("list", "List packages based on criteria").with_options(vec![
                OptionSpec::new(&["--installed"], "List only installed packages"),
                OptionSpec::new(&["--upgradable"], "List packages with available updates"),
                OptionSpec::new(&["--all-versions"], "List all available versions"),
            ]),
            SubcommandSpec::new("clean", "Clear out the local repository cache of retrieved package files"),
            SubcommandSpec::new("autoclean", "Erase old downloaded archive files"),
        ])
        .with_options(vec![
            OptionSpec::new(&["-h", "--help"], "Show help information"),
            OptionSpec::new(&["-v", "--version"], "Show version number"),
            OptionSpec::new(&["-y", "--yes"], "Assume yes to all queries"),
            OptionSpec::new(&["-q", "--quiet"], "Produce quiet log output"),
        ])
}

fn build_dpkg_spec() -> CommandSpec {
    CommandSpec::new("dpkg", "Debian low-level package manager (.deb)").with_subcommands(vec![
        SubcommandSpec::new("-i", "Install a local .deb package file").with_aliases(&["--install"]),
        SubcommandSpec::new("-r", "Remove an installed package keeping configuration").with_aliases(&["--remove"]),
        SubcommandSpec::new("-P", "Purge an installed package including configuration").with_aliases(&["--purge"]),
        SubcommandSpec::new("-l", "List installed packages matching pattern").with_aliases(&["--list"]),
        SubcommandSpec::new("-L", "List files installed by package").with_aliases(&["--listfiles"]),
        SubcommandSpec::new("-S", "Search for package owning a filename").with_aliases(&["--search"]),
        SubcommandSpec::new("-s", "Display status details of installed package").with_aliases(&["--status"]),
        SubcommandSpec::new("--configure", "Configure an unpacked package").with_options(vec![
            OptionSpec::new(&["-a", "--pending"], "Configure all unpacked but unconfigured packages"),
        ]),
    ])
}

fn build_dnf_spec() -> CommandSpec {
    CommandSpec::new("dnf", "Fedora/RHEL/Rocky/AlmaLinux RPM package manager")
        .with_aliases(&["yum"])
        .with_subcommands(vec![
            SubcommandSpec::new("install", "Install one or more RPM packages").with_options(vec![
                OptionSpec::new(&["-y", "--assumeyes"], "Automatic yes to all prompts"),
                OptionSpec::new(&["--allowerasing"], "Allow erasing of installed packages to resolve dependencies"),
                OptionSpec::new(&["--skip-broken"], "Skip packages with broken dependencies"),
                OptionSpec::new(&["--nobest"], "Do not limit transaction to best available package versions"),
                OptionSpec::new(&["--setopt"], "Override repository or DNF configuration options"),
            ]),
            SubcommandSpec::new("remove", "Remove one or more packages and unused dependencies")
                .with_aliases(&["erase"])
                .with_options(vec![
                    OptionSpec::new(&["-y", "--assumeyes"], "Automatic yes to all prompts"),
                    OptionSpec::new(&["--duplicates"], "Remove older duplicates of packages"),
                ]),
            SubcommandSpec::new("upgrade", "Upgrade all installed packages to newest versions")
                .with_aliases(&["update"])
                .with_options(vec![
                    OptionSpec::new(&["-y", "--assumeyes"], "Automatic yes to all prompts"),
                    OptionSpec::new(&["--security"], "Include only packages providing security fixes"),
                    OptionSpec::new(&["--minimal"], "Upgrade only to the lowest version containing fixes"),
                    OptionSpec::new(&["--refresh"], "Force metadata refresh before upgrade"),
                    OptionSpec::new(&["--allowerasing"], "Allow erasing of installed packages to resolve dependencies"),
                ]),
            SubcommandSpec::new("check-update", "Check for available package upgrades without installing")
                .with_options(vec![
                    OptionSpec::new(&["--security"], "Check only for security updates"),
                    OptionSpec::new(&["--refresh"], "Force metadata refresh before checking"),
                ]),
            SubcommandSpec::new("search", "Search package names and summaries for keywords")
                .with_options(vec![OptionSpec::new(&["--all"], "Search descriptions as well as names")]),
            SubcommandSpec::new("info", "Display detailed package information and metadata")
                .with_options(vec![
                    OptionSpec::new(&["--installed"], "Show information for installed packages only"),
                    OptionSpec::new(&["--available"], "Show information for available packages only"),
                ]),
            SubcommandSpec::new("autoremove", "Remove packages originally installed as dependencies no longer needed")
                .with_options(vec![OptionSpec::new(&["-y", "--assumeyes"], "Automatic yes to all prompts")]),
            SubcommandSpec::new("clean", "Remove cached metadata and downloaded packages").with_options(vec![
                OptionSpec::new(&["all"], "Clean all cached metadata and packages"),
                OptionSpec::new(&["metadata"], "Clean cached repository metadata"),
                OptionSpec::new(&["packages"], "Clean cached RPM packages"),
                OptionSpec::new(&["dbcache"], "Clean cached metadata databases"),
            ]),
            SubcommandSpec::new("reinstall", "Reinstall one or more packages")
                .with_options(vec![OptionSpec::new(&["-y", "--assumeyes"], "Automatic yes to all prompts")]),
            SubcommandSpec::new("download", "Download package RPMs without installing them").with_options(vec![
                OptionSpec::new(&["--destdir"], "Directory to download packages into"),
                OptionSpec::new(&["--resolve"], "Download needed dependencies as well"),
                OptionSpec::new(&["--source"], "Download source RPM (SRPM)"),
            ]),
            SubcommandSpec::new("groupinstall", "Install a software group by name")
                .with_options(vec![
                    OptionSpec::new(&["-y", "--assumeyes"], "Automatic yes to all prompts"),
                    OptionSpec::new(&["--with-optional"], "Include optional packages in the group"),
                ]),
            SubcommandSpec::new("history", "View and manage DNF transaction history").with_options(vec![
                OptionSpec::new(&["list"], "List recent transactions"),
                OptionSpec::new(&["info"], "Show details of a specific transaction"),
                OptionSpec::new(&["undo"], "Undo a specific transaction"),
                OptionSpec::new(&["redo"], "Redo a specific transaction"),
                OptionSpec::new(&["rollback"], "Roll back to a specific transaction state"),
            ]),
            SubcommandSpec::new("repoquery", "Query packages matching repository criteria").with_options(vec![
                OptionSpec::new(&["--requires"], "List packages required by the specified package"),
                OptionSpec::new(&["--whatprovides"], "Query packages providing the specified capability"),
                OptionSpec::new(&["--whatrequires"], "Query packages requiring the specified capability"),
                OptionSpec::new(&["--installed"], "Query installed packages only"),
            ]),
            SubcommandSpec::new("provides", "Find which package provides a given file or feature")
                .with_aliases(&["whatprovides"]),
        ])
        .with_options(vec![
            OptionSpec::new(&["-h", "--help"], "Show help information"),
            OptionSpec::new(&["--version"], "Show version number"),
            OptionSpec::new(&["-y", "--assumeyes"], "Assume yes to all queries"),
            OptionSpec::new(&["-q", "--quiet"], "Produce quiet output"),
            OptionSpec::new(&["-v", "--verbose"], "Produce verbose output"),
            OptionSpec::new(&["--refresh"], "Set metadata as expired before running the command"),
            OptionSpec::new(&["--enablerepo"], "Enable additional repository"),
            OptionSpec::new(&["--disablerepo"], "Disable specified repository"),
        ])
}

fn build_rpm_spec() -> CommandSpec {
    CommandSpec::new("rpm", "RPM Package Manager low-level tool (Fedora/RHEL/SUSE)")
        .with_subcommands(vec![
            SubcommandSpec::new("-ivh", "Install an RPM package file with verbose progress")
                .with_aliases(&["-i", "--install"])
                .with_options(vec![
                    OptionSpec::new(&["--nodeps"], "Do not verify package dependencies before installing"),
                    OptionSpec::new(&["--force"], "Force installation even if package/files already exist"),
                    OptionSpec::new(&["--test"], "Dry-run test installation without modifying system"),
                    OptionSpec::new(&["--replacepkgs"], "Reinstall if the package is already present"),
                ]),
            SubcommandSpec::new("-Uvh", "Upgrade an RPM package file (or install if not present)")
                .with_aliases(&["-U", "--upgrade"])
                .with_options(vec![
                    OptionSpec::new(&["--nodeps"], "Do not verify package dependencies"),
                    OptionSpec::new(&["--force"], "Force upgrade"),
                    OptionSpec::new(&["--oldpackage"], "Allow downgrading to an older package version"),
                ]),
            SubcommandSpec::new("-Fvh", "Freshen RPM packages (upgrade only if already installed)")
                .with_aliases(&["-F", "--freshen"]),
            SubcommandSpec::new("-e", "Erase (uninstall) an installed RPM package")
                .with_aliases(&["--erase"])
                .with_options(vec![
                    OptionSpec::new(&["--nodeps"], "Do not check dependencies before erasing"),
                    OptionSpec::new(&["--test"], "Dry-run removal without deleting files"),
                    OptionSpec::new(&["--allmatches"], "Remove all versions of the package"),
                ]),
            SubcommandSpec::new("-qa", "Query and list all installed RPM packages").with_options(vec![
                OptionSpec::new(&["--last"], "Order installed packages by installation time (newest first)"),
                OptionSpec::new(&["--qf"], "Format query output with custom queryformat"),
            ]),
            SubcommandSpec::new("-qi", "Display detailed information about an installed package"),
            SubcommandSpec::new("-ql", "List all files owned by an installed RPM package")
                .with_options(vec![OptionSpec::new(&["-c"], "List configuration files only")]),
            SubcommandSpec::new("-qf", "Query which installed RPM package owns a file path"),
            SubcommandSpec::new("-qp", "Query an uninstalled local .rpm package file").with_options(vec![
                OptionSpec::new(&["-i"], "Show package metadata info"),
                OptionSpec::new(&["-l"], "List files contained inside the .rpm archive"),
                OptionSpec::new(&["-R", "--requires"], "List capabilities required by this .rpm"),
                OptionSpec::new(&["--scripts"], "Show pre/post install & uninstall scripts"),
            ]),
            SubcommandSpec::new("-qR", "List dependencies required by an installed package"),
            SubcommandSpec::new("-qc", "List configuration files of an installed package"),
            SubcommandSpec::new("-Va", "Verify integrity of all installed RPM packages against database"),
            SubcommandSpec::new("-Vp", "Verify a specific package against an .rpm file"),
            SubcommandSpec::new("-K", "Check GPG signature and integrity of an .rpm file")
                .with_aliases(&["--checksig"]),
            SubcommandSpec::new("--import", "Import an ASCII-armored GPG public key into RPM keyring"),
            SubcommandSpec::new("--rebuilddb", "Rebuild the RPM database indices from installed headers"),
        ])
        .with_options(vec![
            OptionSpec::new(&["--version"], "Print RPM version"),
            OptionSpec::new(&["-v", "--verbose"], "Provide verbose output"),
            OptionSpec::new(&["--root"], "Use directory as top level root for RPM operations"),
        ])
}

fn build_pacman_spec() -> CommandSpec {
    CommandSpec::new("pacman", "Arch Linux / Manjaro package manager utility")
        .with_subcommands(vec![
            SubcommandSpec::new("-S", "Install or synchronize packages from remote repositories")
                .with_aliases(&["--sync"])
                .with_options(vec![
                    OptionSpec::new(&["-y", "--refresh"], "Download fresh copy of package databases"),
                    OptionSpec::new(&["-u", "--sysupgrade"], "Upgrade all packages that are out of date"),
                    OptionSpec::new(&["-w", "--downloadonly"], "Download packages only, do not install"),
                    OptionSpec::new(&["-c", "--clean"], "Remove old packages from cache directory"),
                    OptionSpec::new(&["-s", "--search"], "Search remote repositories for matching regex"),
                    OptionSpec::new(&["-i", "--info"], "View information about a remote package"),
                    OptionSpec::new(&["--needed"], "Do not reinstall up-to-date packages"),
                    OptionSpec::new(&["--asdeps"], "Install packages as non-explicit dependencies"),
                    OptionSpec::new(&["--asexplicit"], "Install packages as explicitly requested"),
                    OptionSpec::new(&["--noconfirm"], "Bypass any and all confirmation prompts"),
                ]),
            SubcommandSpec::new("-Syu", "Full system upgrade (refresh repositories and update all packages)")
                .with_options(vec![
                    OptionSpec::new(&["--noconfirm"], "Bypass any and all confirmation prompts"),
                    OptionSpec::new(&["--needed"], "Do not reinstall up-to-date packages"),
                    OptionSpec::new(&["--ignore"], "Ignore a package upgrade"),
                ]),
            SubcommandSpec::new("-Syyu", "Force refresh all package databases and full system upgrade")
                .with_options(vec![OptionSpec::new(&["--noconfirm"], "Bypass confirmation prompts")]),
            SubcommandSpec::new("-R", "Remove one or more packages from the system")
                .with_aliases(&["--remove"])
                .with_options(vec![
                    OptionSpec::new(&["-s", "--recursive"], "Remove dependencies not required by other packages"),
                    OptionSpec::new(&["-n", "--nosave"], "Ignore file backup designations (remove config files)"),
                    OptionSpec::new(&["-c", "--cascade"], "Remove all packages that depend on target"),
                    OptionSpec::new(&["-u", "--unneeded"], "Remove unneeded packages"),
                    OptionSpec::new(&["--noconfirm"], "Bypass confirmation prompts"),
                ]),
            SubcommandSpec::new("-Rns", "Cleanly remove package with all unused dependencies and config files")
                .with_options(vec![OptionSpec::new(&["--noconfirm"], "Bypass confirmation prompts")]),
            SubcommandSpec::new("-Ss", "Search remote repositories for packages matching keyword"),
            SubcommandSpec::new("-Si", "Display detailed information about a remote package"),
            SubcommandSpec::new("-Q", "Query the local package database")
                .with_aliases(&["--query"])
                .with_options(vec![
                    OptionSpec::new(&["-e", "--explicit"], "List packages that were explicitly installed"),
                    OptionSpec::new(&["-d", "--deps"], "List packages installed as dependencies"),
                    OptionSpec::new(&["-t", "--unrequired"], "List packages not required by any installed package"),
                    OptionSpec::new(&["-m", "--foreign"], "List foreign packages (e.g. from AUR)"),
                    OptionSpec::new(&["-s", "--search"], "Search locally-installed packages"),
                    OptionSpec::new(&["-i", "--info"], "View information about a locally-installed package"),
                    OptionSpec::new(&["-l", "--list"], "List all files owned by a given package"),
                    OptionSpec::new(&["-o", "--owns"], "Search for package that owns the specified file"),
                    OptionSpec::new(&["-q", "--quiet"], "Show less information for query output"),
                ]),
            SubcommandSpec::new("-Qdt", "List orphaned packages (dependencies no longer required)")
                .with_options(vec![OptionSpec::new(&["-q", "--quiet"], "Output package names only")]),
            SubcommandSpec::new("-Qi", "Display detailed information about a locally installed package"),
            SubcommandSpec::new("-Ql", "List all files owned by a locally installed package"),
            SubcommandSpec::new("-Qo", "Query which package owns a specific file on the filesystem"),
            SubcommandSpec::new("-Sc", "Clean package cache (remove old downloaded packages)")
                .with_options(vec![OptionSpec::new(&["--noconfirm"], "Bypass confirmation prompt")]),
            SubcommandSpec::new("-Scc", "Clean all package cache (remove all downloaded packages)")
                .with_options(vec![OptionSpec::new(&["--noconfirm"], "Bypass confirmation prompt")]),
            SubcommandSpec::new("-U", "Upgrade or install local/remote package file (.pkg.tar.zst)")
                .with_aliases(&["--upgrade"])
                .with_options(vec![
                    OptionSpec::new(&["--needed"], "Do not reinstall up-to-date packages"),
                    OptionSpec::new(&["--noconfirm"], "Bypass confirmation prompts"),
                    OptionSpec::new(&["--asdeps"], "Install as non-explicit dependency"),
                ]),
            SubcommandSpec::new("-Fy", "Refresh files database"),
            SubcommandSpec::new("-Fs", "Search files database for a filename"),
        ])
        .with_options(vec![
            OptionSpec::new(&["-h", "--help"], "Display syntax and options"),
            OptionSpec::new(&["-V", "--version"], "Display version information"),
            OptionSpec::new(&["-v", "--verbose"], "Output more detailed diagnostic information"),
            OptionSpec::new(&["--noconfirm"], "Bypass any and all confirmation questions"),
            OptionSpec::new(&["--needed"], "Do not reinstall up-to-date targets"),
        ])
}

fn build_flatpak_spec() -> CommandSpec {
    CommandSpec::new("flatpak", "Application sandboxing and package distribution utility")
        .with_subcommands(vec![
            SubcommandSpec::new("install", "Install an application or runtime from a remote or bundle")
                .with_options(vec![
                    OptionSpec::new(&["-y", "--assumeyes"], "Automatically answer yes to all questions"),
                    OptionSpec::new(&["--noninteractive"], "Produce minimal output and never ask questions"),
                    OptionSpec::new(&["--user"], "Install in user storage"),
                    OptionSpec::new(&["--system"], "Install in system-wide storage"),
                    OptionSpec::new(&["--reinstall"], "Reinstall even if already installed"),
                    OptionSpec::new(&["--from"], "Install from a .flatpakref file or URL"),
                    OptionSpec::new(&["--bundle"], "Install from a .flatpak bundle file"),
                    OptionSpec::new(&["--or-update"], "Update the app/runtime if already installed"),
                ]),
            SubcommandSpec::new("uninstall", "Uninstall an installed application or runtime")
                .with_aliases(&["remove"])
                .with_options(vec![
                    OptionSpec::new(&["-y", "--assumeyes"], "Automatically answer yes to all questions"),
                    OptionSpec::new(&["--noninteractive"], "Never ask questions"),
                    OptionSpec::new(&["--user"], "Uninstall from user storage"),
                    OptionSpec::new(&["--system"], "Uninstall from system-wide storage"),
                    OptionSpec::new(&["--unused"], "Remove unused runtimes and extensions"),
                    OptionSpec::new(&["--delete-data"], "Delete application data in ~/.var/app"),
                    OptionSpec::new(&["--all"], "Remove all matching applications/runtimes"),
                ]),
            SubcommandSpec::new("update", "Update installed applications and runtimes").with_options(vec![
                OptionSpec::new(&["-y", "--assumeyes"], "Automatically answer yes to all questions"),
                OptionSpec::new(&["--noninteractive"], "Never ask questions"),
                OptionSpec::new(&["--user"], "Update only user installations"),
                OptionSpec::new(&["--system"], "Update only system-wide installations"),
                OptionSpec::new(&["--app"], "Update only applications"),
                OptionSpec::new(&["--runtime"], "Update only runtimes"),
            ]),
            SubcommandSpec::new("list", "List installed applications and/or runtimes").with_options(vec![
                OptionSpec::new(&["--app"], "List only applications"),
                OptionSpec::new(&["--runtime"], "List only runtimes"),
                OptionSpec::new(&["--all", "-a"], "List all installed apps and runtimes"),
                OptionSpec::new(&["-d", "--show-details"], "Show full details including commit and runtime"),
                OptionSpec::new(&["--columns"], "Specify columns to display"),
                OptionSpec::new(&["--user"], "List only user-installed applications"),
                OptionSpec::new(&["--system"], "List only system-installed applications"),
            ]),
            SubcommandSpec::new("search", "Search for applications and runtimes in remote repositories")
                .with_options(vec![
                    OptionSpec::new(&["--user"], "Search user repositories"),
                    OptionSpec::new(&["--system"], "Search system-wide repositories"),
                    OptionSpec::new(&["--columns"], "Specify columns to display"),
                ]),
            SubcommandSpec::new("info", "Show detailed information about an installed app or runtime")
                .with_options(vec![
                    OptionSpec::new(&["-m", "--show-metadata"], "Show application metadata"),
                    OptionSpec::new(&["--show-permissions"], "Show sandbox permissions"),
                    OptionSpec::new(&["--show-extensions"], "Show configured extensions"),
                    OptionSpec::new(&["--file-access"], "Show permissions for a specific file path"),
                    OptionSpec::new(&["--user"], "Query user installation"),
                    OptionSpec::new(&["--system"], "Query system-wide installation"),
                ]),
            SubcommandSpec::new("run", "Run an application in an isolated sandbox").with_options(vec![
                OptionSpec::new(&["--command"], "Execute alternative command instead of default binary"),
                OptionSpec::new(&["--branch"], "Select specific branch"),
                OptionSpec::new(&["--devel"], "Use development SDK instead of runtime"),
                OptionSpec::new(&["--file-forwarding"], "Enable file forwarding for arguments"),
                OptionSpec::new(&["-v", "--verbose"], "Print debug logging while launching"),
            ]),
            SubcommandSpec::new("remotes", "List configured remote repositories").with_options(vec![
                OptionSpec::new(&["-d", "--show-details"], "Show full details for remote repositories"),
                OptionSpec::new(&["--show-disabled"], "Show disabled remote repositories as well"),
                OptionSpec::new(&["--user"], "List user repositories"),
                OptionSpec::new(&["--system"], "List system-wide repositories"),
            ]),
            SubcommandSpec::new("remote-add", "Add a remote repository").with_options(vec![
                OptionSpec::new(&["--if-not-exists"], "Do nothing if remote with same name exists"),
                OptionSpec::new(&["--from"], "Import remote configuration from .flatpakrepo"),
                OptionSpec::new(&["--user"], "Add remote to user storage"),
                OptionSpec::new(&["--system"], "Add remote to system-wide storage"),
                OptionSpec::new(&["--no-gpg-verify"], "Disable GPG verification"),
            ]),
            SubcommandSpec::new("remote-delete", "Delete a remote repository").with_options(vec![
                OptionSpec::new(&["--force"], "Force deletion even if apps depend on it"),
                OptionSpec::new(&["--user"], "Delete from user storage"),
                OptionSpec::new(&["--system"], "Delete from system-wide storage"),
            ]),
            SubcommandSpec::new("remote-modify", "Modify properties of a remote repository")
                .with_options(vec![
                    OptionSpec::new(&["--enable"], "Enable the remote repository"),
                    OptionSpec::new(&["--disable"], "Disable the remote repository"),
                    OptionSpec::new(&["--url"], "Set new repository URL"),
                ]),
            SubcommandSpec::new("remote-ls", "List applications and runtimes available in a remote")
                .with_options(vec![
                    OptionSpec::new(&["--app"], "List only applications"),
                    OptionSpec::new(&["--runtime"], "List only runtimes"),
                ]),
            SubcommandSpec::new("ps", "List running Flatpak application instances"),
            SubcommandSpec::new("kill", "Stop a running Flatpak application instance"),
            SubcommandSpec::new("override", "Manage sandbox application permissions and overrides")
                .with_options(vec![
                    OptionSpec::new(&["--filesystem"], "Grant filesystem access permission"),
                    OptionSpec::new(&["--nofilesystem"], "Revoke filesystem access permission"),
                    OptionSpec::new(&["--device"], "Grant device access permission"),
                    OptionSpec::new(&["--share"], "Share system subsystem (network, ipc)"),
                    OptionSpec::new(&["--unshare"], "Unshare system subsystem"),
                    OptionSpec::new(&["--socket"], "Grant access to well-known socket"),
                    OptionSpec::new(&["--nosocket"], "Revoke access to well-known socket"),
                    OptionSpec::new(&["--show"], "Show current overrides"),
                    OptionSpec::new(&["--reset"], "Reset all overrides for the application"),
                    OptionSpec::new(&["--user"], "Apply override for current user"),
                    OptionSpec::new(&["--system"], "Apply override system-wide"),
                ]),
            SubcommandSpec::new("permission-show", "Show permissions granted to an application"),
            SubcommandSpec::new("permission-reset", "Reset all permissions granted to an application"),
            SubcommandSpec::new("repair", "Repair flatpak installations and ostree repositories")
                .with_options(vec![
                    OptionSpec::new(&["--dry-run"], "Only print issues without modifying repository"),
                    OptionSpec::new(&["--user"], "Repair user installation"),
                    OptionSpec::new(&["--system"], "Repair system installation"),
                ]),
            SubcommandSpec::new("history", "Show transaction history of installations and changes"),
            SubcommandSpec::new("config", "Manage Flatpak configuration settings").with_options(vec![
                OptionSpec::new(&["--list"], "List all configuration keys and values"),
                OptionSpec::new(&["--set"], "Set configuration key"),
                OptionSpec::new(&["--unset"], "Unset configuration key"),
            ]),
        ])
        .with_options(vec![
            OptionSpec::new(&["-h", "--help"], "Show help information"),
            OptionSpec::new(&["--version"], "Show version number"),
            OptionSpec::new(&["-v", "--verbose"], "Print debug information"),
            OptionSpec::new(&["-y", "--assumeyes"], "Automatically answer yes to all questions"),
            OptionSpec::new(&["--user"], "Operate on user installation"),
            OptionSpec::new(&["--system"], "Operate on system-wide installation"),
        ])
}

fn build_zypper_spec() -> CommandSpec {
    CommandSpec::new("zypper", "openSUSE / SLES package manager utility")
        .with_subcommands(vec![
            SubcommandSpec::new("install", "Install one or more packages")
                .with_aliases(&["in"])
                .with_options(vec![
                    OptionSpec::new(&["-y", "--non-interactive"], "Do not ask anything, use default answers"),
                    OptionSpec::new(&["--no-recommends"], "Do not install recommended packages"),
                ]),
            SubcommandSpec::new("remove", "Remove installed packages")
                .with_aliases(&["rm"])
                .with_options(vec![
                    OptionSpec::new(&["-y", "--non-interactive"], "Skip confirmation prompt"),
                    OptionSpec::new(&["-u", "--clean-deps"], "Automatically remove unneeded dependencies"),
                ]),
            SubcommandSpec::new("update", "Update installed packages with newer versions")
                .with_aliases(&["up"])
                .with_options(vec![OptionSpec::new(&["-y", "--non-interactive"], "Automatic yes")]),
            SubcommandSpec::new("dup", "Perform a distribution upgrade (Tumbleweed/Leap)")
                .with_aliases(&["dist-upgrade"])
                .with_options(vec![OptionSpec::new(&["-y", "--non-interactive"], "Automatic yes")]),
            SubcommandSpec::new("search", "Search for packages matching pattern").with_aliases(&["se"]),
            SubcommandSpec::new("info", "Show full information for specified packages").with_aliases(&["if"]),
            SubcommandSpec::new("refresh", "Refresh all repository metadata").with_aliases(&["ref"]),
            SubcommandSpec::new("clean", "Clean local caches"),
        ])
        .with_options(vec![
            OptionSpec::new(&["-n", "--non-interactive"], "Non-interactive mode"),
            OptionSpec::new(&["--version"], "Print zypper version"),
        ])
}

fn build_snap_spec() -> CommandSpec {
    CommandSpec::new("snap", "Ubuntu / Canonical Snap package manager")
        .with_subcommands(vec![
            SubcommandSpec::new("install", "Install snaps on the system").with_options(vec![
                OptionSpec::new(&["--classic"], "Put snap in classic confinement mode"),
                OptionSpec::new(&["--edge"], "Install from the edge channel"),
                OptionSpec::new(&["--beta"], "Install from the beta channel"),
                OptionSpec::new(&["--stable"], "Install from the stable channel"),
            ]),
            SubcommandSpec::new("remove", "Remove snaps from the system").with_options(vec![
                OptionSpec::new(&["--purge"], "Do not save a snapshot of the snap's data"),
            ]),
            SubcommandSpec::new("refresh", "Update snaps in the system")
                .with_options(vec![OptionSpec::new(&["--list"], "Show available snap updates without installing")]),
            SubcommandSpec::new("list", "List installed snaps").with_options(vec![
                OptionSpec::new(&["--all"], "Show all revisions of installed snaps"),
            ]),
            SubcommandSpec::new("find", "Search for snaps in the store"),
            SubcommandSpec::new("info", "Show detailed information about a snap"),
            SubcommandSpec::new("services", "Query the status of services provided by snaps"),
            SubcommandSpec::new("logs", "Retrieve logs for services").with_options(vec![
                OptionSpec::new(&["-f"], "Follow log output"),
            ]),
            SubcommandSpec::new("enable", "Enable a disabled snap"),
            SubcommandSpec::new("disable", "Disable an installed snap"),
        ])
}

fn build_systemctl_spec() -> CommandSpec {
    CommandSpec::new("systemctl", "Control the systemd system and service manager")
        .with_subcommands(vec![
            SubcommandSpec::new("status", "Show runtime status information about units/services")
                .with_options(vec![
                    OptionSpec::new(&["-l", "--full"], "Do not ellipsize output"),
                    OptionSpec::new(&["--no-pager"], "Do not pipe output into a pager"),
                ]),
            SubcommandSpec::new("start", "Start (activate) one or more units"),
            SubcommandSpec::new("stop", "Stop (deactivate) one or more units"),
            SubcommandSpec::new("restart", "Stop and then start one or more units"),
            SubcommandSpec::new("reload", "Reload configuration without dropping connections"),
            SubcommandSpec::new("reload-or-restart", "Reload service or restart if reload is unsupported"),
            SubcommandSpec::new("enable", "Enable one or more units to start automatically at boot")
                .with_options(vec![OptionSpec::new(&["--now"], "Enable and start unit immediately")]),
            SubcommandSpec::new("disable", "Disable one or more units from starting at boot")
                .with_options(vec![OptionSpec::new(&["--now"], "Disable and stop unit immediately")]),
            SubcommandSpec::new("is-active", "Check whether units are active (running)"),
            SubcommandSpec::new("is-enabled", "Check whether units are enabled to start on boot"),
            SubcommandSpec::new("daemon-reload", "Reload systemd manager configuration and unit files"),
            SubcommandSpec::new("list-units", "List currently active units in memory").with_options(vec![
                OptionSpec::new(&["--type"], "Filter by unit type (service, socket, timer)"),
                OptionSpec::new(&["--state"], "Filter by unit state (active, failed, inactive)"),
            ]),
            SubcommandSpec::new("list-unit-files", "List installed unit files and their enablement status"),
            SubcommandSpec::new("mask", "Mask one or more units completely preventing activation"),
            SubcommandSpec::new("unmask", "Unmask units allowing activation again"),
        ])
        .with_options(vec![
            OptionSpec::new(&["--user"], "Talk to the service manager of the calling user"),
            OptionSpec::new(&["--system"], "Talk to the system service manager (default)"),
            OptionSpec::new(&["--no-pager"], "Do not pipe output into a pager"),
            OptionSpec::new(&["-h", "--help"], "Show help information"),
        ])
}

fn build_journalctl_spec() -> CommandSpec {
    CommandSpec::new("journalctl", "Query the systemd journal logs").with_options(vec![
        OptionSpec::new(&["-u", "--unit"], "Show logs for the specified service unit"),
        OptionSpec::new(&["-f", "--follow"], "Follow journal output live (real-time stream)"),
        OptionSpec::new(&["-n", "--lines"], "Number of journal entries to show"),
        OptionSpec::new(&["-b", "--boot"], "Show messages from the current or specified boot"),
        OptionSpec::new(&["-e", "--pager-end"], "Jump to the end of the journal immediately"),
        OptionSpec::new(&["-k", "--dmesg"], "Show kernel messages only (like dmesg)"),
        OptionSpec::new(&["--since"], "Filter entries since date/time or relative timestamp"),
        OptionSpec::new(&["--until"], "Filter entries until date/time or relative timestamp"),
        OptionSpec::new(&["-p", "--priority"], "Filter by message priority (err, warning, info, debug)"),
        OptionSpec::new(&["-r", "--reverse"], "Show newest entries first"),
        OptionSpec::new(&["--no-pager"], "Do not pipe output into a pager"),
        OptionSpec::new(&["-x", "--catalog"], "Show explanatory help texts from message catalog"),
    ])
}

fn build_docker_spec() -> CommandSpec {
    CommandSpec::new("docker", "Container runtime and image management CLI")
        .with_aliases(&["podman"])
        .with_subcommands(vec![
            SubcommandSpec::new("ps", "List active containers").with_options(vec![
                OptionSpec::new(&["-a", "--all"], "Show all containers (default shows just running)"),
                OptionSpec::new(&["-q", "--quiet"], "Only display numeric container IDs"),
                OptionSpec::new(&["--filter"], "Filter output based on conditions provided"),
            ]),
            SubcommandSpec::new("run", "Create and run a new container from an image").with_options(vec![
                OptionSpec::new(&["-d", "--detach"], "Run container in background and print container ID"),
                OptionSpec::new(&["-it"], "Allocate a pseudo-TTY connected to container stdin"),
                OptionSpec::new(&["--rm"], "Automatically remove the container when it exits"),
                OptionSpec::new(&["-p", "--publish"], "Publish container port(s) to the host"),
                OptionSpec::new(&["-v", "--volume"], "Bind mount a volume"),
                OptionSpec::new(&["--name"], "Assign a custom name to the container"),
                OptionSpec::new(&["--restart"], "Restart policy when container exits"),
                OptionSpec::new(&["-e", "--env"], "Set environment variables"),
                OptionSpec::new(&["--network"], "Connect a container to a network"),
            ]),
            SubcommandSpec::new("exec", "Execute a command inside a running container").with_options(vec![
                OptionSpec::new(&["-it"], "Allocate a pseudo-TTY and keep stdin open"),
                OptionSpec::new(&["-d", "--detach"], "Detached mode: run command in background"),
                OptionSpec::new(&["-u", "--user"], "Username or UID"),
            ]),
            SubcommandSpec::new("logs", "Fetch the logs of a container").with_options(vec![
                OptionSpec::new(&["-f", "--follow"], "Follow log output in real time"),
                OptionSpec::new(&["--tail"], "Number of lines to show from the end of the logs"),
                OptionSpec::new(&["-t", "--timestamps"], "Show timestamps"),
            ]),
            SubcommandSpec::new("images", "List local container images").with_options(vec![
                OptionSpec::new(&["-a", "--all"], "Show all images"),
                OptionSpec::new(&["-q", "--quiet"], "Only show numeric image IDs"),
            ]),
            SubcommandSpec::new("pull", "Download an image from a container registry"),
            SubcommandSpec::new("push", "Upload an image to a container registry"),
            SubcommandSpec::new("build", "Build an image from a Dockerfile").with_options(vec![
                OptionSpec::new(&["-t", "--tag"], "Name and optionally a tag in 'name:tag' format"),
                OptionSpec::new(&["-f", "--file"], "Name of the Dockerfile"),
                OptionSpec::new(&["--no-cache"], "Do not use cache when building the image"),
            ]),
            SubcommandSpec::new("stop", "Stop one or more running containers"),
            SubcommandSpec::new("start", "Start one or more stopped containers"),
            SubcommandSpec::new("restart", "Restart one or more containers"),
            SubcommandSpec::new("rm", "Remove one or more stopped containers").with_options(vec![
                OptionSpec::new(&["-f", "--force"], "Force the removal of a running container"),
                OptionSpec::new(&["-v", "--volumes"], "Remove anonymous volumes associated with container"),
            ]),
            SubcommandSpec::new("rmi", "Remove one or more container images").with_options(vec![
                OptionSpec::new(&["-f", "--force"], "Force removal of the image"),
            ]),
            SubcommandSpec::new("compose", "Define and run multi-container applications").with_subcommands(vec![
                SubcommandSpec::new("up", "Create and start containers").with_options(vec![
                    OptionSpec::new(&["-d", "--detach"], "Detached mode: run containers in background"),
                    OptionSpec::new(&["--build"], "Build images before starting containers"),
                ]),
                SubcommandSpec::new("down", "Stop and remove containers, networks, and volumes").with_options(vec![
                    OptionSpec::new(&["-v", "--volumes"], "Remove named volumes"),
                ]),
                SubcommandSpec::new("logs", "View output from containers").with_options(vec![
                    OptionSpec::new(&["-f", "--follow"], "Follow log output"),
                ]),
                SubcommandSpec::new("restart", "Restart compose services"),
                SubcommandSpec::new("ps", "List compose containers"),
            ]),
            SubcommandSpec::new("system", "Manage Docker system resources").with_subcommands(vec![
                SubcommandSpec::new("prune", "Remove unused data (containers, networks, images)").with_options(vec![
                    OptionSpec::new(&["-a", "--all"], "Remove all unused images, not just dangling ones"),
                    OptionSpec::new(&["--volumes"], "Prune anonymous volumes as well"),
                ]),
                SubcommandSpec::new("df", "Show docker disk usage"),
            ]),
        ])
        .with_options(vec![
            OptionSpec::new(&["-h", "--help"], "Show help information"),
            OptionSpec::new(&["-v", "--version"], "Print version information"),
        ])
}

fn build_git_spec() -> CommandSpec {
    CommandSpec::new("git", "Distributed version control system")
        .with_subcommands(vec![
            SubcommandSpec::new("status", "Show the working tree status").with_options(vec![
                OptionSpec::new(&["-s", "--short"], "Give the output in the short-format"),
                OptionSpec::new(&["-b", "--branch"], "Show the branch and tracking info"),
            ]),
            SubcommandSpec::new("add", "Add file contents to the staging index").with_options(vec![
                OptionSpec::new(&["-A", "--all"], "Stage all changes (new, modified, deleted)"),
                OptionSpec::new(&["-p", "--patch"], "Interactively choose hunks of patch"),
                OptionSpec::new(&["-u", "--update"], "Update tracked files only"),
                OptionSpec::new(&["."], "Add all files in current directory"),
            ]),
            SubcommandSpec::new("commit", "Record staged changes to the repository").with_options(vec![
                OptionSpec::new(&["-m", "--message"], "Use the given commit message"),
                OptionSpec::new(&["-a", "--all"], "Automatically stage modified and deleted files"),
                OptionSpec::new(&["--amend"], "Amend the tip of the current branch"),
                OptionSpec::new(&["--no-verify"], "Bypass pre-commit and commit-msg hooks"),
            ]),
            SubcommandSpec::new("push", "Update remote refs along with associated objects").with_options(vec![
                OptionSpec::new(&["-u", "--set-upstream"], "Add upstream tracking reference"),
                OptionSpec::new(&["-f", "--force"], "Force updates on the remote repository"),
                OptionSpec::new(&["--force-with-lease"], "Safe force push verifying remote ref"),
                OptionSpec::new(&["--tags"], "Push all tags"),
            ]),
            SubcommandSpec::new("pull", "Fetch from and integrate with another repository").with_options(vec![
                OptionSpec::new(&["--rebase"], "Rebase current branch on top of upstream after fetching"),
                OptionSpec::new(&["--autostash"], "Automatically stash before rebase and pop after"),
            ]),
            SubcommandSpec::new("fetch", "Download objects and refs from another repository").with_options(vec![
                OptionSpec::new(&["-p", "--prune"], "Remove obsolete remote-tracking references"),
                OptionSpec::new(&["--all"], "Fetch all remotes"),
            ]),
            SubcommandSpec::new("checkout", "Switch branches or restore working tree files").with_options(vec![
                OptionSpec::new(&["-b"], "Create and switch to a new branch"),
                OptionSpec::new(&["-B"], "Create/reset and checkout a branch"),
            ]),
            SubcommandSpec::new("switch", "Switch branches").with_options(vec![
                OptionSpec::new(&["-c", "--create"], "Create and switch to a new branch"),
            ]),
            SubcommandSpec::new("branch", "List, create, or delete branches").with_options(vec![
                OptionSpec::new(&["-a", "--all"], "List both remote-tracking and local branches"),
                OptionSpec::new(&["-d", "--delete"], "Delete a merged branch"),
                OptionSpec::new(&["-D"], "Force delete a branch"),
                OptionSpec::new(&["-m", "--move"], "Rename a branch"),
            ]),
            SubcommandSpec::new("log", "Show commit logs").with_options(vec![
                OptionSpec::new(&["--oneline"], "Shorthand for '--pretty=oneline --abbrev-commit'"),
                OptionSpec::new(&["--graph"], "Draw text-based graphical commit history"),
                OptionSpec::new(&["-n"], "Limit the number of commits to output"),
            ]),
            SubcommandSpec::new("diff", "Show changes between commits, index, and working tree")
                .with_options(vec![
                    OptionSpec::new(&["--staged", "--cached"], "View staged changes for the next commit"),
                    OptionSpec::new(&["--stat"], "Generate a diffstat"),
                ]),
            SubcommandSpec::new("stash", "Stash changes in a dirty working directory").with_subcommands(vec![
                SubcommandSpec::new("pop", "Apply stashed state and remove from stash list"),
                SubcommandSpec::new("apply", "Apply stashed state and keep in stash list"),
                SubcommandSpec::new("list", "List stashes currently saved"),
                SubcommandSpec::new("drop", "Remove a single stash entry"),
                SubcommandSpec::new("clear", "Remove all stash entries"),
            ]),
            SubcommandSpec::new("restore", "Restore working tree files").with_options(vec![
                OptionSpec::new(&["--staged"], "Restore index / unstage files"),
            ]),
            SubcommandSpec::new("clone", "Clone a repository into a new directory").with_options(vec![
                OptionSpec::new(&["--depth"], "Create a shallow clone with truncated history"),
            ]),
            SubcommandSpec::new("merge", "Join two or more development histories together").with_options(vec![
                OptionSpec::new(&["--no-ff"], "Create a merge commit even on fast-forward"),
                OptionSpec::new(&["--abort"], "Abort current conflict resolution"),
            ]),
            SubcommandSpec::new("rebase", "Reapply commits on top of another base tip").with_options(vec![
                OptionSpec::new(&["-i", "--interactive"], "Interactive rebase"),
                OptionSpec::new(&["--continue"], "Continue rebase after resolving conflicts"),
                OptionSpec::new(&["--abort"], "Abort rebase and return to original branch"),
            ]),
        ])
        .with_options(vec![
            OptionSpec::new(&["--version"], "Print Git version"),
            OptionSpec::new(&["--help"], "Print help synopsis"),
            OptionSpec::new(&["-C"], "Run as if git was started in path"),
        ])
}

fn build_cargo_spec() -> CommandSpec {
    CommandSpec::new("cargo", "Rust package manager and build tool")
        .with_subcommands(vec![
            SubcommandSpec::new("build", "Compile the current package").with_options(vec![
                OptionSpec::new(&["--release"], "Build artifacts in release mode with optimizations"),
                OptionSpec::new(&["--workspace"], "Build all packages in the workspace"),
            ]),
            SubcommandSpec::new("test", "Execute all unit and integration tests").with_options(vec![
                OptionSpec::new(&["--workspace"], "Test all packages in the workspace"),
                OptionSpec::new(&["--nocapture"], "Print test stdout/stderr directly"),
            ]),
            SubcommandSpec::new("run", "Run a binary or example of the local package").with_options(vec![
                OptionSpec::new(&["--release"], "Run optimized release binary"),
                OptionSpec::new(&["-p", "--package"], "Specify package to run"),
            ]),
            SubcommandSpec::new("check", "Analyze the current package and report errors"),
            SubcommandSpec::new("clippy", "Checks a package to catch common mistakes"),
        ])
        .with_options(vec![
            OptionSpec::new(&["--workspace"], "Operate on all packages in the workspace"),
            OptionSpec::new(&["--release"], "Build artifacts in release mode"),
        ])
}

fn build_devops_specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("ssh", "OpenSSH remote login client").with_options(vec![
            OptionSpec::new(&["-p"], "Port to connect to on the remote host"),
            OptionSpec::new(&["-i"], "Identity file (private key) for public key auth"),
            OptionSpec::new(&["-L"], "Local port forwarding [bind_address:]port:host:hostport"),
            OptionSpec::new(&["-R"], "Remote port forwarding [bind_address:]port:host:hostport"),
            OptionSpec::new(&["-D"], "Dynamic SOCKS proxy port forwarding"),
            OptionSpec::new(&["-N"], "Do not execute a remote command (port forwarding only)"),
            OptionSpec::new(&["-X"], "Enables X11 forwarding"),
            OptionSpec::new(&["-v", "-vv", "-vvv"], "Verbose mode (debugging)"),
            OptionSpec::new(&["-C"], "Requests compression of all data"),
        ]),
        CommandSpec::new("curl", "Command line tool for transferring data with URLs").with_options(vec![
            OptionSpec::new(&["-s", "--silent"], "Silent mode"),
            OptionSpec::new(&["-S", "--show-error"], "Show error message on failure"),
            OptionSpec::new(&["-i", "--include"], "Include HTTP response headers in output"),
            OptionSpec::new(&["-I", "--head"], "Fetch HTTP response headers only"),
            OptionSpec::new(&["-L", "--location"], "Follow HTTP redirects"),
            OptionSpec::new(&["-o", "--output"], "Write output to <file> instead of stdout"),
            OptionSpec::new(&["-O", "--remote-name"], "Write output to local file named like remote"),
            OptionSpec::new(&["-X", "--request"], "Specify HTTP request method (GET, POST, PUT, DELETE)"),
            OptionSpec::new(&["-H", "--header"], "Pass custom header line to server"),
            OptionSpec::new(&["-d", "--data"], "HTTP POST data"),
            OptionSpec::new(&["-k", "--insecure"], "Allow insecure TLS connections"),
        ]),
        CommandSpec::new("tar", "Archive utility for storing and extracting files").with_options(vec![
            OptionSpec::new(&["-x", "--extract"], "Extract files from an archive"),
            OptionSpec::new(&["-c", "--create"], "Create a new archive"),
            OptionSpec::new(&["-v", "--verbose"], "Verbosely list files processed"),
            OptionSpec::new(&["-z", "--gzip"], "Filter the archive through gzip (.tar.gz)"),
            OptionSpec::new(&["-j", "--bzip2"], "Filter the archive through bzip2 (.tar.bz2)"),
            OptionSpec::new(&["-J", "--xz"], "Filter the archive through xz (.tar.xz)"),
            OptionSpec::new(&["-f", "--file"], "Use archive file or device"),
            OptionSpec::new(&["-C", "--directory"], "Change to directory before performing operations"),
            OptionSpec::new(&["-xzvf"], "Extract gzipped tar archive verbosely"),
            OptionSpec::new(&["-czvf"], "Create gzipped tar archive verbosely"),
        ]),
        CommandSpec::new("ufw", "Uncomplicated Firewall manager").with_subcommands(vec![
            SubcommandSpec::new("status", "Show firewall status").with_options(vec![
                OptionSpec::new(&["verbose"], "Show verbose firewall rules"),
                OptionSpec::new(&["numbered"], "Show firewall rules with rule numbers"),
            ]),
            SubcommandSpec::new("enable", "Enable the firewall"),
            SubcommandSpec::new("disable", "Disable the firewall"),
            SubcommandSpec::new("reload", "Reload firewall configuration"),
            SubcommandSpec::new("allow", "Allow traffic for service or port"),
            SubcommandSpec::new("deny", "Deny traffic for service or port"),
            SubcommandSpec::new("delete", "Delete a firewall rule"),
            SubcommandSpec::new("reset", "Reset firewall to factory defaults"),
        ]),
        CommandSpec::new("ip", "Show and manipulate routing, network devices, interfaces").with_subcommands(vec![
            SubcommandSpec::new("addr", "Protocol address management")
                .with_aliases(&["a", "address"])
                .with_options(vec![OptionSpec::new(&["show"], "Show IP addresses of all interfaces")]),
            SubcommandSpec::new("link", "Network device configuration")
                .with_aliases(&["l"])
                .with_options(vec![
                    OptionSpec::new(&["set"], "Change device attributes (up, down, mtu)"),
                    OptionSpec::new(&["show"], "Show interface state"),
                ]),
            SubcommandSpec::new("route", "Routing table management")
                .with_aliases(&["r"])
                .with_options(vec![OptionSpec::new(&["show"], "Show routing table")]),
            SubcommandSpec::new("neigh", "Neighbour/ARP table management").with_aliases(&["n"]),
        ]),
        CommandSpec::new("rsync", "Fast, versatile, remote and local file-copying tool").with_options(vec![
            OptionSpec::new(&["-a", "--archive"], "Archive mode (preserves permissions, times, symlinks)"),
            OptionSpec::new(&["-v", "--verbose"], "Increase verbosity"),
            OptionSpec::new(&["-z", "--compress"], "Compress file data during transfer"),
            OptionSpec::new(&["-P", "--progress"], "Show progress and keep partially transferred files"),
            OptionSpec::new(&["-h", "--human-readable"], "Output numbers in human-readable format"),
            OptionSpec::new(&["--delete"], "Delete extraneous files from destination dirs"),
            OptionSpec::new(&["-e"], "Specify the remote shell to use (e.g. ssh)"),
            OptionSpec::new(&["--exclude"], "Exclude files matching pattern"),
        ]),
        CommandSpec::new("ss", "Investigate sockets (modern netstat replacement)").with_options(vec![
            OptionSpec::new(&["-t", "--tcp"], "Display TCP sockets"),
            OptionSpec::new(&["-u", "--udp"], "Display UDP sockets"),
            OptionSpec::new(&["-l", "--listening"], "Display only listening sockets"),
            OptionSpec::new(&["-p", "--processes"], "Show process using socket"),
            OptionSpec::new(&["-n", "--numeric"], "Do not resolve service names"),
            OptionSpec::new(&["-tulpn"], "Show all listening TCP/UDP sockets with process & numeric ports"),
        ]),
    ]
}

fn build_common_specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("sudo", "Execute a command as the superuser or another user").with_options(vec![
            OptionSpec::new(&["-i", "--login"], "Run login shell as target user (root)"),
            OptionSpec::new(&["-s", "--shell"], "Run shell as the target user"),
            OptionSpec::new(&["-u", "--user"], "Run command as specified user instead of root"),
            OptionSpec::new(&["-E", "--preserve-env"], "Preserve existing environment variables"),
        ]),
        CommandSpec::new("chmod", "Change file mode bits (permissions)").with_options(vec![
            OptionSpec::new(&["-R", "--recursive"], "Change files and directories recursively"),
            OptionSpec::new(&["+x"], "Make file executable"),
            OptionSpec::new(&["755"], "rwxr-xr-x (standard directory / executable permissions)"),
            OptionSpec::new(&["644"], "rw-r--r-- (standard file permissions)"),
            OptionSpec::new(&["600"], "rw------- (secure private key permissions)"),
        ]),
        CommandSpec::new("chown", "Change file owner and group").with_options(vec![
            OptionSpec::new(&["-R", "--recursive"], "Operate on files and directories recursively"),
        ]),
        CommandSpec::new("mkdir", "Create directories if they do not already exist").with_options(vec![
            OptionSpec::new(&["-p", "--parents"], "Make parent directories as needed"),
            OptionSpec::new(&["-v", "--verbose"], "Print a message for each created directory"),
        ]),
        CommandSpec::new("rm", "Remove files or directories").with_options(vec![
            OptionSpec::new(&["-r", "-R", "--recursive"], "Remove directories and contents recursively"),
            OptionSpec::new(&["-f", "--force"], "Ignore nonexistent files, never prompt"),
            OptionSpec::new(&["-i"], "Prompt before every removal"),
        ]),
        CommandSpec::new("ls", "List directory contents").with_options(vec![
            OptionSpec::new(&["-la", "-lah"], "Long format, all files, human sizes"),
            OptionSpec::new(&["-l"], "Use a long listing format"),
            OptionSpec::new(&["-a", "--all"], "Do not ignore entries starting with ."),
            OptionSpec::new(&["-h", "--human-readable"], "Print sizes in human readable format"),
        ]),
        CommandSpec::new("grep", "Print lines that match patterns").with_options(vec![
            OptionSpec::new(&["-i", "--ignore-case"], "Ignore case distinctions"),
            OptionSpec::new(&["-r", "-R", "--recursive"], "Read all files under each directory recursively"),
            OptionSpec::new(&["-n", "--line-number"], "Prefix each line with 1-based line number"),
            OptionSpec::new(&["-v", "--invert-match"], "Select non-matching lines"),
            OptionSpec::new(&["-E", "--extended-regexp"], "Interpret pattern as extended regex"),
        ]),
        CommandSpec::new("find", "Search for files in a directory hierarchy").with_options(vec![
            OptionSpec::new(&["-name"], "Match base of file name against shell pattern"),
            OptionSpec::new(&["-iname"], "Case-insensitive file name pattern"),
            OptionSpec::new(&["-type"], "File type (f regular file, d directory, l symlink)"),
            OptionSpec::new(&["-mtime"], "File data modified n*24 hours ago"),
            OptionSpec::new(&["-size"], "File uses n units of space (e.g. +100M)"),
        ]),
        CommandSpec::new("df", "Report file system disk space usage").with_options(vec![
            OptionSpec::new(&["-h", "--human-readable"], "Print sizes in powers of 1024 (e.g. 1G)"),
            OptionSpec::new(&["-T", "--print-type"], "Print file system type"),
        ]),
        CommandSpec::new("du", "Estimate file and directory space usage").with_options(vec![
            OptionSpec::new(&["-sh"], "Summary of size in human-readable format"),
            OptionSpec::new(&["-h", "--human-readable"], "Print sizes in human readable format"),
            OptionSpec::new(&["-d 1", "--max-depth=1"], "Print total for directory depth 1"),
        ]),
        CommandSpec::new("free", "Display amount of free and used memory in the system").with_options(vec![
            OptionSpec::new(&["-h", "--human"], "Display memory in human-readable format"),
            OptionSpec::new(&["-m"], "Display memory in megabytes"),
        ]),
        CommandSpec::new("ps", "Report a snapshot of current processes").with_options(vec![
            OptionSpec::new(&["aux"], "See every process on the system (BSD style)"),
            OptionSpec::new(&["-ef"], "Standard full format listing of all processes"),
        ]),
        CommandSpec::new("tail", "Output the last part of files").with_options(vec![
            OptionSpec::new(&["-f", "--follow"], "Follow output live as file grows"),
            OptionSpec::new(&["-n 100"], "Output the last 100 lines"),
        ]),
        CommandSpec::new("ping", "Send ICMP ECHO_REQUEST to network hosts").with_options(vec![
            OptionSpec::new(&["-c 4"], "Stop after sending 4 packets"),
        ]),
    ]
}

#[derive(Debug, Default)]
pub struct CompletionEngine {
    pub registry: SpecRegistry,
}

impl CompletionEngine {
    pub fn new() -> Self {
        Self {
            registry: SpecRegistry::new_builtin(),
        }
    }

    /// Parses the active prompt line into a structured `CompletionContext`.
    pub fn parse_context(
        &self,
        full_line: &str,
        cursor_pos: Option<usize>,
        cwd: &str,
        host: &str,
    ) -> CompletionContext {
        let pos = cursor_pos.unwrap_or(full_line.len()).min(full_line.len());
        let line_before_cursor = &full_line[..pos];

        let delimiters = [";", "&&", "||", "|"];
        let mut last_delim_end = 0;
        for delim in delimiters {
            if let Some(idx) = line_before_cursor.rfind(delim) {
                last_delim_end = last_delim_end.max(idx + delim.len());
            }
        }

        let active_segment = &line_before_cursor[last_delim_end..];
        let ends_with_space = active_segment.ends_with(char::is_whitespace);
        let tokens: Vec<String> = active_segment
            .split_whitespace()
            .map(String::from)
            .collect();

        let current_word = if ends_with_space || active_segment.is_empty() {
            String::new()
        } else {
            tokens.last().cloned().unwrap_or_default()
        };

        let mut is_sudo = false;
        let mut command_root = String::new();
        if let Some(first) = tokens.first() {
            if first.eq_ignore_ascii_case("sudo") {
                is_sudo = true;
                command_root = tokens
                    .get(1)
                    .map(|s| s.to_lowercase())
                    .unwrap_or_else(|| "sudo".to_string());
            } else {
                command_root = first.to_lowercase();
            }
        }

        let token_index = if current_word.is_empty() {
            tokens.len() + 1
        } else {
            tokens.len()
        };

        CompletionContext {
            full_line: full_line.to_string(),
            cursor_position: pos,
            line_before_cursor: line_before_cursor.to_string(),
            tokens,
            current_word,
            token_index,
            command_root,
            is_sudo,
            cwd: cwd.to_string(),
            host: host.to_string(),
        }
    }

    /// Generates and ranks completions from specs and optional command history.
    pub fn get_completions(
        &self,
        full_line: &str,
        cursor_pos: Option<usize>,
        history: &[String],
        limit: usize,
    ) -> Vec<CompletionItem> {
        let ctx = self.parse_context(full_line, cursor_pos, "", "localhost");
        if ctx.line_before_cursor.trim().is_empty() && ctx.tokens.is_empty() {
            return Vec::new();
        }

        let mut items = self.spec_completions(&ctx);
        items.extend(self.history_completions(&ctx, history));

        items.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut seen = HashSet::new();
        let mut ranked = Vec::new();

        for mut item in items {
            let clean = item.text.trim().to_string();
            if seen.insert(clean) {
                if item.suffix_to_insert.is_empty() {
                    if !ctx.current_word.is_empty()
                        && item
                            .text
                            .to_lowercase()
                            .starts_with(&ctx.current_word.to_lowercase())
                    {
                        item.suffix_to_insert = item.text[ctx.current_word.len()..].to_string();
                    } else if ctx.current_word.is_empty() {
                        item.suffix_to_insert = item.text.clone();
                    }
                }
                ranked.push(item);
                if ranked.len() >= limit {
                    break;
                }
            }
        }

        ranked
    }

    fn spec_completions(&self, ctx: &CompletionContext) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        let ends_space = ctx.line_before_cursor.ends_with(char::is_whitespace);

        // Case A: First command or `sudo <cmd>`
        let typing_root = ctx.tokens.is_empty()
            || (ctx.tokens.len() == 1 && !ends_space)
            || (ctx.is_sudo && (ctx.tokens.len() == 1 || (ctx.tokens.len() == 2 && !ends_space)));

        if typing_root {
            let prefix = ctx.current_word.to_lowercase();
            for cmd_name in self.registry.command_names() {
                if ctx.is_sudo && cmd_name == "sudo" {
                    continue;
                }
                if prefix.is_empty() || cmd_name.starts_with(&prefix) {
                    let desc = self
                        .registry
                        .get_spec(cmd_name)
                        .map(|s| s.description.clone())
                        .unwrap_or_default();
                    let score = if cmd_name == prefix { 3.0 } else { 2.5 };
                    items.push(CompletionItem {
                        text: cmd_name.to_string(),
                        description: desc,
                        completion_type: CompletionType::Command,
                        source: CompletionSource::Spec,
                        score,
                        prefix_to_replace: ctx.current_word.clone(),
                        suffix_to_insert: String::new(),
                    });
                }
            }
            return items;
        }

        // Case B: Delegate to the specific command's `CommandSpec`
        if let Some(spec) = self.registry.get_spec(&ctx.command_root) {
            items.extend(spec.get_completions(ctx));
        }

        items
    }

    fn history_completions(
        &self,
        ctx: &CompletionContext,
        history: &[String],
    ) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        let line_prefix = ctx.line_before_cursor.trim();
        if line_prefix.is_empty() {
            return items;
        }
        for cmd in history {
            if cmd.starts_with(line_prefix) && cmd.trim() != line_prefix {
                items.push(CompletionItem {
                    text: cmd.clone(),
                    description: "Command History".to_string(),
                    completion_type: CompletionType::History,
                    source: CompletionSource::History,
                    score: 2.0,
                    prefix_to_replace: line_prefix.to_string(),
                    suffix_to_insert: cmd[line_prefix.len()..].to_string(),
                });
            }
        }
        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_engine_compound_and_sudo_context() {
        let engine = CompletionEngine::new();
        let ctx = engine.parse_context("cd /var && sudo systemctl res", None, "/var", "localhost");
        assert!(ctx.is_sudo);
        assert_eq!(ctx.command_root, "systemctl");
        assert_eq!(ctx.current_word, "res");

        let results = engine.get_completions("cd /var && sudo systemctl res", None, &[], 6);
        assert!(!results.is_empty());
        assert_eq!(results[0].text, "restart");
        assert_eq!(results[0].suffix_to_insert, "tart");
    }

    #[test]
    fn test_package_managers_rpm_dnf_apt_pacman_flatpak_zypper_snap() {
        let engine = CompletionEngine::new();

        // RPM
        let rpm_subs = engine.get_completions("sudo rpm -", None, &[], 10);
        assert!(rpm_subs.iter().any(|i| i.text == "-ivh"));
        assert!(rpm_subs.iter().any(|i| i.text == "-qa"));
        let rpm_flags = engine.get_completions("sudo rpm -ivh --no", None, &[], 6);
        assert!(rpm_flags.iter().any(|i| i.text == "--nodeps"));

        // DNF & YUM alias
        let dnf_subs = engine.get_completions("sudo dnf in", None, &[], 6);
        assert!(dnf_subs.iter().any(|i| i.text == "install"));
        let yum_flags = engine.get_completions("sudo yum install --allow", None, &[], 6);
        assert!(yum_flags.iter().any(|i| i.text == "--allowerasing"));

        // APT & APT-GET alias + DPKG
        let apt_flags = engine.get_completions("sudo apt-get install --no-", None, &[], 6);
        assert!(apt_flags.iter().any(|i| i.text == "--no-install-recommends"));
        let dpkg_subs = engine.get_completions("sudo dpkg -", None, &[], 8);
        assert!(dpkg_subs.iter().any(|i| i.text == "-i"));

        // Pacman
        let pac_subs = engine.get_completions("sudo pacman -Sy", None, &[], 6);
        assert!(pac_subs.iter().any(|i| i.text == "-Syu"));

        // Flatpak subcommand options
        let fp = engine.get_completions("flatpak in", None, &[], 6);
        assert!(fp.iter().any(|i| i.text == "install"));
        assert!(fp.iter().any(|i| i.text == "info"));
        let fp_override = engine.get_completions("flatpak override --file", None, &[], 6);
        assert!(fp_override.iter().any(|i| i.text == "--filesystem"));

        // Zypper & Snap
        let zyp = engine.get_completions("sudo zypper du", None, &[], 6);
        assert!(zyp.iter().any(|i| i.text == "dup"));
        let snap = engine.get_completions("sudo snap install --cl", None, &[], 6);
        assert!(snap.iter().any(|i| i.text == "--classic"));
    }
}
