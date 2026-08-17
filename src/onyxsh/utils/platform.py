# onyxsh/utils/platform.py

import os
import shutil
from pathlib import Path
from typing import Dict, List, Optional, Union

from .exceptions import ConfigError
from .logger import get_logger


class PlatformInfo:
    """Information about the current platform (assumed to be Linux)."""

    def __init__(self):
        self.logger = get_logger("onyxsh.platform")
        self.home_dir = Path.home()
        self.config_dir = self._get_config_directory()
        self.cache_dir = self._get_cache_directory()
        self.ssh_dir = self.home_dir / ".ssh"
        self.architecture = self._detect_architecture()
        self._detect_commands()

    def _detect_architecture(self) -> str:
        """Detect the system architecture."""
        import platform

        return platform.machine() or "unknown"

    def _get_config_directory(self) -> Path:
        """Get the configuration directory for Linux."""
        if xdg_config := os.environ.get("XDG_CONFIG_HOME"):
            return Path(xdg_config) / "onyxsh"
        return self.home_dir / ".config" / "onyxsh"

    def _get_cache_directory(self) -> Path:
        """Get the cache directory for Linux."""
        if xdg_cache := os.environ.get("XDG_CACHE_HOME"):
            return Path(xdg_cache) / "onyxsh"
        return self.home_dir / ".cache" / "onyxsh"

    def _detect_commands(self):
        """Detect available system commands that are essential for the application."""
        self.commands = {}
        command_list = ["ssh", "sshpass", "sftp", "rsync"]
        for cmd in command_list:
            if cmd_path := shutil.which(cmd):
                self.commands[cmd] = cmd_path

    def has_command(self, command: str) -> bool:
        """Check if a command is available."""
        return command in self.commands


class PathManager:
    """Path management utilities for Linux."""

    def __init__(self, platform_info: PlatformInfo):
        self.platform_info = platform_info
        self.logger = get_logger("onyxsh.platform.paths")

    def normalize_path(self, path: Union[str, Path]) -> Path:
        """Normalize a path by expanding user and resolving it."""
        path = Path(path).expanduser()
        if not path.is_absolute():
            path = path.resolve()
        return path

    def create_directory_safe(self, directory: Path, mode: int = 0o755) -> bool:
        """Safely create a directory with appropriate permissions."""
        try:
            directory.mkdir(parents=True, exist_ok=True)
            directory.chmod(mode)
            return True
        except Exception as e:
            self.logger.error(f"Failed to create directory {directory}: {e}")
            return False


class CommandBuilder:
    """Build commands for a Linux environment."""

    def __init__(self, platform_info: PlatformInfo):
        self.platform_info = platform_info

    def build_remote_command(
        self,
        command_type: str,
        hostname: str,
        username: Optional[str] = None,
        key_file: Optional[str] = None,
        port: Optional[int] = None,
        options: Optional[Dict[str, str]] = None,
        remote_path: Optional[str] = None,
    ) -> List[str]:
        """Builds a remote command (ssh, sftp)."""
        if not self.platform_info.has_command(command_type):
            raise ConfigError(f"{command_type.upper()} command not found")

        cmd = [shutil.which(command_type)]
        if options:
            for key, value in options.items():
                cmd.extend(["-o", f"{key}={value}"])
        if key_file:
            cmd.extend(["-i", key_file])
        if port:
            port_flag = "-P" if command_type == "sftp" else "-p"
            cmd.extend([port_flag, str(port)])
        target = f"{username}@{hostname}" if username else hostname
        if command_type == "sftp" and remote_path:
            target = f"{target}:{remote_path.strip()}"
        cmd.append(target)
        return cmd


class EnvironmentManager:
    """Manage environment variables for terminal sessions."""

    def __init__(self, platform_info: PlatformInfo):
        self.platform_info = platform_info

    def get_terminal_environment(self) -> Dict[str, str]:
        """Get environment variables for terminal sessions."""
        env = os.environ.copy()
        env["TERM"] = "xterm-256color"
        env["COLORTERM"] = "truecolor"
        if "LANG" not in env:
            import locale

            try:
                system_locale = locale.getdefaultlocale()[0]
                env["LANG"] = f"{system_locale}.UTF-8" if system_locale else "C.UTF-8"
            except Exception:
                env["LANG"] = "C.UTF-8"
        return env


