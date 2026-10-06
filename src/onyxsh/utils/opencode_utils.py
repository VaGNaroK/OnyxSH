# onyxsh/utils/opencode_utils.py
"""
OpenCode CLI utility and integration helper for OnyxSH.

Provides:
- Binary detection for native Linux and Flatpak sandbox environments.
- Version inspection and semantic compatibility checking.
- In-memory thread-safe caching of availability and version info.
- Command builders for TUI interactive sessions and headless JSON execution.
"""

import os
import re
import shutil
import subprocess
import threading
import time
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

from .logger import get_logger
from .platform import get_command_builder, is_flatpak_sandbox

logger = get_logger("onyxsh.utils.opencode")

# Constants
OPENCODE_MIN_VERSION = "0.1.0"
OPENCODE_INSTALL_CMD = "curl -fsSL https://opencode.ai/v2/install | bash"
OPENCODE_DOCS_URL = "https://opencode.ai"
DEFAULT_TIMEOUT_SECONDS = 120

# Settings keys
SETTING_OPENCODE_ENABLED = "opencode.enabled"
SETTING_OPENCODE_BINARY_PATH = "opencode.binary_path"
SETTING_OPENCODE_LAUNCH_MODE = "opencode.launch_mode"

# Cache TTL in seconds
_CACHE_TTL_SECONDS = 30.0

# In-memory detection cache
_cache_lock = threading.Lock()
_cached_binary_path: Optional[str] = None
_cached_version: Optional[str] = None
_cached_available: Optional[bool] = None
_cached_timestamp: float = 0.0


def clear_opencode_cache() -> None:
    """Clear in-memory cached detection results."""
    global _cached_binary_path, _cached_version, _cached_available, _cached_timestamp
    with _cache_lock:
        _cached_binary_path = None
        _cached_version = None
        _cached_available = None
        _cached_timestamp = 0.0
    logger.debug("OpenCode detection cache cleared")


def _get_setting(settings_manager: Any, dot_key: str, default: Any = None) -> Any:
    """Retrieve setting supporting dot-notation or snake_case fallback."""
    if settings_manager is None:
        try:
            from ..settings.manager import get_settings_manager

            settings_manager = get_settings_manager()
        except Exception:
            return default

    # 1. Try dot key
    val = settings_manager.get(dot_key, None)
    if val is not None:
        return val

    # 2. Try snake_case key
    snake_key = dot_key.replace(".", "_")
    val = settings_manager.get(snake_key, None)
    if val is not None:
        return val

    return default


def parse_semantic_version(version_str: str) -> Optional[Tuple[int, int, int]]:
    """
    Extract (major, minor, patch) integer tuple from a version string.
    
    Returns:
        (major, minor, patch) tuple or None if parsing fails.
    """
    if not version_str:
        return None
    match = re.search(r"(\d+)(?:\.(\d+))?(?:\.(\d+))?", version_str.strip())
    if not match:
        return None
    major = int(match.group(1))
    minor = int(match.group(2)) if match.group(2) is not None else 0
    patch = int(match.group(3)) if match.group(3) is not None else 0
    return (major, minor, patch)


def is_version_compatible(
    version_str: str, min_version: str = OPENCODE_MIN_VERSION
) -> bool:
    """
    Check if detected version satisfies the minimum version requirement.
    
    Args:
        version_str: Detected version string (e.g. '0.1.2' or 'opencode 1.0.0').
        min_version: Minimum required version string (default: OPENCODE_MIN_VERSION).
    """
    parsed_current = parse_semantic_version(version_str)
    parsed_min = parse_semantic_version(min_version)

    if not parsed_current or not parsed_min:
        # If version string cannot be parsed, conservatively allow if non-empty
        return bool(version_str and version_str.strip())

    return parsed_current >= parsed_min


def get_opencode_binary_path(settings_manager: Any = None) -> Optional[str]:
    """
    Resolve the absolute or callable path to the OpenCode binary.
    
    Checks in order:
    1. Custom path in settings (opencode.binary_path).
    2. Native system PATH (shutil.which).
    3. Common user installation directories (~/.local/bin, /usr/local/bin).
    4. Host query when running in Flatpak sandbox.
    """
    # 1. Check custom path from settings
    custom_path = _get_setting(settings_manager, SETTING_OPENCODE_BINARY_PATH, "")
    if custom_path and isinstance(custom_path, str) and custom_path.strip():
        custom_path = custom_path.strip()
        expanded = str(Path(custom_path).expanduser())
        # In sandbox, host files might not be directly in local mount, but if it exists or in flatpak allow it
        if os.path.exists(expanded) or is_flatpak_sandbox() or shutil.which(custom_path):
            return expanded

    # 2. Check local PATH
    if which_path := shutil.which("opencode"):
        return which_path

    # 3. Check common fallback locations
    candidates = [
        Path.home() / ".opencode" / "bin" / "opencode",
        Path.home() / ".local" / "bin" / "opencode",
        Path.home() / ".cargo" / "bin" / "opencode",
        Path("/usr/local/bin/opencode"),
        Path("/usr/bin/opencode"),
    ]
    for candidate in candidates:
        cand_str = str(candidate)
        if os.path.exists(cand_str) and os.access(cand_str, os.X_OK):
            return cand_str

    # 4. If running in Flatpak sandbox, query host
    if is_flatpak_sandbox():
        builder = get_command_builder()
        # Query host using interactive bash which sources ~/.bashrc (where curl installer appends PATH)
        try:
            cmd = builder.wrap_host_command(["bash", "-ic", "which opencode"])
            output = subprocess.check_output(
                cmd, stderr=subprocess.DEVNULL, timeout=2
            ).decode("utf-8", errors="replace").strip()
            if output:
                lines = [line.strip() for line in output.splitlines() if line.strip()]
                for line in reversed(lines):
                    if line.endswith("opencode") and not line.startswith("which:"):
                        return line
        except Exception:
            pass

        # Check standard host candidate paths directly via test -x
        host_candidates = [
            "~/.opencode/bin/opencode",
            "~/.local/bin/opencode",
            "/usr/local/bin/opencode",
            "/usr/bin/opencode",
        ]
        for host_cand in host_candidates:
            try:
                test_cmd = builder.wrap_host_command(["bash", "-c", f"test -x {host_cand} && echo {host_cand}"])
                output = subprocess.check_output(
                    test_cmd, stderr=subprocess.DEVNULL, timeout=2
                ).decode("utf-8", errors="replace").strip()
                if output and output.endswith("opencode"):
                    resolve_cmd = builder.wrap_host_command(["bash", "-c", f"echo {host_cand}"])
                    res = subprocess.check_output(
                        resolve_cmd, stderr=subprocess.DEVNULL, timeout=2
                    ).decode("utf-8", errors="replace").strip()
                    return res or host_cand
            except Exception:
                pass

        # Query host which directly
        cmd = builder.wrap_host_command(["which", "opencode"])
        try:
            output = subprocess.check_output(
                cmd, stderr=subprocess.DEVNULL, timeout=2
            ).decode("utf-8", errors="replace").strip()
            if output:
                first_line = output.splitlines()[0].strip()
                if first_line and not first_line.startswith("which:"):
                    return first_line
        except Exception:
            pass

        # Try host-spawn running opencode --version as fallback test
        test_cmd = builder.wrap_host_command(["opencode", "--version"])
        try:
            ret = subprocess.call(
                test_cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=2
            )
            if ret == 0:
                return "opencode"
        except Exception:
            pass

    return None


