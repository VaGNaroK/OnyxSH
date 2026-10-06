# tests/test_opencode_utils.py

import os
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

from onyxsh.utils.opencode_utils import (
    DEFAULT_TIMEOUT_SECONDS,
    OPENCODE_INSTALL_CMD,
    OPENCODE_MIN_VERSION,
    SETTING_OPENCODE_BINARY_PATH,
    SETTING_OPENCODE_ENABLED,
    SETTING_OPENCODE_LAUNCH_MODE,
    build_opencode_command,
    clear_opencode_cache,
    get_opencode_binary_path,
    get_opencode_install_command,
    get_opencode_version,
    is_opencode_available,
    is_version_compatible,
    parse_semantic_version,
)
from onyxsh.utils.platform import CommandBuilder, PlatformInfo


class TestOpenCodeUtils(unittest.TestCase):
    """Test suite for OpenCode CLI integration utilities and detection."""

    def setUp(self):
        clear_opencode_cache()

    def tearDown(self):
        clear_opencode_cache()

    def test_parse_semantic_version(self):
        self.assertEqual(parse_semantic_version("0.1.0"), (0, 1, 0))
        self.assertEqual(parse_semantic_version("1.2.3"), (1, 2, 3))
        self.assertEqual(parse_semantic_version("0.2"), (0, 2, 0))
        self.assertEqual(parse_semantic_version("v1.4.0"), (1, 4, 0))
        self.assertEqual(parse_semantic_version("opencode 0.3.5-beta.1"), (0, 3, 5))
        self.assertIsNone(parse_semantic_version(""))
        self.assertIsNone(parse_semantic_version(None))
        self.assertIsNone(parse_semantic_version("no-digits-here"))

    def test_is_version_compatible(self):
        self.assertTrue(is_version_compatible("0.1.0", "0.1.0"))
        self.assertTrue(is_version_compatible("0.2.0", "0.1.0"))
        self.assertTrue(is_version_compatible("1.0.0", "0.1.0"))
        self.assertTrue(is_version_compatible("v0.1.5", "0.1.0"))
        self.assertFalse(is_version_compatible("0.0.9", "0.1.0"))
        self.assertFalse(is_version_compatible("0.0.1", "0.1.0"))

    def test_get_opencode_binary_path_custom_setting(self):
        mock_settings = MagicMock()
        mock_settings.get.side_effect = lambda key, default=None: (
            "/opt/custom/bin/opencode" if key in (SETTING_OPENCODE_BINARY_PATH, "opencode_binary_path") else default
        )

        with patch("os.path.exists", return_value=True):
            path = get_opencode_binary_path(mock_settings)
            self.assertEqual(path, "/opt/custom/bin/opencode")

    def test_get_opencode_binary_path_native_which(self):
        mock_settings = MagicMock()
        mock_settings.get.return_value = ""

        with patch("shutil.which", side_effect=lambda cmd: "/usr/bin/opencode" if cmd == "opencode" else None):
            path = get_opencode_binary_path(mock_settings)
            self.assertEqual(path, "/usr/bin/opencode")

    def test_get_opencode_binary_path_fallback_candidate(self):
        mock_settings = MagicMock()
        mock_settings.get.return_value = ""

        def fake_exists(p):
            return str(p) == "/usr/local/bin/opencode"

        with patch("shutil.which", return_value=None), \
             patch("os.path.exists", side_effect=fake_exists), \
             patch("os.access", return_value=True):
            path = get_opencode_binary_path(mock_settings)
            self.assertEqual(path, "/usr/local/bin/opencode")

    def test_get_opencode_binary_path_flatpak_host(self):
        mock_settings = MagicMock()
        mock_settings.get.return_value = ""

        with patch("onyxsh.utils.opencode_utils.is_flatpak_sandbox", return_value=True), \
             patch("shutil.which", return_value=None), \
             patch("os.path.exists", return_value=False), \
             patch("subprocess.check_output", return_value=b"/home/user/.local/bin/opencode\n"):
            path = get_opencode_binary_path(mock_settings)
            self.assertEqual(path, "/home/user/.local/bin/opencode")

    def test_get_opencode_binary_path_not_found(self):
        mock_settings = MagicMock()
        mock_settings.get.return_value = ""

        with patch("onyxsh.utils.opencode_utils.is_flatpak_sandbox", return_value=False), \
             patch("shutil.which", return_value=None), \
             patch("os.path.exists", return_value=False):
            path = get_opencode_binary_path(mock_settings)
            self.assertIsNone(path)

    def test_get_opencode_version_success(self):
        with patch("onyxsh.utils.opencode_utils.get_opencode_binary_path", return_value="/usr/bin/opencode"), \
             patch("subprocess.check_output", return_value=b"opencode version 0.4.1\n"):
            version = get_opencode_version("/usr/bin/opencode")
            self.assertEqual(version, "0.4.1")

    def test_get_opencode_version_failure(self):
        with patch("onyxsh.utils.opencode_utils.get_opencode_binary_path", return_value="/usr/bin/opencode"), \
             patch("subprocess.check_output", side_effect=Exception("Execution failed")):
            version = get_opencode_version("/usr/bin/opencode")
            self.assertIsNone(version)

    def test_is_opencode_available_disabled_in_settings(self):
        mock_settings = MagicMock()
        mock_settings.get.side_effect = lambda key, default=None: (
            False if key in (SETTING_OPENCODE_ENABLED, "opencode_enabled") else default
        )
        self.assertFalse(is_opencode_available(mock_settings))

    def test_is_opencode_available_and_caching(self):
        mock_settings = MagicMock()
        mock_settings.get.side_effect = lambda key, default=None: (
            True if key in (SETTING_OPENCODE_ENABLED, "opencode_enabled") else default
        )

        with patch("onyxsh.utils.opencode_utils.get_opencode_binary_path", return_value="/usr/bin/opencode") as mock_find:
            # First check (cache miss)
            self.assertTrue(is_opencode_available(mock_settings))
            self.assertEqual(mock_find.call_count, 1)

            # Second check (cache hit, should not invoke get_opencode_binary_path again)
            self.assertTrue(is_opencode_available(mock_settings))
            self.assertEqual(mock_find.call_count, 1)

            # Force refresh bypasses cache
            self.assertTrue(is_opencode_available(mock_settings, force_refresh=True))
            self.assertEqual(mock_find.call_count, 2)

    def test_is_opencode_available_version_validation(self):
        mock_settings = MagicMock()
        mock_settings.get.return_value = True

        with patch("onyxsh.utils.opencode_utils.get_opencode_binary_path", return_value="/usr/bin/opencode"), \
             patch("onyxsh.utils.opencode_utils.get_opencode_version", return_value="0.0.5"):
            self.assertFalse(is_opencode_available(mock_settings, force_refresh=True, validate_version=True))

        with patch("onyxsh.utils.opencode_utils.get_opencode_binary_path", return_value="/usr/bin/opencode"), \
             patch("onyxsh.utils.opencode_utils.get_opencode_version", return_value="0.2.0"):
            self.assertTrue(is_opencode_available(mock_settings, force_refresh=True, validate_version=True))

    def test_build_opencode_command_native(self):
        with patch("onyxsh.utils.opencode_utils.is_flatpak_sandbox", return_value=False), \
             patch("onyxsh.utils.opencode_utils.get_opencode_binary_path", return_value="/usr/bin/opencode"):
            cmd = build_opencode_command()
            self.assertEqual(cmd, ["/usr/bin/opencode"])

            cmd_sub = build_opencode_command(subcommand="serve", args=["--port", "8080"])
            self.assertEqual(cmd_sub, ["/usr/bin/opencode", "serve", "--port", "8080"])

    def test_launch_mode_setting(self):
        self.assertEqual(SETTING_OPENCODE_LAUNCH_MODE, "opencode.launch_mode")

    def test_command_builder_wrap_host_command(self):
        platform_info = PlatformInfo()
        builder = CommandBuilder(platform_info)

        raw_cmd = ["opencode", "run", "hello"]

        # Native environment
        with patch("onyxsh.utils.platform.is_flatpak_sandbox", return_value=False):
            wrapped = builder.wrap_host_command(raw_cmd, working_directory="/tmp")
            self.assertEqual(wrapped, raw_cmd)

        # Flatpak with host-spawn
        with patch("onyxsh.utils.platform.is_flatpak_sandbox", return_value=True), \
             patch("shutil.which", side_effect=lambda x: "/usr/bin/host-spawn" if x == "host-spawn" else None), \
             patch("os.path.exists", return_value=True):
            wrapped = builder.wrap_host_command(
                raw_cmd, working_directory="/home/user/project", env={"FOO": "bar"}
            )
            self.assertEqual(
                wrapped,
                [
                    "/usr/bin/host-spawn",
                    "-cwd",
                    "/home/user/project",
                    "--env",
                    "FOO=bar",
                    "opencode",
                    "run",
                    "hello",
                ],
            )

        # Flatpak fallback with flatpak-spawn
        with patch("onyxsh.utils.platform.is_flatpak_sandbox", return_value=True), \
             patch("shutil.which", side_effect=lambda x: "/usr/bin/flatpak-spawn" if x == "flatpak-spawn" else None), \
             patch("os.path.exists", return_value=False):
            wrapped = builder.wrap_host_command(
                raw_cmd, working_directory="/home/user/project", env={"FOO": "bar"}
            )
            self.assertEqual(
                wrapped,
                [
                    "flatpak-spawn",
                    "--host",
                    "--watch-bus",
                    "--directory=/home/user/project",
                    "--env=FOO=bar",
                    "opencode",
                    "run",
                    "hello",
                ],
            )

    def test_get_opencode_install_command(self):
        cmd = get_opencode_install_command()
        self.assertEqual(cmd, OPENCODE_INSTALL_CMD)


if __name__ == "__main__":
    unittest.main()