# Singleton instances for efficiency
_platform_info: Optional[PlatformInfo] = None
_path_manager: Optional[PathManager] = None
_command_builder: Optional[CommandBuilder] = None
_environment_manager: Optional[EnvironmentManager] = None


def get_platform_info() -> PlatformInfo:
    """Get the global platform information instance."""
    global _platform_info
    if _platform_info is None:
        _platform_info = PlatformInfo()
    return _platform_info


def get_path_manager() -> PathManager:
    global _path_manager
    if _path_manager is None:
        _path_manager = PathManager(get_platform_info())
    return _path_manager


def get_command_builder() -> CommandBuilder:
    global _command_builder
    if _command_builder is None:
        _command_builder = CommandBuilder(get_platform_info())
    return _command_builder


def get_environment_manager() -> EnvironmentManager:
    global _environment_manager
    if _environment_manager is None:
        _environment_manager = EnvironmentManager(get_platform_info())
    return _environment_manager


def _read_os_release() -> Dict[str, str]:
    """
    Read host or system /etc/os-release as a key/value mapping.
    Prioritizes the real host OS release file when running inside Flatpak sandboxes.

    Returns empty dict when file is unavailable.
    """
    candidates = [
        Path("/var/run/host/os-release"),
        Path("/run/host/os-release"),
        Path("/run/host/usr/lib/os-release"),
        Path("/run/host/etc/os-release"),
        Path("/etc/os-release"),
        Path("/usr/lib/os-release"),
    ]
    for os_release_path in candidates:
        if not os_release_path.exists():
            continue
        try:
            data: Dict[str, str] = {}
            for raw_line in os_release_path.read_text(encoding="utf-8").splitlines():
                line = raw_line.strip()
                if not line or line.startswith("#") or "=" not in line:
                    continue
                key, value = line.split("=", 1)
                value = value.strip().strip('"').strip("'")
                data[key] = value
            # If this is the real host or native file, return it
            if data and ("flatpak" not in data.get("NAME", "").lower() or os_release_path == candidates[-1]):
                return data
        except Exception:
            continue

    # If inside Flatpak and candidate files weren't directly accessible, try portal
    if shutil.which("flatpak-spawn"):
        try:
            import subprocess
            out = subprocess.check_output(
                ["flatpak-spawn", "--host", "cat", "/etc/os-release"],
                stderr=subprocess.DEVNULL,
                timeout=2,
            ).decode("utf-8", errors="replace")
            data: Dict[str, str] = {}
            for raw_line in out.splitlines():
                line = raw_line.strip()
                if not line or line.startswith("#") or "=" not in line:
                    continue
                key, value = line.split("=", 1)
                data[key] = value.strip().strip('"').strip("'")
            if data:
                return data
        except Exception:
            pass

    return {}


def detect_os_context() -> str:
    """
    Detect the real host OS name and base distribution for AI context.
    Properly identifies the host operating system even when running inside Flatpak.
    """
    info = _read_os_release()
    os_name = info.get("PRETTY_NAME") or info.get("NAME") or "Linux"
    base_distro = info.get("ID_LIKE") or info.get("ID") or ""

    # Fallback to lsb-release if os-release did not yield a specific name
    if os_name == "Linux":
        for lsb_path in [
            Path("/var/run/host/etc/lsb-release"),
            Path("/run/host/etc/lsb-release"),
            Path("/etc/lsb-release"),
        ]:
            if lsb_path.exists():
                try:
                    for line in lsb_path.read_text(encoding="utf-8").splitlines():
                        if line.startswith("DISTRIB_DESCRIPTION="):
                            os_name = line.split("=", 1)[1].strip().strip('"').strip("'")
                            break
                except Exception:
                    pass

    # Append base distro context (e.g. "Linux Mint 22.3 (based on ubuntu debian)")
    if base_distro and base_distro.lower() not in os_name.lower() and "freedesktop" not in base_distro.lower():
        return f"{os_name} (based on {base_distro})"
    return os_name


def is_flatpak_sandbox() -> bool:
    """Return True if the current process is running inside a Flatpak container."""
    return os.path.exists("/.flatpak-info") or bool(os.environ.get("FLATPAK_ID"))