def get_opencode_version(
    binary_path: Optional[str] = None, settings_manager: Any = None
) -> Optional[str]:
    """
    Execute OpenCode binary with --version and extract the version string.
    
    Args:
        binary_path: Optional explicit binary path. If None, resolves automatically.
        settings_manager: Optional settings manager.
        
    Returns:
        Clean version string (e.g. '0.2.1') or None if failed.
    """
    target_bin = binary_path or get_opencode_binary_path(settings_manager)
    if not target_bin:
        return None

    builder = get_command_builder()
    raw_cmd = [target_bin, "--version"]
    cmd = builder.wrap_host_command(raw_cmd)

    try:
        output = subprocess.check_output(
            cmd, stderr=subprocess.STDOUT, timeout=3
        ).decode("utf-8", errors="replace").strip()

        # Regex match for semantic version
        match = re.search(r"v?(\d+\.\d+(?:\.\d+)?(?:-[0-9A-Za-z.-]+)?)", output)
        if match:
            return match.group(1)
        return output.splitlines()[0].strip() if output else None
    except Exception as e:
        logger.debug(f"Failed to query OpenCode version: {e}")
        return None


def is_opencode_available(
    settings_manager: Any = None,
    force_refresh: bool = False,
    validate_version: bool = False,
) -> bool:
    """
    Check if OpenCode is enabled in settings, installed, and runnable.
    
    Caches the availability result in memory for _CACHE_TTL_SECONDS.
    
    Args:
        settings_manager: Optional settings manager.
        force_refresh: If True, bypasses cache and re-detects.
        validate_version: If True, also ensures version >= OPENCODE_MIN_VERSION.
    """
    global _cached_binary_path, _cached_version, _cached_available, _cached_timestamp

    # Check if disabled by user settings
    enabled = _get_setting(settings_manager, SETTING_OPENCODE_ENABLED, True)
    if not enabled:
        return False

    now = time.time()
    with _cache_lock:
        if (
            not force_refresh
            and _cached_available is not None
            and (now - _cached_timestamp) < _CACHE_TTL_SECONDS
        ):
            return _cached_available

    bin_path = get_opencode_binary_path(settings_manager)
    available = False
    version_str: Optional[str] = None

    if bin_path:
        if validate_version:
            version_str = get_opencode_version(bin_path, settings_manager)
            if version_str and is_version_compatible(version_str, OPENCODE_MIN_VERSION):
                available = True
            else:
                logger.warning(
                    f"OpenCode detected at '{bin_path}' but version '{version_str}' is incompatible (< {OPENCODE_MIN_VERSION})"
                )
                available = False
        else:
            available = True

    with _cache_lock:
        _cached_binary_path = bin_path
        _cached_version = version_str
        _cached_available = available
        _cached_timestamp = now

    return available


def build_opencode_command(
    subcommand: Optional[str] = None,
    args: Optional[List[str]] = None,
    working_directory: Optional[str] = None,
    settings_manager: Any = None,
) -> List[str]:
    """
    Build execution command line for OpenCode, automatically applying
    Flatpak host-spawn or flatpak-spawn wrappers if running inside a sandbox.
    
    Args:
        subcommand: Subcommand such as 'run', 'serve', etc. (None for interactive TUI).
        args: Optional additional arguments list.
        working_directory: Working directory to set for host execution.
        settings_manager: Optional settings manager.
        
    Returns:
        List of command tokens ready for execution.
    """
    bin_path = get_opencode_binary_path(settings_manager) or "opencode"
    cmd = [bin_path]
    if subcommand:
        cmd.append(subcommand)
    if args:
        cmd.extend(args)

    builder = get_command_builder()
    return builder.wrap_host_command(cmd, working_directory=working_directory)


def get_opencode_install_command() -> str:
    """Return the official installation command string for OpenCode CLI."""
    return OPENCODE_INSTALL_CMD
