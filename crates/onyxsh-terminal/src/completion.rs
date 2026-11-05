//! Multi-source Intelligent Autocomplete Engine and Declarative Command Specs.
//! Port of `src/onyxsh/terminal/completion/engine.py`, `models.py`, and `specs/*`.

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionType {
    Command,
    Subcommand,
    OptionFlag,
    History,
    Snippet,
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
pub struct CommandSpec {
    pub name: String,
    pub description: String,
    pub subcommands: Vec<(String, String)>,
    pub options: Vec<(String, String)>,
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
        let builtin = [
            (
                "git",
                "Distributed version control system",
                vec![
                    ("status", "Show the working tree status"),
                    ("checkout", "Switch branches or restore working tree files"),
                    ("commit", "Record changes to the repository"),
                    ("push", "Update remote refs along with associated objects"),
                    ("pull", "Fetch from and integrate with another repository"),
                    ("branch", "List, create, or delete branches"),
                    ("log", "Show commit logs"),
                    ("diff", "Show changes between commits"),
                ],
                vec![
                    ("--version", "Print git version"),
                    ("--help", "Display help information"),
                ],
            ),
            (
                "docker",
                "Container runtime and management CLI",
                vec![
                    ("ps", "List containers"),
                    ("images", "List images"),
                    ("run", "Run a command in a new container"),
                    ("exec", "Execute a command in a running container"),
                    ("logs", "Fetch the logs of a container"),
                    ("compose", "Docker Compose multi-container applications"),
                    ("build", "Build an image from a Dockerfile"),
                ],
                vec![
                    ("-d", "Run container in background (detached)"),
                    ("-it", "Interactive TTY allocation"),
                    ("--rm", "Automatically remove container when it exits"),
                ],
            ),
            (
                "systemctl",
                "Control the systemd system and service manager",
                vec![
                    ("status", "Show runtime status of units"),
                    ("start", "Start (activate) one or more units"),
                    ("stop", "Stop (deactivate) one or more units"),
                    ("restart", "Stop and then start one or more units"),
                    ("enable", "Enable one or more unit files"),
                    ("disable", "Disable one or more unit files"),
                    ("daemon-reload", "Reload systemd manager configuration"),
                ],
                vec![
                    ("--user", "Talk to the service manager of the calling user"),
                    ("--now", "Start or stop unit immediately when enabling/disabling"),
                ],
            ),
            (
                "apt",
                "Debian/Ubuntu package manager",
                vec![
                    ("update", "Update list of available packages"),
                    ("upgrade", "Upgrade the system by installing/upgrading packages"),
                    ("install", "Install packages"),
                    ("remove", "Remove packages"),
                    ("search", "Search in package descriptions"),
                    ("autoremove", "Remove automatically all unused packages"),
                ],
                vec![("-y", "Automatic yes to prompts")],
            ),
            (
                "flatpak",
                "Build, install, and run sandboxed desktop applications",
                vec![
                    ("install", "Install an application or runtime"),
                    ("update", "Update an installed application or runtime"),
                    ("uninstall", "Uninstall an installed application"),
                    ("list", "List installed apps and/or runtimes"),
                    ("run", "Run an application"),
                    ("info", "Show info for an installed app or runtime"),
                ],
                vec![
                    ("--user", "Work on the user installation"),
                    ("--system", "Work on the system-wide installation"),
                    ("-y", "Automatically answer yes for all questions"),
                ],
            ),
            (
                "cargo",
                "Rust package manager and build tool",
                vec![
                    ("build", "Compile the current package"),
                    ("test", "Execute all unit and integration tests"),
                    ("run", "Run a binary or example of the local package"),
                    ("check", "Analyze the current package and report errors"),
                    ("clippy", "Checks a package to catch common mistakes"),
                ],
                vec![
                    ("--workspace", "Operate on all packages in the workspace"),
                    ("--release", "Build artifacts in release mode, with optimizations"),
                ],
            ),
        ];

        for (name, desc, subs, opts) in builtin {
            specs.insert(
                name.to_string(),
                CommandSpec {
                    name: name.to_string(),
                    description: desc.to_string(),
                    subcommands: subs
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect(),
                    options: opts
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect(),
                },
            );
        }

        Self { specs }
    }

    pub fn get_spec(&self, name: &str) -> Option<&CommandSpec> {
        self.specs.get(name)
    }

    pub fn command_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.specs.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }
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
                    if !ctx.current_word.is_empty() && item.text.starts_with(&ctx.current_word) {
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

        // Case B: Subcommands and flags of `command_root`
        if let Some(spec) = self.registry.get_spec(&ctx.command_root) {
            let prefix = ctx.current_word.to_lowercase();
            if prefix.starts_with('-') {
                for (opt, desc) in &spec.options {
                    if opt.to_lowercase().starts_with(&prefix) {
                        items.push(CompletionItem {
                            text: opt.clone(),
                            description: desc.clone(),
                            completion_type: CompletionType::OptionFlag,
                            source: CompletionSource::Spec,
                            score: 2.8,
                            prefix_to_replace: ctx.current_word.clone(),
                            suffix_to_insert: String::new(),
                        });
                    }
                }
            } else {
                for (sub, desc) in &spec.subcommands {
                    if prefix.is_empty() || sub.to_lowercase().starts_with(&prefix) {
                        items.push(CompletionItem {
                            text: sub.clone(),
                            description: desc.clone(),
                            completion_type: CompletionType::Subcommand,
                            source: CompletionSource::Spec,
                            score: 2.9,
                            prefix_to_replace: ctx.current_word.clone(),
                            suffix_to_insert: String::new(),
                        });
                    }
                }
            }
        }

        items
    }

    fn history_completions(&self, ctx: &CompletionContext, history: &[String]) -> Vec<CompletionItem> {
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
    fn test_flatpak_and_git_subcommand_completions() {
        let engine = CompletionEngine::new();
        let fp = engine.get_completions("flatpak in", None, &[], 6);
        assert!(fp.iter().any(|i| i.text == "install"));
        assert!(fp.iter().any(|i| i.text == "info"));
    }
}
