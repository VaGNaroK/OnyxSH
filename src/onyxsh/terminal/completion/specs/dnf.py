# onyxsh/terminal/completion/specs/dnf.py
"""
Completion specifications for DNF package manager family (dnf, yum).
Used primarily in Fedora, Red Hat Enterprise Linux, CentOS, Rocky Linux, and AlmaLinux.
"""

from ....utils.translation_utils import _
from .base import CommandSpec, OptionSpec, SubcommandSpec


def get_dnf_spec() -> CommandSpec:
    """Builds and returns the DNF/YUM command spec."""
    return CommandSpec(
        name="dnf",
        description=_("Fedora/RHEL package manager utility"),
        aliases=["yum"],
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
                ],
                examples=["dnf install curl git", "dnf install -y nginx", "dnf install --allowerasing podman"],
            ),
            SubcommandSpec(
                name="remove",
                description=_("Remove one or more packages and their unused dependencies"),
                aliases=["erase"],
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatic yes to all prompts")),
                    OptionSpec(["--duplicates"], _("Remove older duplicates of packages")),
                ],
                examples=["dnf remove nginx", "dnf remove -y httpd"],
            ),
            SubcommandSpec(
                name="upgrade",
                description=_("Upgrade all installed packages to newest versions"),
                aliases=["update"],
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatic yes to all prompts")),
                    OptionSpec(["--security"], _("Include only packages providing security fixes")),
                    OptionSpec(["--minimal"], _("Upgrade only to the lowest version containing fixes")),
                    OptionSpec(["--refresh"], _("Force metadata refresh before upgrade")),
                    OptionSpec(["--allowerasing"], _("Allow erasing of installed packages to resolve dependencies")),
                ],
                examples=["dnf upgrade -y", "dnf upgrade --refresh", "dnf upgrade --security"],
            ),
            SubcommandSpec(
                name="check-update",
                description=_("Check for available package upgrades without installing them"),
                options=[
                    OptionSpec(["--security"], _("Check only for security updates")),
                    OptionSpec(["--refresh"], _("Force metadata refresh before checking")),
                ],
                examples=["dnf check-update", "dnf check-update --security"],
            ),
            SubcommandSpec(
                name="search",
                description=_("Search package names and summaries for keywords"),
                options=[
                    OptionSpec(["--all"], _("Search descriptions as well as names and summaries")),
                ],
                examples=["dnf search python3", "dnf search ripgrep"],
            ),
            SubcommandSpec(
                name="info",
                description=_("Display detailed package information and metadata"),
                options=[
                    OptionSpec(["--installed"], _("Show information for installed packages only")),
                    OptionSpec(["--available"], _("Show information for available packages only")),
                ],
                examples=["dnf info podman", "dnf info --installed kernel"],
            ),
            SubcommandSpec(
                name="autoremove",
                description=_("Remove packages originally installed as dependencies that are no longer needed"),
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatic yes to all prompts")),
                ],
                examples=["dnf autoremove -y"],
            ),
            SubcommandSpec(
                name="clean",
                description=_("Remove cached metadata and downloaded packages"),
                options=[
                    OptionSpec(["all"], _("Clean all cached metadata and packages")),
                    OptionSpec(["metadata"], _("Clean cached repository metadata")),
                    OptionSpec(["packages"], _("Clean cached RPM packages")),
                    OptionSpec(["dbcache"], _("Clean cached metadata databases")),
                ],
                examples=["dnf clean all", "dnf clean metadata"],
            ),
            SubcommandSpec(
                name="reinstall",
                description=_("Reinstall one or more packages"),
                options=[
                    OptionSpec(["-y", "--assumeyes"], _("Automatic yes to all prompts")),
                ],
                examples=["dnf reinstall kernel-core"],
            ),
            SubcommandSpec(
                name="download",
                description=_("Download package RPMs without installing them"),
                options=[
                    OptionSpec(["--destdir"], _("Directory to download packages into"), takes_value=True, value_name="<dir>"),
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
