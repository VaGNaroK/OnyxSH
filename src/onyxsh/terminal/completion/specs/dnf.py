# onyxsh/terminal/completion/specs/dnf.py
"""
Completion specifications for DNF package manager family (dnf, yum, microdnf).
Fedora / RHEL / CentOS / Rocky / AlmaLinux.
"""

from ....utils.translation_utils import _
from .base import CommandSpec, OptionSpec, SubcommandSpec


def get_dnf_spec() -> CommandSpec:
    """Builds and returns the DNF command spec."""
    return CommandSpec(
        name="dnf",
        description=_("Fedora/RHEL package manager utility"),
        aliases=["yum", "microdnf"],
        subcommands=[
            SubcommandSpec(
                name="install",
                description=_("Install one or more packages"),
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatically answer yes to all questions")),
                    OptionSpec(["--refresh"], _("Refresh metadata before running the command")),
                    OptionSpec(["--enablerepo"], _("Enable additional repositories"), takes_value=True, value_name="<repoid>"),
                    OptionSpec(["--disablerepo"], _("Disable repositories"), takes_value=True, value_name="<repoid>"),
                    OptionSpec(["--releasever"], _("Override the $releasever variable"), takes_value=True, value_name="<version>"),
                ],
                examples=["dnf install curl git", "dnf install -y nginx"],
            ),
            SubcommandSpec(
                name="remove",
                description=_("Remove one or more installed packages"),
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatically answer yes to all questions")),
                    OptionSpec(["--noautoremove"], _("Do not auto-remove unused dependencies")),
                ],
                examples=["dnf remove nginx"],
            ),
            SubcommandSpec(
                name="reinstall",
                description=_("Reinstall packages even if already installed"),
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatically answer yes to all questions")),
                ],
            ),
            SubcommandSpec(
                name="upgrade",
                description=_("Upgrade all installed packages to newest versions"),
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatically answer yes to all questions")),
                    OptionSpec(["--refresh"], _("Refresh metadata before running the command")),
                    OptionSpec(["--security"], _("Limit to security-related upgrades")),
                ],
                examples=["dnf upgrade -y"],
            ),
            SubcommandSpec(
                name="update",
                description=_("Alias for upgrade (update package metadata and packages)"),
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatically answer yes to all questions")),
                    OptionSpec(["--refresh"], _("Refresh metadata before running the command")),
                ],
            ),
            SubcommandSpec(
                name="downgrade",
                description=_("Downgrade packages to an older version"),
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatically answer yes to all questions")),
                ],
            ),
            SubcommandSpec(
                name="check-update",
                description=_("Check for available package updates"),
                options=[
                    OptionSpec(["--refresh"], _("Refresh metadata before running the command")),
                ],
                examples=["dnf check-update"],
            ),
            SubcommandSpec(
                name="search",
                description=_("Search package names and descriptions"),
                examples=["dnf search python3", "dnf search ripgrep"],
            ),
            SubcommandSpec(
                name="info",
                description=_("Display detailed package information"),
                examples=["dnf info vlc", "dnf info openssh-server"],
            ),
            SubcommandSpec(
                name="list",
                description=_("List packages based on criteria"),
                options=[
                    OptionSpec(["--installed"], _("List only installed packages")),
                    OptionSpec(["--available"], _("List only available packages")),
                    OptionSpec(["--updates"], _("List packages with available updates")),
                ],
                examples=["dnf list --installed", "dnf list --updates"],
            ),
            SubcommandSpec(
                name="provides",
                description=_("Find which package provides a file or feature"),
                examples=["dnf provides /usr/bin/python3"],
            ),
            SubcommandSpec(
                name="whatprovides",
                description=_("Alias for provides"),
            ),
            SubcommandSpec(
                name="repolist",
                description=_("List configured software repositories"),
                options=[
                    OptionSpec(["--all"], _("Show all repositories including disabled ones")),
                    OptionSpec(["--enabled"], _("Show only enabled repositories")),
                ],
                examples=["dnf repolist", "dnf repolist --all"],
            ),
            SubcommandSpec(
                name="group",
                description=_("Manage package groups (install, remove, list)"),
                subcommands=[
                    SubcommandSpec(name="install", description=_("Install a package group")),
                    SubcommandSpec(name="remove", description=_("Remove a package group")),
                    SubcommandSpec(name="list", description=_("List package groups")),
                    SubcommandSpec(name="info", description=_("Show package group information")),
                ],
                examples=['dnf group install "Development Tools"'],
            ),
            SubcommandSpec(
                name="history",
                description=_("Display or manage transaction history"),
                subcommands=[
                    SubcommandSpec(name="list", description=_("List past transactions")),
                    SubcommandSpec(name="info", description=_("Show transaction details")),
                    SubcommandSpec(name="undo", description=_("Revert a past transaction")),
                    SubcommandSpec(name="redo", description=_("Repeat a past transaction")),
                ],
                examples=["dnf history", "dnf history undo last"],
            ),
            SubcommandSpec(
                name="clean",
                description=_("Remove cached package data"),
                options=[
                    OptionSpec(["all"], _("Remove all cached data")),
                    OptionSpec(["packages"], _("Remove cached packages only")),
                    OptionSpec(["metadata"], _("Remove repository metadata only")),
                ],
                examples=["dnf clean all"],
            ),
            SubcommandSpec(
                name="autoremove",
                description=_("Remove automatically installed dependencies no longer needed"),
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatically answer yes to all questions")),
                ],
                examples=["dnf autoremove -y"],
            ),
            SubcommandSpec(
                name="makecache",
                description=_("Download and cache repository metadata"),
            ),
        ],
        global_options=[
            OptionSpec(["-h", "--help"], _("Show help information")),
            OptionSpec(["-y", "--assumeyes"], _("Assume yes to all queries")),
            OptionSpec(["-q", "--quiet"], _("Produce quiet log output")),
            OptionSpec(["-v", "--verbose"], _("Produce verbose log output")),
            OptionSpec(["--version"], _("Show version number")),
            OptionSpec(["--refresh"], _("Refresh metadata before running the command")),
        ],
    )
