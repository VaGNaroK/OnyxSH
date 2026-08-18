# onyxsh/terminal/completion/specs/common.py
"""
Completion specifications for essential Linux coreutils and process tools.
"""

from typing import List

from ....utils.translation_utils import _
from .base import CommandSpec, OptionSpec, SubcommandSpec


def get_common_specs() -> List[CommandSpec]:
    """Builds and returns command specs for standard Linux utilities."""
    return [
        CommandSpec(
            name="sudo",
            description=_("Execute a command as the superuser or another user"),
            global_options=[
                OptionSpec(["-i", "--login"], _("Run login shell as the target user (root)")),
                OptionSpec(["-s", "--shell"], _("Run shell as the target user")),
                OptionSpec(["-u", "--user"], _("Run command as specified user instead of root"), takes_value=True, value_name="<user>"),
                OptionSpec(["-E", "--preserve-env"], _("Preserve existing environment variables")),
                OptionSpec(["-v", "--validate"], _("Update user's cached credentials without running a command")),
                OptionSpec(["-k", "--reset-timestamp"], _("Invalidate user's cached credentials")),
            ],
        ),
        CommandSpec(
            name="chmod",
            description=_("Change file mode bits (permissions)"),
            global_options=[
                OptionSpec(["-R", "--recursive"], _("Change files and directories recursively")),
                OptionSpec(["-v", "--verbose"], _("Output a diagnostic for every file processed")),
                OptionSpec(["+x"], _("Make file executable")),
                OptionSpec(["755"], _("rwxr-xr-x (standard directory / executable permissions)")),
                OptionSpec(["644"], _("rw-r--r-- (standard file permissions)")),
                OptionSpec(["600"], _("rw------- (secure private key permissions)")),
            ],
        ),
        CommandSpec(
            name="chown",
            description=_("Change file owner and group"),
            global_options=[
                OptionSpec(["-R", "--recursive"], _("Operate on files and directories recursively")),
                OptionSpec(["-v", "--verbose"], _("Output a diagnostic for every file processed")),
            ],
        ),
        CommandSpec(
            name="mkdir",
            description=_("Create directories if they do not already exist"),
            global_options=[
                OptionSpec(["-p", "--parents"], _("Make parent directories as needed, no error if existing")),
                OptionSpec(["-v", "--verbose"], _("Print a message for each created directory")),
            ],
        ),
        CommandSpec(
            name="rm",
            description=_("Remove files or directories"),
            global_options=[
                OptionSpec(["-r", "-R", "--recursive"], _("Remove directories and their contents recursively")),
                OptionSpec(["-f", "--force"], _("Ignore nonexistent files and arguments, never prompt")),
                OptionSpec(["-i"], _("Prompt before every removal")),
                OptionSpec(["-v", "--verbose"], _("Explain what is being done")),
            ],
        ),
        CommandSpec(
            name="ls",
            description=_("List directory contents"),
            global_options=[
                OptionSpec(["-l"], _("Use a long listing format with permissions, owner, size and date")),
                OptionSpec(["-a", "--all"], _("Do not ignore entries starting with .")),
                OptionSpec(["-h", "--human-readable"], _("With -l, print sizes in human readable format (e.g., 1K 234M 2G)")),
                OptionSpec(["-t"], _("Sort by modification time, newest first")),
                OptionSpec(["-r", "--reverse"], _("Reverse order while sorting")),
                OptionSpec(["-la", "-lah", "-laht"], _("Common combination: long format, all files, human sizes")),
            ],
        ),
        CommandSpec(
            name="grep",
            description=_("Print lines that match patterns"),
            global_options=[
                OptionSpec(["-i", "--ignore-case"], _("Ignore case distinctions in patterns and input data")),
                OptionSpec(["-r", "-R", "--recursive"], _("Read all files under each directory, recursively")),
                OptionSpec(["-n", "--line-number"], _("Prefix each line of output with the 1-based line number")),
                OptionSpec(["-v", "--invert-match"], _("Invert the sense of matching, to select non-matching lines")),
                OptionSpec(["-l", "--files-with-matches"], _("Suppress normal output; instead print the name of each input file")),
                OptionSpec(["-c", "--count"], _("Suppress normal output; instead print a count of matching lines")),
                OptionSpec(["-E", "--extended-regexp"], _("Interpret PATTERNS as extended regular expressions")),
            ],
        ),
        CommandSpec(
            name="find",
            description=_("Search for files in a directory hierarchy"),
            global_options=[
                OptionSpec(["-name"], _("Match base of file name against shell pattern"), takes_value=True, value_name="<pattern>"),
                OptionSpec(["-iname"], _("Like -name, but match is case insensitive"), takes_value=True, value_name="<pattern>"),
                OptionSpec(["-type"], _("File type (f for regular file, d for directory, l for symlink)"), takes_value=True, value_name="<f|d|l>"),
                OptionSpec(["-mtime"], _("File data was last modified n*24 hours ago"), takes_value=True),
                OptionSpec(["-size"], _("File uses n units of space (e.g. +100M)"), takes_value=True),
                OptionSpec(["-exec"], _("Execute command on matched files"), takes_value=True),
                OptionSpec(["-delete"], _("Delete found files")),
            ],
        ),
        CommandSpec(
            name="kill",
            description=_("Send a signal to a process (terminate/kill)"),
            global_options=[
                OptionSpec(["-9", "-KILL"], _("SIGKILL: Force kill immediately (uninterceptable)")),
                OptionSpec(["-15", "-TERM"], _("SIGTERM: Graceful termination request (default)")),
                OptionSpec(["-HUP", "-1"], _("SIGHUP: Hangup / reload configuration")),
                OptionSpec(["-l"], _("List all signal names")),
            ],
        ),
        CommandSpec(
            name="pkill",
            description=_("Look up or signal processes based on name and other attributes"),
            global_options=[
                OptionSpec(["-f", "--full"], _("The pattern is matched against the full command line")),
                OptionSpec(["-9", "--signal KILL"], _("Force kill matching processes")),
                OptionSpec(["-u"], _("Only match processes whose effective user ID matches"), takes_value=True, value_name="<user>"),
            ],
        ),
    ]
