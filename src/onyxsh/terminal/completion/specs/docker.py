# onyxsh/terminal/completion/specs/docker.py
"""
Completion specifications for Docker and Podman container engines.
"""

from ....utils.translation_utils import _
from .base import CommandSpec, OptionSpec, SubcommandSpec


def get_docker_spec() -> CommandSpec:
    """Builds and returns the Docker command spec."""
    return CommandSpec(
        name="docker",
        description=_("Container runtime and image management CLI"),
        aliases=["podman"],
        subcommands=[
            SubcommandSpec(
                name="ps",
                description=_("List active containers"),
                options=[
                    OptionSpec(["-a", "--all"], _("Show all containers (default shows just running)")),
                    OptionSpec(["-q", "--quiet"], _("Only display numeric container IDs")),
                    OptionSpec(["--filter"], _("Filter output based on conditions provided"), takes_value=True),
                ],
                examples=["docker ps", "docker ps -a"],
            ),
            SubcommandSpec(
                name="run",
                description=_("Create and run a new container from an image"),
                options=[
                    OptionSpec(["-d", "--detach"], _("Run container in background and print container ID")),
                    OptionSpec(["-it"], _("Allocate a pseudo-TTY connected to the container's stdin")),
                    OptionSpec(["--rm"], _("Automatically remove the container when it exits")),
                    OptionSpec(["-p", "--publish"], _("Publish container port(s) to the host"), takes_value=True, value_name="<host:container>"),
                    OptionSpec(["-v", "--volume"], _("Bind mount a volume"), takes_value=True, value_name="<host:container>"),
                    OptionSpec(["--name"], _("Assign a custom name to the container"), takes_value=True, value_name="<name>"),
                    OptionSpec(["--restart"], _("Restart policy to apply when container exits"), takes_value=True, value_name="<always|unless-stopped>"),
                    OptionSpec(["-e", "--env"], _("Set environment variables"), takes_value=True, value_name="<KEY=VAL>"),
                    OptionSpec(["--network"], _("Connect a container to a network"), takes_value=True, value_name="<net>"),
                ],
                examples=["docker run -d -p 80:80 --name web nginx", "docker run -it --rm ubuntu bash"],
            ),
            SubcommandSpec(
                name="exec",
                description=_("Execute a command inside a running container"),
                options=[
                    OptionSpec(["-it"], _("Allocate a pseudo-TTY and keep stdin open")),
                    OptionSpec(["-d", "--detach"], _("Detached mode: run command in the background")),
                    OptionSpec(["-u", "--user"], _("Username or UID"), takes_value=True, value_name="<user>"),
                ],
                examples=["docker exec -it web bash", "docker exec -it db psql -U postgres"],
            ),
            SubcommandSpec(
                name="logs",
                description=_("Fetch the logs of a container"),
                options=[
                    OptionSpec(["-f", "--follow"], _("Follow log output in real time")),
                    OptionSpec(["--tail"], _("Number of lines to show from the end of the logs"), takes_value=True, value_name="<N>"),
                    OptionSpec(["-t", "--timestamps"], _("Show timestamps")),
                ],
                examples=["docker logs -f web", "docker logs --tail 100 api"],
            ),
            SubcommandSpec(
                name="images",
                description=_("List local container images"),
                options=[
                    OptionSpec(["-a", "--all"], _("Show all images")),
                    OptionSpec(["-q", "--quiet"], _("Only show numeric image IDs")),
                ],
                examples=["docker images"],
            ),
            SubcommandSpec(
                name="pull",
                description=_("Download an image from a container registry"),
                examples=["docker pull ubuntu:latest", "docker pull redis:alpine"],
            ),
            SubcommandSpec(
                name="push",
                description=_("Upload an image to a container registry"),
            ),
            SubcommandSpec(
                name="build",
                description=_("Build an image from a Dockerfile"),
                options=[
                    OptionSpec(["-t", "--tag"], _("Name and optionally a tag in the 'name:tag' format"), takes_value=True, value_name="<tag>"),
                    OptionSpec(["-f", "--file"], _("Name of the Dockerfile"), takes_value=True, value_name="<Dockerfile>"),
                    OptionSpec(["--no-cache"], _("Do not use cache when building the image")),
                ],
                examples=["docker build -t myapp:latest ."],
            ),
            SubcommandSpec(
                name="stop",
                description=_("Stop one or more running containers"),
                options=[
                    OptionSpec(["-t", "--time"], _("Seconds to wait before killing the container"), takes_value=True),
                ],
                examples=["docker stop web", "docker stop $(docker ps -q)"],
            ),
            SubcommandSpec(
                name="start",
                description=_("Start one or more stopped containers"),
                examples=["docker start web"],
            ),
            SubcommandSpec(
                name="restart",
                description=_("Restart one or more containers"),
                examples=["docker restart web"],
            ),
            SubcommandSpec(
                name="rm",
                description=_("Remove one or more stopped containers"),
                options=[
                    OptionSpec(["-f", "--force"], _("Force the removal of a running container")),
                    OptionSpec(["-v", "--volumes"], _("Remove anonymous volumes associated with container")),
                ],
                examples=["docker rm web", "docker rm -f $(docker ps -aq)"],
            ),
            SubcommandSpec(
                name="rmi",
                description=_("Remove one or more container images"),
                options=[
                    OptionSpec(["-f", "--force"], _("Force removal of the image")),
                ],
            ),
            SubcommandSpec(
                name="compose",
                description=_("Define and run multi-container applications with Docker Compose"),
                subcommands=[
                    SubcommandSpec("up", _("Create and start containers"), options=[
                        OptionSpec(["-d", "--detach"], _("Detached mode: Run containers in background")),
                        OptionSpec(["--build"], _("Build images before starting containers")),
                    ]),
                    SubcommandSpec("down", _("Stop and remove containers, networks, and volumes"), options=[
                        OptionSpec(["-v", "--volumes"], _("Remove named volumes")),
                    ]),
                    SubcommandSpec("logs", _("View output from containers"), options=[
                        OptionSpec(["-f", "--follow"], _("Follow log output")),
                    ]),
                    SubcommandSpec("restart", _("Restart compose services")),
                    SubcommandSpec("ps", _("List compose containers")),
                ],
            ),
            SubcommandSpec(
                name="system",
                description=_("Manage Docker system resources"),
                subcommands=[
                    SubcommandSpec("prune", _("Remove unused data (containers, networks, dangling images)"), options=[
                        OptionSpec(["-a", "--all"], _("Remove all unused images, not just dangling ones")),
                        OptionSpec(["--volumes"], _("Prune anonymous volumes as well")),
                    ]),
                    SubcommandSpec("df", _("Show docker disk usage")),
                ],
            ),
        ],
        global_options=[
            OptionSpec(["-h", "--help"], _("Show help information")),
            OptionSpec(["-v", "--version"], _("Print version information")),
        ],
    )