def get_user_shell() -> str:
    """
    Get the default shell for the user.
    When running in Flatpak, queries the host system's user shell.
    """
    if is_flatpak_sandbox() and shutil.which("flatpak-spawn"):
        try:
            import subprocess
            out = subprocess.check_output(
                ["flatpak-spawn", "--host", "sh", "-c", 'getent passwd "$USER" | cut -d: -f7 || echo "$SHELL"'],
                stderr=subprocess.DEVNULL,
                timeout=2,
            ).decode("utf-8", errors="replace").strip()
            if out and os.path.isabs(out):
                return out
        except Exception:
            pass
        return "/bin/bash"

    try:
        import gi
        gi.require_version("Vte", "3.91")
        from gi.repository import Vte
        return Vte.get_user_shell()
    except Exception:
        return os.environ.get("SHELL", "/bin/bash")


def _version_tuple(version: str) -> tuple[int, int]:
    """Parse semantic distro version into (major, minor)."""
    if not version:
        return (0, 0)
    parts = version.split(".")
    try:
        major = int(parts[0])
        minor = int(parts[1]) if len(parts) > 1 else 0
        return (major, minor)
    except Exception:
        return (0, 0)


def is_ubuntu_at_least(major: int, minor: int) -> bool:
    """
    Return True when current distro is Ubuntu and version is >= major.minor.
    """
    info = _read_os_release()
    distro_id = info.get("ID", "").strip().lower()
    if distro_id != "ubuntu":
        return False
    current = _version_tuple(info.get("VERSION_ID", ""))
    return current >= (major, minor)


def apply_graphics_safety_fallbacks() -> None:
    """
    Apply conservative GTK/GSK environment fallbacks for unstable stacks.

    On Ubuntu 25.10+, some combinations of GTK4 + Mesa/Zink have shown
    crashes/segfaults with the default GL renderer. We force software Cairo
    when GSK_RENDERER is not explicitly set by the user.
    """
    logger = get_logger("onyxsh.platform.graphics")

    # Respect user override.
    if os.environ.get("GSK_RENDERER"):
        return

    if is_ubuntu_at_least(25, 10):
        os.environ["GSK_RENDERER"] = "cairo"
        logger.warning(
            "Detected Ubuntu 25.10+; forcing GSK_RENDERER=cairo for stability."
        )


def should_use_native_tooltips() -> bool:
    """
    Decide whether custom popover tooltips should be disabled for stability.

    Env override:
      - ONYXSH_NATIVE_TOOLTIPS=1/true/yes/on forces native tooltips.
    """
    env_value = os.environ.get("ONYXSH_NATIVE_TOOLTIPS", "").strip().lower()
    if env_value in {"1", "true", "yes", "on"}:
        return True

    # Ubuntu 25.10+ currently uses native GTK tooltips due to popover flicker
    # and renderer instability reported in the field.
    return is_ubuntu_at_least(25, 10)


def get_config_directory() -> Path:
    return get_platform_info().config_dir


def get_ssh_directory() -> Path:
    return get_platform_info().ssh_dir


def has_command(command: str) -> bool:
    return get_platform_info().has_command(command)


def normalize_path(path: Union[str, Path]) -> Path:
    return get_path_manager().normalize_path(path)


def ensure_directory_exists(directory: Union[str, Path], mode: int = 0o755) -> bool:
    """Ensure directory exists, creating it if necessary."""
    try:
        path_manager = get_path_manager()
        directory_path = path_manager.normalize_path(directory)
        if directory_path.exists():
            if not directory_path.is_dir():
                raise ConfigError(
                    f"Path exists but is not a directory: {directory_path}"
                )
            return True
        return path_manager.create_directory_safe(directory_path, mode)
    except Exception as e:
        logger = get_logger("onyxsh.platform.directory")
        logger.error(f"Failed to ensure directory exists: {directory}: {e}")
        raise ConfigError(f"Failed to create directory: {directory}")


def get_package_manager() -> str:
    """Detect default system package manager (apt, pacman, dnf, zypper)."""
    for pm in ("apt", "pacman", "dnf", "zypper", "apk"):
        if shutil.which(pm):
            return pm
    return "apt"


