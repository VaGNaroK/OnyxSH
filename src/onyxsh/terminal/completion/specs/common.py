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
            name="cd",
            description=_("Change the current working directory"),
            global_options=[
                OptionSpec([".."], _("Parent directory (go up one level)")),
                OptionSpec(["~"], _("User home directory")),
                OptionSpec(["-"], _("Previous working directory")),
                OptionSpec(["/tmp"], _("Temporary directory")),
                OptionSpec(["/etc"], _("System configuration directory")),
                OptionSpec(["/var/log"], _("System logs directory")),
            ],
        ),
        CommandSpec(
            name="pwd",
            description=_("Print the current working directory name"),
            global_options=[
                OptionSpec(["-P"], _("Avoid all symlinks and print physical directory")),
                OptionSpec(["-L"], _("Print the environment PWD including symlinks")),
            ],
        ),
        CommandSpec(
            name="cp",
            description=_("Copy files and directories"),
            global_options=[
                OptionSpec(["-r", "-R", "--recursive"], _("Copy directories recursively")),
                OptionSpec(["-v", "--verbose"], _("Explain what is being done")),
                OptionSpec(["-i", "--interactive"], _("Prompt before overwrite")),
                OptionSpec(["-a", "--archive"], _("Archive mode: preserve attributes, symlinks, recursive")),
                OptionSpec(["-u", "--update"], _("Copy only when source file is newer")),
                OptionSpec(["-p"], _("Preserve specified attributes (mode,ownership,timestamps)")),
            ],
        ),
        CommandSpec(
            name="mv",
            description=_("Move or rename files and directories"),
            global_options=[
                OptionSpec(["-f", "--force"], _("Do not prompt before overwriting")),
                OptionSpec(["-i", "--interactive"], _("Prompt before overwrite")),
                OptionSpec(["-v", "--verbose"], _("Explain what is being done")),
                OptionSpec(["-u", "--update"], _("Move only when source file is newer")),
            ],
        ),
        CommandSpec(
            name="touch",
            description=_("Change file timestamps or create empty files"),
            global_options=[
                OptionSpec(["-a"], _("Change only the access time")),
                OptionSpec(["-m"], _("Change only the modification time")),
                OptionSpec(["-c", "--no-create"], _("Do not create any files")),
            ],
        ),
        CommandSpec(
            name="cat",
            description=_("Concatenate files and print on the standard output"),
            global_options=[
                OptionSpec(["-n", "--number"], _("Number all output lines")),
                OptionSpec(["-b", "--number-nonblank"], _("Number nonempty output lines, overrides -n")),
                OptionSpec(["-s", "--squeeze-blank"], _("Suppress repeated empty output lines")),
                OptionSpec(["-A", "--show-all"], _("Equivalent to -vET (show non-printing characters)")),
            ],
        ),
        CommandSpec(
            name="less",
            description=_("View file contents with backwards and forwards navigation"),
            global_options=[
                OptionSpec(["-N", "--LINE-NUMBERS"], _("Show line numbers")),
                OptionSpec(["-S", "--chop-long-lines"], _("Chop long lines instead of wrapping")),
                OptionSpec(["-F", "--quit-if-one-screen"], _("Quit if the entire file fits on the first screen")),
                OptionSpec(["-R", "--RAW-CONTROL-CHARS"], _("Output raw control characters (ANSI colors)")),
            ],
        ),
        CommandSpec(
            name="head",
            description=_("Output the first part of files"),
            global_options=[
                OptionSpec(["-n 10"], _("Print the first 10 lines")),
                OptionSpec(["-n 20"], _("Print the first 20 lines")),
                OptionSpec(["-n 50"], _("Print the first 50 lines")),
                OptionSpec(["-c"], _("Print the first K bytes of each file")),
            ],
        ),
        CommandSpec(
            name="tail",
            description=_("Output the last part of files"),
            global_options=[
                OptionSpec(["-f", "--follow"], _("Follow output live (output appended data as file grows)")),
                OptionSpec(["-n 50"], _("Output the last 50 lines")),
                OptionSpec(["-n 100"], _("Output the last 100 lines")),
                OptionSpec(["-F"], _("Follow and retry if file is rotated or recreated")),
            ],
        ),
        CommandSpec(
            name="clear",
            description=_("Clear the terminal screen"),
            global_options=[
                OptionSpec(["-x"], _("Do not clear scrollback buffer")),
            ],
        ),
        CommandSpec(
            name="tree",
            description=_("List contents of directories in a tree-like format"),
            global_options=[
                OptionSpec(["-L 2"], _("Max display depth of the directory tree: 2")),
                OptionSpec(["-L 3"], _("Max display depth of the directory tree: 3")),
                OptionSpec(["-a"], _("All files are printed, including hidden files")),
                OptionSpec(["-d"], _("List directories only")),
                OptionSpec(["-h"], _("Print the size in human readable format")),
            ],
        ),
        CommandSpec(
            name="df",
            description=_("Report file system disk space usage"),
            global_options=[
                OptionSpec(["-h", "--human-readable"], _("Print sizes in powers of 1024 (e.g., 1K 234M 2G)")),
                OptionSpec(["-T", "--print-type"], _("Print file system type")),
                OptionSpec(["-i", "--inodes"], _("List inode information instead of block usage")),
            ],
        ),
        CommandSpec(
            name="du",
            description=_("Estimate file and directory space usage"),
            global_options=[
                OptionSpec(["-sh *"], _("Summary of size for each item in current directory")),
                OptionSpec(["-h", "--human-readable"], _("Print sizes in human readable format")),
                OptionSpec(["-d 1", "--max-depth=1"], _("Print the total for directory depth 1")),
                OptionSpec(["-a", "--all"], _("Write counts for all files, not just directories")),
            ],
        ),
        CommandSpec(
            name="free",
            description=_("Display amount of free and used memory in the system"),
            global_options=[
                OptionSpec(["-h", "--human"], _("Display memory in human-readable format")),
                OptionSpec(["-m", "--mebi"], _("Display amount of memory in megabytes")),
                OptionSpec(["-g", "--gibi"], _("Display amount of memory in gigabytes")),
                OptionSpec(["-s 2"], _("Continuously display memory every 2 seconds")),
            ],
        ),
        CommandSpec(
            name="htop",
            description=_("Interactive process viewer and system monitor"),
        ),
        CommandSpec(
            name="top",
            description=_("Display Linux processes in real time"),
            global_options=[
                OptionSpec(["-b"], _("Batch mode operation")),
                OptionSpec(["-n 1"], _("Number of iterations before exiting")),
                OptionSpec(["-u"], _("Monitor only specific user processes")),
            ],
        ),
        CommandSpec(
            name="ps",
            description=_("Report a snapshot of the current processes"),
            global_options=[
                OptionSpec(["aux"], _("See every process on the system (BSD style)")),
                OptionSpec(["-ef"], _("Standard full format listing of all processes")),
                OptionSpec(["-u"], _("Display user-oriented format")),
                OptionSpec(["--forest"], _("ASCII art process tree")),
            ],
        ),
        CommandSpec(
            name="which",
            description=_("Locate a command executable in PATH"),
            global_options=[
                OptionSpec(["-a"], _("Print all matching pathnames of each argument")),
            ],
        ),
        CommandSpec(
            name="whereis",
            description=_("Locate the binary, source, and manual page files for a command"),
        ),
        CommandSpec(
            name="echo",
            description=_("Display a line of text or variable value"),
            global_options=[
                OptionSpec(["-e"], _("Enable interpretation of backslash escapes")),
                OptionSpec(["-n"], _("Do not output the trailing newline")),
            ],
        ),
        CommandSpec(
            name="nano",
            description=_("Simple, user-friendly terminal text editor"),
            global_options=[
                OptionSpec(["-l", "--linenumbers"], _("Show line numbers in editor")),
                OptionSpec(["-m", "--mouse"], _("Enable mouse support")),
            ],
        ),
        CommandSpec(
            name="vim",
            description=_("Vi IMproved, a highly configurable text editor"),
            global_options=[
                OptionSpec(["+"], _("Open at the end of the file")),
                OptionSpec(["-R"], _("Read-only mode")),
            ],
        ),
        CommandSpec(
            name="ln",
            description=_("Make links between files"),
            global_options=[
                OptionSpec(["-s", "--symbolic"], _("Make symbolic (soft) links instead of hard links")),
                OptionSpec(["-f", "--force"], _("Remove existing destination files")),
                OptionSpec(["-v", "--verbose"], _("Print name of each linked file")),
            ],
        ),
        CommandSpec(
            name="uname",
            description=_("Print operating system and kernel information"),
            global_options=[
                OptionSpec(["-a", "--all"], _("Print all system information")),
                OptionSpec(["-r", "--kernel-release"], _("Print the kernel release")),
                OptionSpec(["-m", "--machine"], _("Print the machine hardware architecture")),
            ],
        ),
        CommandSpec(
            name="history",
            description=_("Display or manipulate the command history list"),
            global_options=[
                OptionSpec(["-c"], _("Clear the history list")),
                OptionSpec(["10"], _("Show last 10 commands")),
            ],
        ),
        CommandSpec(
            name="ping",
            description=_("Send ICMP ECHO_REQUEST to network hosts"),
            global_options=[
                OptionSpec(["-c 4"], _("Stop after sending 4 ECHO_REQUEST packets")),
                OptionSpec(["-i 1"], _("Wait 1 second between sending each packet")),
                OptionSpec(["-W 2"], _("Time to wait for a response in seconds")),
            ],
        ),
        CommandSpec(
            name="wget",
            description=_("Non-interactive network downloader"),
            global_options=[
                OptionSpec(["-c", "--continue"], _("Resume getting a partially-downloaded file")),
                OptionSpec(["-O"], _("Write output to specific file"), takes_value=True),
                OptionSpec(["-q", "--quiet"], _("Quiet mode (turn off output)")),
                OptionSpec(["-b", "--background"], _("Go to background immediately after startup")),
            ],
        ),
        CommandSpec(
            name="scp",
            description=_("Secure copy files between hosts over SSH"),
            global_options=[
                OptionSpec(["-r"], _("Recursively copy entire directories")),
                OptionSpec(["-P"], _("Port to connect to on the remote host"), takes_value=True),
                OptionSpec(["-v"], _("Verbose mode")),
                OptionSpec(["-C"], _("Enable compression")),
            ],
        ),
        CommandSpec(
            name="zip",
            description=_("Package and compress files into a zip archive"),
            global_options=[
                OptionSpec(["-r"], _("Travel the directory structure recursively")),
                OptionSpec(["-e"], _("Encrypt contents with password")),
            ],
        ),
        CommandSpec(
            name="unzip",
            description=_("Extract compressed files from a ZIP archive"),
            global_options=[
                OptionSpec(["-l"], _("List archive files without extracting")),
                OptionSpec(["-d"], _("Extract files into specified directory"), takes_value=True),
                OptionSpec(["-q"], _("Quiet mode")),
            ],
        ),
        CommandSpec(
            name="gzip",
            description=_("Compress or expand files with Lempel-Ziv coding (LZ77)"),
            global_options=[
                OptionSpec(["-d", "--decompress"], _("Decompress / expand files")),
                OptionSpec(["-k", "--keep"], _("Keep (do not delete) input files")),
                OptionSpec(["-9", "--best"], _("Best compression level")),
            ],
        ),
        CommandSpec(
            name="gunzip",
            description=_("Decompress and expand files compressed with gzip"),
            global_options=[
                OptionSpec(["-k", "--keep"], _("Keep (do not delete) input files")),
            ],
        ),
    ]
