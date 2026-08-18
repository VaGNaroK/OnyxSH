# onyxsh/terminal/completion/specs/devops.py
"""
Completion specifications for DevOps, networking and system utilities (ssh, curl, tar, ufw, ip, ping, rsync, ss).
"""

from typing import List

from ....utils.translation_utils import _
from .base import CommandSpec, OptionSpec, SubcommandSpec


def get_devops_specs() -> List[CommandSpec]:
    """Builds and returns command specs for networking and DevOps tools."""
    return [
        CommandSpec(
            name="ssh",
            description=_("OpenSSH remote login client"),
            global_options=[
                OptionSpec(["-p"], _("Port to connect to on the remote host"), takes_value=True, value_name="<port>"),
                OptionSpec(["-i"], _("Identity file (private key) for public key authentication"), takes_value=True, value_name="<key_file>"),
                OptionSpec(["-L"], _("Local port forwarding [bind_address:]port:host:hostport"), takes_value=True),
                OptionSpec(["-R"], _("Remote port forwarding [bind_address:]port:host:hostport"), takes_value=True),
                OptionSpec(["-D"], _("Dynamic SOCKS proxy port forwarding"), takes_value=True, value_name="<port>"),
                OptionSpec(["-N"], _("Do not execute a remote command (useful for port forwarding only)")),
                OptionSpec(["-X"], _("Enables X11 forwarding")),
                OptionSpec(["-v", "-vv", "-vvv"], _("Verbose mode (debugging)")),
                OptionSpec(["-C"], _("Requests compression of all data")),
            ],
        ),
        CommandSpec(
            name="curl",
            description=_("Command line tool for transferring data with URLs"),
            global_options=[
                OptionSpec(["-s", "--silent"], _("Silent mode (don't show progress meter or error messages)")),
                OptionSpec(["-S", "--show-error"], _("When used with -s, show error message on failure")),
                OptionSpec(["-i", "--include"], _("Include protocol response headers in the output")),
                OptionSpec(["-I", "--head"], _("Fetch HTTP response headers only")),
                OptionSpec(["-L", "--location"], _("Follow HTTP redirects")),
                OptionSpec(["-o", "--output"], _("Write output to <file> instead of stdout"), takes_value=True, value_name="<file>"),
                OptionSpec(["-O", "--remote-name"], _("Write output to a local file named like the remote file")),
                OptionSpec(["-X", "--request"], _("Specify request method (GET, POST, PUT, DELETE, PATCH)"), takes_value=True, value_name="<METHOD>"),
                OptionSpec(["-H", "--header"], _("Pass custom header line to server"), takes_value=True, value_name="<header>"),
                OptionSpec(["-d", "--data"], _("HTTP POST data"), takes_value=True, value_name="<data>"),
                OptionSpec(["-k", "--insecure"], _("Allow insecure server connections when using SSL")),
                OptionSpec(["-u", "--user"], _("Server user and password"), takes_value=True, value_name="<user:password>"),
            ],
        ),
        CommandSpec(
            name="tar",
            description=_("Archive utility for storing and extracting files"),
            global_options=[
                OptionSpec(["-x", "--extract"], _("Extract files from an archive")),
                OptionSpec(["-c", "--create"], _("Create a new archive")),
                OptionSpec(["-v", "--verbose"], _("Verbosely list files processed")),
                OptionSpec(["-z", "--gzip"], _("Filter the archive through gzip (.tar.gz / .tgz)")),
                OptionSpec(["-j", "--bzip2"], _("Filter the archive through bzip2 (.tar.bz2)")),
                OptionSpec(["-J", "--xz"], _("Filter the archive through xz (.tar.xz)")),
                OptionSpec(["-f", "--file"], _("Use archive file or device ARCHIVE"), takes_value=True, value_name="<archive>"),
                OptionSpec(["-C", "--directory"], _("Change to directory before performing any operations"), takes_value=True, value_name="<dir>"),
            ],
        ),
        CommandSpec(
            name="ufw",
            description=_("Program for managing a netfilter firewall (Uncomplicated Firewall)"),
            subcommands=[
                SubcommandSpec("status", _("Show firewall status"), options=[
                    OptionSpec(["verbose"], _("Show verbose firewall rules")),
                    OptionSpec(["numbered"], _("Show firewall rules with rule numbers")),
                ]),
                SubcommandSpec("enable", _("Enable the firewall")),
                SubcommandSpec("disable", _("Disable the firewall")),
                SubcommandSpec("reload", _("Reload firewall configuration")),
                SubcommandSpec("allow", _("Allow traffic for service or port"), examples=["ufw allow 22/tcp", "ufw allow 80", "ufw allow 443"]),
                SubcommandSpec("deny", _("Deny traffic for service or port"), examples=["ufw deny 23"]),
                SubcommandSpec("delete", _("Delete a rule"), examples=["ufw delete allow 80", "ufw delete 3"]),
                SubcommandSpec("reset", _("Reset firewall to factory defaults")),
            ],
        ),
        CommandSpec(
            name="ip",
            description=_("Show and manipulate routing, network devices, interfaces and tunnels"),
            subcommands=[
                SubcommandSpec("addr", _("Protocol address management"), aliases=["a", "address"], options=[
                    OptionSpec(["show"], _("Show IP addresses of all interfaces")),
                ], examples=["ip addr show", "ip a"]),
                SubcommandSpec("link", _("Network device configuration"), aliases=["l"], options=[
                    OptionSpec(["set"], _("Change device attributes (up, down, mtu)"), takes_value=True),
                    OptionSpec(["show"], _("Show interface state")),
                ], examples=["ip link show", "ip link set eth0 up"]),
                SubcommandSpec("route", _("Routing table management"), aliases=["r"], options=[
                    OptionSpec(["show"], _("Show routing table")),
                ], examples=["ip route show"]),
                SubcommandSpec("neigh", _("Neighbour/ARP table management"), aliases=["n"]),
            ],
        ),
        CommandSpec(
            name="rsync",
            description=_("Fast, versatile, remote (and local) file-copying tool"),
            global_options=[
                OptionSpec(["-a", "--archive"], _("Archive mode; equals -rlptgoD (preserves permissions, times, symlinks)")),
                OptionSpec(["-v", "--verbose"], _("Increase verbosity")),
                OptionSpec(["-z", "--compress"], _("Compress file data during the transfer")),
                OptionSpec(["-P", "--partial", "--progress"], _("Show progress during transfer and keep partially transferred files")),
                OptionSpec(["-h", "--human-readable"], _("Output numbers in a human-readable format")),
                OptionSpec(["--delete"], _("Delete extraneous files from destination dirs")),
                OptionSpec(["-e"], _("Specify the remote shell to use (e.g. 'ssh -p 2222')"), takes_value=True),
                OptionSpec(["--exclude"], _("Exclude files matching pattern"), takes_value=True),
            ],
        ),
        CommandSpec(
            name="ss",
            description=_("Another utility to investigate sockets (modern netstat replacement)"),
            global_options=[
                OptionSpec(["-t", "--tcp"], _("Display TCP sockets")),
                OptionSpec(["-u", "--udp"], _("Display UDP sockets")),
                OptionSpec(["-l", "--listening"], _("Display only listening sockets")),
                OptionSpec(["-p", "--processes"], _("Show process using socket")),
                OptionSpec(["-n", "--numeric"], _("Do not try to resolve service names or addresses")),
                OptionSpec(["-a", "--all"], _("Display both listening and non-listening sockets")),
            ],
        ),
    ]
