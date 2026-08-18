# onyxsh/terminal/completion/specs/apt.py
"""
Completion specifications for APT package manager family (apt, apt-get, dpkg).
"""

from ....utils.translation_utils import _
from .base import CommandSpec, OptionSpec, SubcommandSpec


def get_apt_spec() -> CommandSpec:
    """Builds and returns the APT command spec."""
    return CommandSpec(
        name="apt",
        description=_("Debian/Ubuntu package manager utility"),
        aliases=["apt-get"],
        subcommands=[
            SubcommandSpec(
                name="install",
                description=_("Install one or more packages"),
                options=[
                    OptionSpec(["-y", "--yes"], _("Automatic yes to prompts")),
                    OptionSpec(["--no-install-recommends"], _("Do not install recommended packages")),
                    OptionSpec(["--reinstall"], _("Reinstall packages even if up to date")),
                    OptionSpec(["--dry-run", "-s"], _("Simulate installation")),
                ],
                examples=["apt install curl git", "apt install -y nginx"],
            ),
            SubcommandSpec(
                name="update",
                description=_("Update list of available packages from repositories"),
                options=[
                    OptionSpec(["-q", "--quiet"], _("Quiet mode")),
                ],
                examples=["apt update"],
            ),
            SubcommandSpec(
                name="upgrade",
                description=_("Upgrade all installed packages to newest versions"),
                options=[
                    OptionSpec(["-y", "--yes"], _("Automatic yes to prompts")),
                    OptionSpec(["--without-new-pkgs"], _("Do not install new packages during upgrade")),
                ],
                examples=["apt upgrade -y"],
            ),
            SubcommandSpec(
                name="full-upgrade",
                description=_("Upgrade packages removing/installing dependencies if needed"),
                options=[
                    OptionSpec(["-y", "--yes"], _("Automatic yes to prompts")),
                ],
            ),
            SubcommandSpec(
                name="remove",
                description=_("Remove one or more installed packages"),
                options=[
                    OptionSpec(["--purge"], _("Remove configuration files along with packages")),
                    OptionSpec(["-y", "--yes"], _("Automatic yes to prompts")),
                ],
                examples=["apt remove nginx", "apt remove --purge apache2"],
            ),
            SubcommandSpec(
                name="purge",
                description=_("Remove packages and all their configuration files"),
                options=[
                    OptionSpec(["-y", "--yes"], _("Automatic yes to prompts")),
                ],
            ),
            SubcommandSpec(
                name="autoremove",
                description=_("Remove automatically installed dependencies no longer needed"),
                options=[
                    OptionSpec(["--purge"], _("Also remove configuration files of orphaned packages")),
                    OptionSpec(["-y", "--yes"], _("Automatic yes to prompts")),
                ],
                examples=["apt autoremove -y"],
            ),
            SubcommandSpec(
                name="search",
                description=_("Search package names and descriptions"),
                examples=["apt search python3", "apt search ripgrep"],
            ),
            SubcommandSpec(
                name="show",
                description=_("Display detailed package information"),
                examples=["apt show vlc", "apt show openssh-server"],
            ),
            SubcommandSpec(
                name="list",
                description=_("List packages based on criteria"),
                options=[
                    OptionSpec(["--installed"], _("List only installed packages")),
                    OptionSpec(["--upgradable"], _("List packages with available updates")),
                    OptionSpec(["--all-versions"], _("List all available versions")),
                ],
                examples=["apt list --installed", "apt list --upgradable"],
            ),
            SubcommandSpec(
                name="clean",
                description=_("Clear out the local repository cache of retrieved package files"),
            ),
            SubcommandSpec(
                name="autoclean",
                description=_("Erase old downloaded archive files"),
            ),
        ],
        global_options=[
            OptionSpec(["-h", "--help"], _("Show help information")),
            OptionSpec(["-v", "--version"], _("Show version number")),
            OptionSpec(["-y", "--yes"], _("Assume yes to all queries")),
            OptionSpec(["-q", "--quiet"], _("Produce quiet log output")),
        ],
    )
