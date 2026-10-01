# onyxsh/terminal/completion/specs/dnf.py
"""
Completion specifications for DNF package manager family (dnf, yum, microdnf).
Used primarily in Fedora, Red Hat Enterprise Linux, CentOS, Rocky Linux, and AlmaLinux.
"""

from ....utils.translation_utils import _
from .base import CommandSpec, OptionSpec, SubcommandSpec


def get_dnf_spec() -> CommandSpec:
    """Builds and returns the DNF/YUM command spec."""
    return CommandSpec(
        name="dnf",
        description=_("Fedora/RHEL package manager utility"),
        aliases=["yum", "microdnf"],
        subcommands=[
            SubcommandSpec(
                name="install",
                description=_("Install one or more packages"),
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatic yes to all prompts")),
                    OptionSpec(["--allowerasing"], _("Allow erasing of installed packages to resolve dependencies")),
                    OptionSpec(["--skip-broken"], _("Skip packages with broken dependencies")),
                    OptionSpec(["--nobest"], _("Do not limit transaction to best available package versions")),
                    OptionSpec(["--setopt"], _("Override repository or DNF configuration options"), takes_value=True),
                    OptionSpec(["--refresh"], _("Refresh metadata before running the command")),
                    OptionSpec(["--enablerepo"], _("Enable additional repositories"), takes_value=True, value_name="<repoid>"),
                    OptionSpec(["--disablerepo"], _("Disable repositories"), takes_value=True, value_name="<repoid>"),
                    OptionSpec(["--releasever"], _("Override the $releasever variable"), takes_value=True, value_name="<version>"),
                ],
                examples=["dnf install curl git", "dnf install -y nginx", "dnf install --allowerasing podman"],
            ),
            SubcommandSpec(
                name="remove",
                description=_("Remove one or more packages and their unused dependencies"),
                aliases=["erase"],
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatic yes to all prompts")),
                    OptionSpec(["--noautoremove"], _("Do not auto-remove unused dependencies")),
                    OptionSpec(["--duplicates"], _("Remove older duplicates of packages")),
                ],
                examples=["dnf remove nginx"],
            ),
            SubcommandSpec(
                name="reinstall",
                description=_("Reinstall packages even if already installed"),
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatically answer yes to all questions")),
                ],
                examples=["dnf reinstall vim"],
            ),
            SubcommandSpec(
                name="upgrade",
                description=_("Upgrade all installed packages"),
                aliases=["update"],
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatic yes to all prompts")),
                    OptionSpec(["--refresh"], _("Refresh metadata before upgrading")),
                    OptionSpec(["--allowerasing"], _("Allow erasing of installed packages to resolve dependencies")),
                    OptionSpec(["--skip-broken"], _("Skip packages with broken dependencies")),
                ],
                examples=["dnf upgrade", "dnf upgrade --refresh"],
            ),
            SubcommandSpec(
                name="check-update",
                description=_("Check for available package updates"),
                options=[
                    OptionSpec(["--changelogs"], _("Show package changelogs")),
                    OptionSpec(["--refresh"], _("Refresh metadata before checking")),
                ],
                examples=["dnf check-update"],
            ),
            SubcommandSpec(
                name="search",
                description=_("Search for packages by name or description"),
                options=[
                    OptionSpec(["--all"], _("Search in package names and descriptions")),
                    OptionSpec(["--exact"], _("Search for exact package name match")),
                ],
                examples=["dnf search curl", "dnf search --all 'web server'"],
            ),
            SubcommandSpec(
                name="info",
                description=_("Display detailed information about packages"),
                options=[
                    OptionSpec(["--installed"], _("Show information for installed packages only")),
                    OptionSpec(["--showduplicates"], _("Show duplicate packages from all repositories"]),
                ],
                examples=["dnf info curl", "dnf info --installed nginx"],
            ),
            SubcommandSpec(
                name="list",
                description=_("List packages matching the given criteria"),
                options=[
                    OptionSpec(["--installed"], _("List installed packages")),
                    OptionSpec(["--available"], _("List available packages in repositories")),
                    OptionSpec(["--updates"], _("List packages that have updates available")),
                    OptionSpec(["--obsoletes"], _("List obsolete packages")),
                ],
                examples=["dnf list installed", "dnf list updates"],
            ),
            SubcommandSpec(
                name="provides",
                description=_("Find which package provides a given file or feature"),
                aliases=["whatprovides"],
                examples=["dnf provides /usr/bin/curl", "dnf provides '*/vim'"],
            ),
            SubcommandSpec(
                name="repolist",
                description=_("List enabled or disabled repositories"),
                options=[
                    OptionSpec(["--enabled"], _("List only enabled repositories")),
                    OptionSpec(["--disabled"], _("List only disabled repositories")),
                    OptionSpec(["--all"], _("List all repositories")),
                ],
                examples=["dnf repolist", "dnf repolist --all"],
            ),
            SubcommandSpec(
                name="group",
                description=_("Manage package groups"),
                aliases=["groups"],
                options=[
                    OptionSpec(["list"], _("List available groups")),
                    OptionSpec(["info"], _("Show information about a group"), takes_value=True, value_name="<group>"),
                    OptionSpec(["install"], _("Install a group"), takes_value=True, value_name="<group>"),
                    OptionSpec(["remove"], _("Remove a group"), takes_value=True, value_name="<group>"),
                    OptionSpec(["update"], _("Update a group"), takes_value=True, value_name="<group>"),
                ],
                examples=["dnf group list", "dnf group install 'Development Tools'"],
            ),
            SubcommandSpec(
                name="autoremove",
                description=_("Remove packages that are no longer needed"),
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatic yes to all prompts")),
                ],
                examples=["dnf autoremove"],
            ),
            SubcommandSpec(
                name="clean",
                description=_("Remove cached package data"),
                options=[
                    OptionSpec(["all"], _("Remove all cached files")),
                    OptionSpec(["metadata"], _("Remove only repository metadata")),
                    OptionSpec(["packages"], _("Remove downloaded packages")),
                    OptionSpec(["dbcache"], _("Remove cached DNF database files")),
                ],
                examples=["dnf clean all"],
            ),
            SubcommandSpec(
                name="makecache",
                description=_("Generate or refresh the local package cache"),
                options=[
                    OptionSpec(["--refresh"], _("Force a full refresh of all repositories")),
                    OptionSpec(["--timer"], _("Use the cache timer mechanism to refresh")),
                ],
                examples=["dnf makecache", "dnf makecache --refresh"],
            ),
            SubcommandSpec(
                name="downgrade",
                description=_("Downgrade specified packages to an older version"),
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatic yes to all prompts")),
                ],
                examples=["dnf downgrade curl"],
            ),
            SubcommandSpec(
                name="download",
                description=_("Download packages to the current directory without installing"),
                options=[
                    OptionSpec(["--destdir"], _("Set destination directory for downloaded packages"), takes_value=True, value_name="<dir>"),
                    OptionSpec(["--resolve"], _("Download needed dependencies as well")),
                    OptionSpec(["--source"], _("Download source RPM (SRPM)")),
                ],
                examples=["dnf download --destdir=/tmp nginx", "dnf download --resolve git"],
            ),
            SubcommandSpec(
                name="groupinstall",
                description=_("Install a software group by name"),
                aliases=["group install"],
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatic yes to all prompts")),
                    OptionSpec(["--with-optional"], _("Include optional packages in the group")),
                ],
                examples=["dnf groupinstall 'Development Tools'"],
            ),
            SubcommandSpec(
                name="history",
                description=_("View and manage DNF transaction history"),
                options=[
                    OptionSpec(["list"], _("List recent transactions")),
                    OptionSpec(["info"], _("Show details of a specific transaction"), takes_value=True, value_name="<id>"),
                    OptionSpec(["undo"], _("Undo a specific transaction"), takes_value=True, value_name="<id>"),
                    OptionSpec(["redo"], _("Redo a specific transaction"), takes_value=True, value_name="<id>"),
                    OptionSpec(["rollback"], _("Roll back to a specific transaction state"), takes_value=True, value_name="<id>"),
                ],
                examples=["dnf history list", "dnf history info last", "dnf history undo last"],
            ),
            SubcommandSpec(
                name="repoquery",
                description=_("Search for packages matching various repository criteria"),
                options=[
                    OptionSpec(["--requires"], _("List packages required by the specified package")),
                    OptionSpec(["--whatprovides"], _("Query packages providing the specified capability"), takes_value=True),
                    OptionSpec(["--whatrequires"], _("Query packages requiring the specified capability"), takes_value=True),
                    OptionSpec(["--installed"], _("Query installed packages only")),
                ],
                examples=["dnf repoquery --whatprovides /usr/bin/python3"],
            ),
        ],
        global_options=[
            OptionSpec(["-h", "--help"], _("Show help information")),
            OptionSpec(["--version"], _("Show version number")),
            OptionSpec(["-y", "--assumeyes"], _("Assume yes to all queries")),
            OptionSpec(["-q", "--quiet"], _("Produce quiet output")),
            OptionSpec(["-v", "--verbose"], _("Produce verbose output")),
            OptionSpec(["--refresh"], _("Set metadata as expired before running the command")),
            OptionSpec(["--enablerepo"], _("Enable additional repository"), takes_value=True, value_name="<repoid>"),
            OptionSpec(["--disablerepo"], _("Disable specified repository"), takes_value=True, value_name="<repoid>"),
            OptionSpec(["--color"], _("Control colored output (auto, never, always)"), takes_value=True),
        ],
    )
