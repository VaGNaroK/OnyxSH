# onyxsh/terminal/completion/specs/registry.py
"""
Central registry for all command completion specifications.
"""

from typing import Dict, List, Optional, Type

from .base import CommandSpec
from .apt import get_apt_spec
from .dnf import get_dnf_spec
from .pacman import get_pacman_spec
from .systemctl import get_systemctl_spec
from .journalctl import get_journalctl_spec
from .docker import get_docker_spec
from .podman import get_podman_spec
from .git import get_git_spec
from .cargo import get_cargo_spec
from .npm import get_npm_spec
from .python import get_python_spec
from .pip import get_pip_spec
from .flatpak import get_flatpak_spec
from .snap import get_snap_spec
from .gh import get_gh_spec
from .kubectl import get_kubectl_spec
from .systemd_analyze import get_systemd_analyze_spec
from .scp import get_scp_spec
from .rsync import get_rsync_spec
from .curl import get_curl_spec
from .wget import get_wget_spec
from .ssh import get_ssh_spec
from .tar import get_tar_spec
from .unzip import get_unzip_spec
from .zip import get_zip_spec
from .find import get_find_spec
from .grep import get_grep_spec
from .sed import get_sed_spec
from .awk import get_awk_spec
from .chmod import get_chmod_spec
from .chown import get_chown_spec
from .ls import get_ls_spec
from .cp import get_cp_spec
from .mv import get_mv_spec
from .rm import get_rm_spec
from .ps import get_ps_spec
from .top import get_top_spec
from .htop import get_htop_spec
from .pgrep import get_pgrep_spec
from .kill import get_kill_spec
from .man import get_man_spec
from .firefox import get_firefox_spec
from .chromium import get_chromium_spec
from .code import get_code_spec
from .nvim import get_nvim_spec
from .vim import get_vim_spec
from .emacs import get_emacs_spec
from .nano import get_nano_spec
from .tmux import get_tmux_spec
from .screen import get_screen_spec
from .docker_compose import get_docker_compose_spec
from .podman_compose import get_podman_compose_spec
from .gpg import get_gpg_spec
from .openssl import get_openssl_spec
from .network_manager import get_nmcli_spec, get_nmtui_spec


class SpecRegistry:
    """Registry for managing all command specs."""

    def __init__(self):
        self._specs: Dict[str, CommandSpec] = {}
        self._load_specs()

    def _load_specs(self) -> None:
        """Load all available command specs."""
        specs: List[CommandSpec] = [
            get_apt_spec(),
            get_dnf_spec(),
            get_pacman_spec(),
            get_systemctl_spec(),
            get_journalctl_spec(),
            get_docker_spec(),
            get_podman_spec(),
            get_git_spec(),
            get_cargo_spec(),
            get_npm_spec(),
            get_python_spec(),
            get_pip_spec(),
            get_flatpak_spec(),
            get_snap_spec(),
            get_gh_spec(),
            get_kubectl_spec(),
            get_systemd_analyze_spec(),
            get_scp_spec(),
            get_rsync_spec(),
            get_curl_spec(),
            get_wget_spec(),
            get_ssh_spec(),
            get_tar_spec(),
            get_unzip_spec(),
            get_zip_spec(),
            get_find_spec(),
            get_grep_spec(),
            get_sed_spec(),
            get_awk_spec(),
            get_chmod_spec(),
            get_chown_spec(),
            get_ls_spec(),
            get_cp_spec(),
            get_mv_spec(),
            get_rm_spec(),
            get_ps_spec(),
            get_top_spec(),
            get_htop_spec(),
            get_pgrep_spec(),
            get_kill_spec(),
            get_man_spec(),
            get_firefox_spec(),
            get_chromium_spec(),
            get_code_spec(),
            get_nvim_spec(),
            get_vim_spec(),
            get_emacs_spec(),
            get_nano_spec(),
            get_tmux_spec(),
            get_screen_spec(),
            get_docker_compose_spec(),
            get_podman_compose_spec(),
            get_gpg_spec(),
            get_openssl_spec(),
            get_nmcli_spec(),
            get_nmtui_spec(),
        ]

        for spec in specs:
            self._register_spec(spec)

    def _register_spec(self, spec: CommandSpec) -> None:
        """Register a command spec and its aliases."""
        self._specs[spec.name] = spec
        for alias in spec.aliases:
            self._specs[alias] = spec

    def get_spec(self, command: str) -> Optional[CommandSpec]:
        """Get a command spec by name or alias."""
        return self._specs.get(command)

    def get_all_commands(self) -> List[str]:
        """Get all registered command names."""
        commands = set(self._specs.keys())
        return sorted(commands)

    def has_spec(self, command: str) -> bool:
        """Check if a spec exists for a command."""
        return command in self._specs
