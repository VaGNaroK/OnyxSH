# onyxsh/terminal/completion/specs/pacman.py
"""
Completion specifications for Pacman package manager family.
Used in Arch Linux, Manjaro, EndeavourOS, and derivative distributions.
"""

from ....utils.translation_utils import _
from .base import CommandSpec, OptionSpec, SubcommandSpec


def get_pacman_spec() -> CommandSpec:
    """Builds and returns the Pacman command spec."""
    return CommandSpec(
        name="pacman",
        description=_("Arch Linux package manager utility"),
        aliases=[],
        subcommands=[
            SubcommandSpec(
                name="-S",
                description=_("Install or synchronize packages from remote repositories"),
                aliases=["--sync"],
                options=[
                    OptionSpec(["-y", "--refresh"], _("Download fresh copy of package databases from servers")),
                    OptionSpec(["-u", "--sysupgrade"], _("Upgrade all packages that are out of date")),
                    OptionSpec(["-w", "--downloadonly"], _("Download packages only, do not install or unpack")),
                    OptionSpec(["-c", "--clean"], _("Remove old packages from cache directory")),
                    OptionSpec(["-s", "--search"], _("Search remote repositories for matching regex")),
                    OptionSpec(["-i", "--info"], _("View information about a remote package")),
                    OptionSpec(["-g", "--groups"], _("Display all packages in specified group")),
                    OptionSpec(["--needed"], _("Do not reinstall up-to-date packages")),
                    OptionSpec(["--asdeps"], _("Install packages as non-explicit dependencies")),
                    OptionSpec(["--asexplicit"], _("Install packages as explicitly requested")),
                    OptionSpec(["--noconfirm"], _("Bypass any and all 'Are you sure?' prompts")),
                ],
                examples=["pacman -S firefox", "pacman -S --needed git base-devel", "pacman -Sy"],
            ),
            SubcommandSpec(
                name="-Syu",
                description=_("Full system upgrade (refresh repositories and update all packages)"),
                options=[
                    OptionSpec(["--noconfirm"], _("Bypass any and all confirmation prompts")),
                    OptionSpec(["--needed"], _("Do not reinstall up-to-date packages")),
                    OptionSpec(["--ignore"], _("Ignore a package upgrade"), takes_value=True, value_name="<pkg>"),
                ],
                examples=["pacman -Syu", "pacman -Syu --noconfirm"],
            ),
            SubcommandSpec(
                name="-Syyu",
                description=_("Force refresh all package databases and full system upgrade"),
                options=[
                    OptionSpec(["--noconfirm"], _("Bypass any and all confirmation prompts")),
                ],
                examples=["pacman -Syyu"],
            ),
            SubcommandSpec(
                name="-R",
                description=_("Remove one or more packages from the system"),
                aliases=["--remove"],
                options=[
                    OptionSpec(["-s", "--recursive"], _("Remove dependencies not required by other packages")),
                    OptionSpec(["-n", "--nosave"], _("Ignore file backup designates (remove config files)")),
                    OptionSpec(["-c", "--cascade"], _("Remove all target packages and all packages that depend on them")),
                    OptionSpec(["-u", "--unneeded"], _("Remove unneeded packages")),
                    OptionSpec(["--noconfirm"], _("Bypass any confirmation prompts")),
                ],
                examples=["pacman -R package", "pacman -Rns package"],
            ),
            SubcommandSpec(
                name="-Rns",
                description=_("Cleanly remove package with all unused dependencies and config files"),
                options=[
                    OptionSpec(["--noconfirm"], _("Bypass any confirmation prompts")),
                ],
                examples=["pacman -Rns nginx", "pacman -Rns $(pacman -Qtdq)"],
            ),
            SubcommandSpec(
                name="-Ss",
                description=_("Search remote repositories for packages matching keyword"),
                examples=["pacman -Ss python", "pacman -Ss ripgrep"],
            ),
            SubcommandSpec(
                name="-Si",
                description=_("Display detailed information about a remote package"),
                examples=["pacman -Si linux", "pacman -Si docker"],
            ),
            SubcommandSpec(
                name="-Q",
                description=_("Query the local package database"),
                aliases=["--query"],
                options=[
                    OptionSpec(["-e", "--explicit"], _("List packages that were explicitly installed")),
                    OptionSpec(["-d", "--deps"], _("List packages that were installed as dependencies")),
                    OptionSpec(["-t", "--unrequired"], _("List packages not required by any installed package")),
                    OptionSpec(["-m", "--foreign"], _("List packages not found in sync database (e.g. from AUR)")),
                    OptionSpec(["-s", "--search"], _("Search locally-installed packages for matching strings")),
                    OptionSpec(["-i", "--info"], _("View information about a locally-installed package")),
                    OptionSpec(["-l", "--list"], _("List all files owned by a given package")),
                    OptionSpec(["-o", "--owns"], _("Search for package that owns the specified file"), takes_value=True, value_name="<file>"),
                    OptionSpec(["-q", "--quiet"], _("Show less information for query output")),
                ],
                examples=["pacman -Qe", "pacman -Qdt", "pacman -Qm"],
            ),
            SubcommandSpec(
                name="-Qdt",
                description=_("List orphaned packages (dependencies no longer required by any package)"),
                options=[
                    OptionSpec(["-q", "--quiet"], _("Output package names only (suitable for piping)")),
                ],
                examples=["pacman -Qdt", "pacman -Qdtq"],
            ),
            SubcommandSpec(
                name="-Qi",
                description=_("Display detailed information about a locally installed package"),
                examples=["pacman -Qi glibc", "pacman -Qi systemd"],
            ),
            SubcommandSpec(
                name="-Ql",
                description=_("List all files owned by a locally installed package"),
                examples=["pacman -Ql bash", "pacman -Ql python"],
            ),
            SubcommandSpec(
                name="-Qo",
                description=_("Query which package owns a specific file on the filesystem"),
                examples=["pacman -Qo /usr/bin/python3", "pacman -Qo /etc/systemd/system.conf"],
            ),
            SubcommandSpec(
                name="-Sc",
                description=_("Clean package cache (remove old downloaded packages from /var/cache/pacman/pkg)"),
                options=[
                    OptionSpec(["--noconfirm"], _("Bypass confirmation prompt")),
                ],
                examples=["pacman -Sc", "pacman -Sc --noconfirm"],
            ),
            SubcommandSpec(
                name="-Scc",
                description=_("Clean all package cache (remove all downloaded packages from cache)"),
                options=[
                    OptionSpec(["--noconfirm"], _("Bypass confirmation prompt")),
                ],
                examples=["pacman -Scc"],
            ),
            SubcommandSpec(
                name="-U",
                description=_("Upgrade or install local or remote package file (.pkg.tar.zst)"),
                aliases=["--upgrade"],
                options=[
                    OptionSpec(["--needed"], _("Do not reinstall up-to-date packages")),
                    OptionSpec(["--noconfirm"], _("Bypass any confirmation prompts")),
                    OptionSpec(["--asdeps"], _("Install packages as non-explicit dependencies")),
                ],
                examples=["pacman -U package.pkg.tar.zst", "pacman -U https://example.com/pkg.tar.zst"],
            ),
            SubcommandSpec(
                name="-Fy",
                description=_("Refresh files database (for searching which package provides a file)"),
                examples=["pacman -Fy"],
            ),
            SubcommandSpec(
                name="-Fs",
                description=_("Search files database for a filename"),
                examples=["pacman -Fs libvte-2.91.so", "pacman -Fs python3"],
            ),
        ],
        global_options=[
            OptionSpec(["-h", "--help"], _("Display syntax and options")),
            OptionSpec(["-V", "--version"], _("Display version information")),
            OptionSpec(["-v", "--verbose"], _("Output more detailed diagnostic information")),
            OptionSpec(["-q", "--quiet"], _("Show less information for query and search outputs")),
            OptionSpec(["--noconfirm"], _("Bypass any and all confirmation questions")),
            OptionSpec(["--needed"], _("Do not reinstall up-to-date targets")),
            OptionSpec(["--color"], _("Colorize output (auto, always, never)"), takes_value=True),
            OptionSpec(["--cachedir"], _("Set an alternate package cache location"), takes_value=True, value_name="<dir>"),
            OptionSpec(["--config"], _("Set an alternate configuration file"), takes_value=True, value_name="<file>"),
            OptionSpec(["--root"], _("Set an alternate installation root"), takes_value=True, value_name="<path>"),
        ],
    )
