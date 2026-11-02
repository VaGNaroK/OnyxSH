# onyxsh/terminal/completion/specs/flatpak.py
"""
Completion specifications for Flatpak application and package management utility.
Covers installation, runtimes, sandboxes, overrides, remotes, and maintenance.
"""

from ....utils.translation_utils import _
from .base import CommandSpec, OptionSpec, SubcommandSpec


def get_flatpak_spec() -> CommandSpec:
    """Builds and returns the Flatpak command specification."""
    return CommandSpec(
        name="flatpak",
        description=_("Application sandboxing and package distribution utility"),
        subcommands=[
            SubcommandSpec(
                name="install",
                description=_("Install an application or runtime from a remote or bundle"),
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatically answer yes to all questions")),
                    OptionSpec(["--noninteractive"], _("Produce minimal output and never ask questions")),
                    OptionSpec(["--user"], _("Install in user storage")),
                    OptionSpec(["--system"], _("Install in system-wide storage")),
                    OptionSpec(["--reinstall"], _("Reinstall even if already installed")),
                    OptionSpec(["--from"], _("Install from a .flatpakref file or URL"), takes_value=True, value_name="<location>"),
                    OptionSpec(["--bundle"], _("Install from a .flatpak bundle file"), takes_value=True, value_name="<location>"),
                    OptionSpec(["--arch"], _("Specify the architecture to install"), takes_value=True, value_name="<arch>"),
                    OptionSpec(["--subpath"], _("Install only a specific subpath"), takes_value=True, value_name="<path>"),
                    OptionSpec(["--or-update"], _("Update the app/runtime if it is already installed")),
                ],
                examples=[
                    "flatpak install flathub org.gnome.Calculator",
                    "flatpak install -y --user flathub io.github.vagnarok.OnyxSH",
                ],
            ),
            SubcommandSpec(
                name="uninstall",
                description=_("Uninstall an installed application or runtime"),
                aliases=["remove"],
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatically answer yes to all questions")),
                    OptionSpec(["--noninteractive"], _("Produce minimal output and never ask questions")),
                    OptionSpec(["--user"], _("Uninstall from user storage")),
                    OptionSpec(["--system"], _("Uninstall from system-wide storage")),
                    OptionSpec(["--unused"], _("Remove unused runtimes and extensions")),
                    OptionSpec(["--delete-data"], _("Delete application data in ~/.var/app")),
                    OptionSpec(["--all"], _("Remove all matching applications/runtimes")),
                ],
                examples=[
                    "flatpak uninstall org.gnome.Calculator",
                    "flatpak uninstall --unused -y",
                ],
            ),
            SubcommandSpec(
                name="update",
                description=_("Update installed applications and runtimes"),
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatically answer yes to all questions")),
                    OptionSpec(["--noninteractive"], _("Produce minimal output and never ask questions")),
                    OptionSpec(["--user"], _("Update only user installations")),
                    OptionSpec(["--system"], _("Update only system-wide installations")),
                    OptionSpec(["--app"], _("Update only applications")),
                    OptionSpec(["--runtime"], _("Update only runtimes")),
                    OptionSpec(["--subpath"], _("Update only a specific subpath"), takes_value=True, value_name="<path>"),
                ],
                examples=[
                    "flatpak update",
                    "flatpak update -y",
                    "flatpak update --user",
                ],
            ),
            SubcommandSpec(
                name="list",
                description=_("List installed applications and/or runtimes"),
                options=[
                    OptionSpec(["--app"], _("List only applications")),
                    OptionSpec(["--runtime"], _("List only runtimes")),
                    OptionSpec(["--all", "-a"], _("List all installed apps and runtimes")),
                    OptionSpec(["-d", "--show-details"], _("Show full details including commit and runtime")),
                    OptionSpec(["--columns"], _("Specify columns to display"), takes_value=True, value_name="<columns>"),
                    OptionSpec(["--user"], _("List only user-installed applications")),
                    OptionSpec(["--system"], _("List only system-installed applications")),
                    OptionSpec(["--arch"], _("Filter by architecture"), takes_value=True, value_name="<arch>"),
                ],
                examples=[
                    "flatpak list --app",
                    "flatpak list -d",
                    "flatpak list --columns=application,version,size",
                ],
            ),
            SubcommandSpec(
                name="search",
                description=_("Search for applications and runtimes in remote repositories"),
                options=[
                    OptionSpec(["--user"], _("Search user repositories")),
                    OptionSpec(["--system"], _("Search system-wide repositories")),
                    OptionSpec(["--columns"], _("Specify columns to display"), takes_value=True, value_name="<columns>"),
                ],
                examples=[
                    "flatpak search vlc",
                    "flatpak search text editor",
                ],
            ),
            SubcommandSpec(
                name="info",
                description=_("Show detailed information about an installed application or runtime"),
                options=[
                    OptionSpec(["-m", "--show-metadata"], _("Show application metadata")),
                    OptionSpec(["--show-permissions"], _("Show sandbox permissions")),
                    OptionSpec(["--show-extensions"], _("Show configured extensions")),
                    OptionSpec(["--file-access"], _("Show permissions for a specific file access"), takes_value=True, value_name="<path>"),
                    OptionSpec(["--user"], _("Query user installation")),
                    OptionSpec(["--system"], _("Query system-wide installation")),
                ],
                examples=[
                    "flatpak info org.gnome.Calculator",
                    "flatpak info --show-permissions io.github.vagnarok.OnyxSH",
                ],
            ),
            SubcommandSpec(
                name="run",
                description=_("Run an application in an isolated sandbox"),
                options=[
                    OptionSpec(["--command"], _("Execute alternative command instead of default binary"), takes_value=True, value_name="<cmd>"),
                    OptionSpec(["--branch"], _("Select specific branch"), takes_value=True, value_name="<branch>"),
                    OptionSpec(["--arch"], _("Specify architecture to run"), takes_value=True, value_name="<arch>"),
                    OptionSpec(["--devel"], _("Use development SDK instead of runtime")),
                    OptionSpec(["--file-forwarding"], _("Enable file forwarding for arguments")),
                    OptionSpec(["-v", "--verbose"], _("Print debug logging while launching")),
                ],
                examples=[
                    "flatpak run org.gnome.Calculator",
                    "flatpak run --command=bash io.github.vagnarok.OnyxSH",
                ],
            ),
            SubcommandSpec(
                name="remotes",
                description=_("List configured remote repositories"),
                options=[
                    OptionSpec(["-d", "--show-details"], _("Show full details for remote repositories")),
                    OptionSpec(["--show-disabled"], _("Show disabled remote repositories as well")),
                    OptionSpec(["--user"], _("List user repositories")),
                    OptionSpec(["--system"], _("List system-wide repositories")),
                    OptionSpec(["--columns"], _("Specify columns to display"), takes_value=True, value_name="<columns>"),
                ],
                examples=[
                    "flatpak remotes",
                    "flatpak remotes -d",
                ],
            ),
            SubcommandSpec(
                name="remote-add",
                description=_("Add a remote repository"),
                options=[
                    OptionSpec(["--if-not-exists"], _("Do nothing if remote with same name already exists")),
                    OptionSpec(["--from"], _("Import remote configuration from .flatpakrepo file or URL"), takes_value=True, value_name="<location>"),
                    OptionSpec(["--user"], _("Add remote to user storage")),
                    OptionSpec(["--system"], _("Add remote to system-wide storage")),
                    OptionSpec(["--no-gpg-verify"], _("Disable GPG verification")),
                    OptionSpec(["--prio"], _("Set remote priority"), takes_value=True, value_name="<n>"),
                ],
                examples=[
                    "flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo",
                ],
            ),
            SubcommandSpec(
                name="remote-delete",
                description=_("Delete a remote repository"),
                options=[
                    OptionSpec(["--force"], _("Force deletion even if apps or runtimes depend on it")),
                    OptionSpec(["--user"], _("Delete from user storage")),
                    OptionSpec(["--system"], _("Delete from system-wide storage")),
                ],
                examples=[
                    "flatpak remote-delete flathub",
                ],
            ),
            SubcommandSpec(
                name="remote-modify",
                description=_("Modify properties of a remote repository"),
                options=[
                    OptionSpec(["--enable"], _("Enable the remote repository")),
                    OptionSpec(["--disable"], _("Disable the remote repository")),
                    OptionSpec(["--url"], _("Set new repository URL"), takes_value=True, value_name="<url>"),
                    OptionSpec(["--prio"], _("Set repository priority"), takes_value=True, value_name="<n>"),
                    OptionSpec(["--user"], _("Modify user storage remote")),
                    OptionSpec(["--system"], _("Modify system-wide storage remote")),
                ],
                examples=[
                    "flatpak remote-modify --disable flathub-beta",
                ],
            ),
            SubcommandSpec(
                name="remote-ls",
                description=_("List applications and runtimes available in a remote repository"),
                options=[
                    OptionSpec(["--app"], _("List only applications")),
                    OptionSpec(["--runtime"], _("List only runtimes")),
                    OptionSpec(["--user"], _("List from user remotes")),
                    OptionSpec(["--system"], _("List from system remotes")),
                    OptionSpec(["--columns"], _("Specify columns to display"), takes_value=True, value_name="<columns>"),
                ],
                examples=[
                    "flatpak remote-ls flathub --app",
                ],
            ),
            SubcommandSpec(
                name="ps",
                description=_("List running Flatpak application instances"),
                options=[
                    OptionSpec(["--columns"], _("Specify columns to display"), takes_value=True, value_name="<columns>"),
                    OptionSpec(["-v", "--verbose"], _("Print debug information")),
                ],
                examples=[
                    "flatpak ps",
                ],
            ),
            SubcommandSpec(
                name="kill",
                description=_("Stop a running Flatpak application instance"),
                examples=[
                    "flatpak kill org.gnome.Calculator",
                ],
            ),
            SubcommandSpec(
                name="override",
                description=_("Manage sandbox application permissions and overrides"),
                options=[
                    OptionSpec(["--filesystem"], _("Grant filesystem access permission"), takes_value=True, value_name="<path>"),
                    OptionSpec(["--nofilesystem"], _("Revoke filesystem access permission"), takes_value=True, value_name="<path>"),
                    OptionSpec(["--device"], _("Grant device access permission"), takes_value=True, value_name="<device>"),
                    OptionSpec(["--share"], _("Share system subsystem (network, ipc)"), takes_value=True, value_name="<subsys>"),
                    OptionSpec(["--unshare"], _("Unshare system subsystem"), takes_value=True, value_name="<subsys>"),
                    OptionSpec(["--socket"], _("Grant access to well-known socket"), takes_value=True, value_name="<socket>"),
                    OptionSpec(["--nosocket"], _("Revoke access to well-known socket"), takes_value=True, value_name="<socket>"),
                    OptionSpec(["--show"], _("Show current overrides")),
                    OptionSpec(["--reset"], _("Reset all overrides for the application")),
                    OptionSpec(["--user"], _("Apply override for current user")),
                    OptionSpec(["--system"], _("Apply override system-wide")),
                ],
                examples=[
                    "flatpak override --user --filesystem=host org.gnome.Calculator",
                    "flatpak override --show io.github.vagnarok.OnyxSH",
                ],
            ),
            SubcommandSpec(
                name="permission-show",
                description=_("Show permissions granted to an application"),
                examples=[
                    "flatpak permission-show org.gnome.Calculator",
                ],
            ),
            SubcommandSpec(
                name="permission-reset",
                description=_("Reset all permissions granted to an application"),
                examples=[
                    "flatpak permission-reset org.gnome.Calculator",
                ],
            ),
            SubcommandSpec(
                name="repair",
                description=_("Repair flatpak installations and ostree repositories"),
                options=[
                    OptionSpec(["--dry-run"], _("Only print issues without modifying repository")),
                    OptionSpec(["--user"], _("Repair user installation")),
                    OptionSpec(["--system"], _("Repair system installation")),
                    OptionSpec(["-v", "--verbose"], _("Verbose output")),
                ],
                examples=[
                    "flatpak repair",
                    "flatpak repair --user",
                ],
            ),
            SubcommandSpec(
                name="history",
                description=_("Show transaction history of installations and changes"),
                options=[
                    OptionSpec(["--user"], _("Show user history")),
                    OptionSpec(["--system"], _("Show system history")),
                    OptionSpec(["--columns"], _("Specify columns to display"), takes_value=True, value_name="<columns>"),
                ],
                examples=[
                    "flatpak history",
                ],
            ),
            SubcommandSpec(
                name="config",
                description=_("Manage Flatpak configuration settings"),
                options=[
                    OptionSpec(["--list"], _("List all configuration keys and values")),
                    OptionSpec(["--set"], _("Set configuration key"), takes_value=True, value_name="<key=val>"),
                    OptionSpec(["--unset"], _("Unset configuration key"), takes_value=True, value_name="<key>"),
                    OptionSpec(["--user"], _("Work on user configuration")),
                    OptionSpec(["--system"], _("Work on system-wide configuration")),
                ],
                examples=[
                    "flatpak config --list",
                ],
            ),
        ],
        global_options=[
            OptionSpec(["-h", "--help"], _("Show help information")),
            OptionSpec(["--version"], _("Show version number")),
            OptionSpec(["-v", "--verbose"], _("Print debug information")),
            OptionSpec(["-y", "--assumeyes"], _("Automatically answer yes to all questions")),
            OptionSpec(["--noninteractive"], _("Produce minimal output and never ask questions")),
            OptionSpec(["--user"], _("Operate on user installation")),
            OptionSpec(["--system"], _("Operate on system-wide installation")),
            OptionSpec(["--installation"], _("Operate on specific named installation"), takes_value=True, value_name="<name>"),
            OptionSpec(["--arch"], _("Specify target architecture"), takes_value=True, value_name="<arch>"),
        ],
    )
