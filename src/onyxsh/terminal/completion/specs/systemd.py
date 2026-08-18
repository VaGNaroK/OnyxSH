# onyxsh/terminal/completion/specs/systemd.py
"""
Completion specifications for Systemd service manager and journalctl.
"""

from ....utils.translation_utils import _
from .base import CommandSpec, OptionSpec, SubcommandSpec


def get_systemctl_spec() -> CommandSpec:
    """Builds and returns the systemctl command spec."""
    return CommandSpec(
        name="systemctl",
        description=_("Control the systemd system and service manager"),
        subcommands=[
            SubcommandSpec(
                name="status",
                description=_("Show runtime status information about units/services"),
                options=[
                    OptionSpec(["-l", "--full"], _("Do not ellipsize output")),
                    OptionSpec(["--no-pager"], _("Do not pipe output into a pager")),
                ],
                examples=["systemctl status nginx", "systemctl status sshd"],
            ),
            SubcommandSpec(
                name="start",
                description=_("Start (activate) one or more units"),
                examples=["systemctl start docker", "systemctl start postgresql"],
            ),
            SubcommandSpec(
                name="stop",
                description=_("Stop (deactivate) one or more units"),
                examples=["systemctl stop apache2"],
            ),
            SubcommandSpec(
                name="restart",
                description=_("Stop and then start one or more units"),
                examples=["systemctl restart networking", "systemctl restart nginx"],
            ),
            SubcommandSpec(
                name="reload",
                description=_("Reload configuration without dropping connections"),
                examples=["systemctl reload nginx"],
            ),
            SubcommandSpec(
                name="reload-or-restart",
                description=_("Reload service or restart if reload is not supported"),
            ),
            SubcommandSpec(
                name="enable",
                description=_("Enable one or more units to start automatically at boot"),
                options=[
                    OptionSpec(["--now"], _("Enable and start unit immediately")),
                ],
                examples=["systemctl enable --now docker", "systemctl enable ssh"],
            ),
            SubcommandSpec(
                name="disable",
                description=_("Disable one or more units from starting at boot"),
                options=[
                    OptionSpec(["--now"], _("Disable and stop unit immediately")),
                ],
                examples=["systemctl disable apache2"],
            ),
            SubcommandSpec(
                name="is-active",
                description=_("Check whether units are active (running)"),
                examples=["systemctl is-active docker"],
            ),
            SubcommandSpec(
                name="is-enabled",
                description=_("Check whether units are enabled to start on boot"),
            ),
            SubcommandSpec(
                name="daemon-reload",
                description=_("Reload systemd manager configuration and unit files"),
                examples=["systemctl daemon-reload"],
            ),
            SubcommandSpec(
                name="list-units",
                description=_("List currently active units in memory"),
                options=[
                    OptionSpec(["--type"], _("Filter by unit type (service, socket, timer)"), takes_value=True),
                    OptionSpec(["--state"], _("Filter by unit state (active, failed, inactive)"), takes_value=True),
                ],
            ),
            SubcommandSpec(
                name="list-unit-files",
                description=_("List installed unit files and their enabled/disabled status"),
            ),
            SubcommandSpec(
                name="mask",
                description=_("Mask one or more units completely preventing activation"),
            ),
            SubcommandSpec(
                name="unmask",
                description=_("Unmask units allowing activation again"),
            ),
        ],
        global_options=[
            OptionSpec(["--user"], _("Talk to the service manager of the calling user")),
            OptionSpec(["--system"], _("Talk to the system service manager (default)")),
            OptionSpec(["--no-pager"], _("Do not pipe output into a pager")),
            OptionSpec(["-h", "--help"], _("Show help information")),
        ],
    )


def get_journalctl_spec() -> CommandSpec:
    """Builds and returns the journalctl command spec."""
    return CommandSpec(
        name="journalctl",
        description=_("Query the systemd journal logs"),
        subcommands=[],
        global_options=[
            OptionSpec(["-u", "--unit"], _("Show logs for the specified service unit"), takes_value=True, value_name="<unit>"),
            OptionSpec(["-f", "--follow"], _("Follow journal output live (real-time stream)")),
            OptionSpec(["-n", "--lines"], _("Number of journal entries to show"), takes_value=True, value_name="<N>"),
            OptionSpec(["-b", "--boot"], _("Show messages from the current or specified boot")),
            OptionSpec(["-e", "--pager-end"], _("Jump to the end of the journal immediately")),
            OptionSpec(["-k", "--dmesg"], _("Show kernel messages only (like dmesg)")),
            OptionSpec(["--since"], _("Filter entries since date/time or relative timestamp"), takes_value=True, value_name="<time>"),
            OptionSpec(["--until"], _("Filter entries until date/time or relative timestamp"), takes_value=True, value_name="<time>"),
            OptionSpec(["-p", "--priority"], _("Filter by message priority (err, warning, info, debug)"), takes_value=True, value_name="<prio>"),
            OptionSpec(["-r", "--reverse"], _("Show newest entries first")),
            OptionSpec(["--no-pager"], _("Do not pipe output into a pager")),
            OptionSpec(["-x", "--catalog"], _("Show explanatory help texts from message catalog")),
        ],
    )