def detect_gpu_info() -> Dict[str, Union[str, int]]:
    """
    Detect GPU device and VRAM capacity for AI model context estimation.
    Supports native execution and Flatpak sandbox (via flatpak-spawn).

    Returns:
        Dict with keys: 'vendor', 'name', 'vram_mb', 'recommended_context_tokens', 'description'
    """
    import glob
    import subprocess
    import shutil

    def _run_cmd(cmd_list: list) -> str:
        # 1. Try directly first
        if shutil.which(cmd_list[0]):
            try:
                return subprocess.check_output(
                    cmd_list, stderr=subprocess.DEVNULL, timeout=2
                ).decode("utf-8", errors="replace").strip()
            except Exception:
                pass
        # 2. If inside Flatpak sandbox, try running via host portal
        if shutil.which("flatpak-spawn"):
            try:
                return subprocess.check_output(
                    ["flatpak-spawn", "--host"] + cmd_list,
                    stderr=subprocess.DEVNULL,
                    timeout=2,
                ).decode("utf-8", errors="replace").strip()
            except Exception:
                pass
        return ""

    # 1. Try NVIDIA via nvidia-smi (directly or via host)
    smi_out = _run_cmd(["nvidia-smi", "--query-gpu=name,memory.total", "--format=csv,noheader,nounits"])
    if smi_out:
        try:
            lines = smi_out.splitlines()
            first = lines[0].split(",")
            name = first[0].strip()
            vram_mb = int(first[1].strip())
            vram_gb = round(vram_mb / 1024, 1)

            if vram_mb >= 24000:
                rec_tokens = 65536
                rec_label = "64K"
            elif vram_mb >= 12000:
                rec_tokens = 32768
                rec_label = "32K"
            elif vram_mb >= 8000:
                rec_tokens = 16384
                rec_label = "16K ou 32K"
            elif vram_mb >= 5000:
                rec_tokens = 8192
                rec_label = "8K ou 16K"
            else:
                rec_tokens = 4096
                rec_label = "4K"

            return {
                "vendor": "nvidia",
                "name": name,
                "vram_mb": vram_mb,
                "recommended_context_tokens": rec_tokens,
                "description": f"{name} ({vram_gb} GB VRAM) — Recomendado: {rec_label}",
            }
        except Exception:
            pass


    # 2. Try AMD / Intel / DRM Sysfs
    for vram_file in glob.glob("/sys/class/drm/card*/device/mem_info_vram_total"):
        try:
            with open(vram_file, "r", encoding="utf-8") as f:
                vram_bytes = int(f.read().strip())
                vram_mb = vram_bytes // (1024 * 1024)
                if vram_mb > 512:
                    vram_gb = round(vram_mb / 1024, 1)
                    if vram_mb >= 16000:
                        rec_tokens, rec_label = 32768, "32K"
                    elif vram_mb >= 8000:
                        rec_tokens, rec_label = 16384, "16K"
                    elif vram_mb >= 4000:
                        rec_tokens, rec_label = 8192, "8K"
                    else:
                        rec_tokens, rec_label = 4096, "4K"

                    return {
                        "vendor": "drm",
                        "name": "GPU Dedicada (DRM/Mesa)",
                        "vram_mb": vram_mb,
                        "recommended_context_tokens": rec_tokens,
                        "description": f"GPU Dedicada ({vram_gb} GB VRAM) — Recomendado: {rec_label}",
                    }
        except Exception:
            pass

    # 3. Fallback to System RAM (CPU execution)
    try:
        with open("/proc/meminfo", "r", encoding="utf-8") as f:
            for line in f:
                if line.startswith("MemTotal:"):
                    kb = int(line.split()[1])
                    ram_gb = round(kb / (1024 * 1024), 1)
                    return {
                        "vendor": "system",
                        "name": "CPU / Memória RAM do Sistema",
                        "vram_mb": kb // 1024,
                        "recommended_context_tokens": 8192,
                        "description": f"CPU / RAM ({ram_gb} GB RAM) — Recomendado: 4K ou 8K",
                    }
    except Exception:
        pass

    return {
        "vendor": "unknown",
        "name": "Dispositivo Padrão",
        "vram_mb": 8192,
        "recommended_context_tokens": 8192,
        "description": "Recomendado: 8K",
    }


