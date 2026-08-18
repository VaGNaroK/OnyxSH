# onyxsh/terminal/completion/specs/registry.py
"""
Registry for command specifications, lookup and completion resolution.
"""

from typing import Dict, List, Optional

from .apt import get_apt_spec
from .base import CommandSpec
from .common import get_common_specs
from .devops import get_devops_specs
from .docker import get_docker_spec
from .git import get_git_spec
from .systemd import get_journalctl_spec, get_systemctl_spec


class SpecRegistry:
    """Central registry of known command specifications."""

    def __init__(self) -> None:
        self._specs: Dict[str, CommandSpec] = {}
        self._load_specs()

    def _load_specs(self) -> None:
        """Initializes and registers all built-in command specifications."""
        specs: List[CommandSpec] = [
            get_apt_spec(),
            get_systemctl_spec(),
            get_journalctl_spec(),
            get_docker_spec(),
            get_git_spec(),
        ]
        specs.extend(get_devops_specs())
        specs.extend(get_common_specs())

        for spec in specs:
            self._specs[spec.name.lower()] = spec
            for alias in spec.aliases:
                self._specs[alias.lower()] = spec

    def get_spec(self, command_name: str) -> Optional[CommandSpec]:
        """Looks up a command specification by executable name."""
        return self._specs.get(command_name.lower())

    def get_all_command_names(self) -> List[str]:
        """Returns all registered primary command names."""
        return sorted(list(self._specs.keys()))


_instance: Optional[SpecRegistry] = None


def get_spec_registry() -> SpecRegistry:
    """Returns the singleton SpecRegistry instance."""
    global _instance
    if _instance is None:
        _instance = SpecRegistry()
    return _instance
