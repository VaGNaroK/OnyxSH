"""
Unit tests validating clean removal of Keyboard Copy Mode.

Ensures:
1. Loading an existing settings.json with 'toggle-keyboard-copy-mode' purges the obsolete shortcut without error.
2. Default shortcuts do not contain 'toggle-keyboard-copy-mode' or '<Control><Shift>space'.
3. The copy_mode module is completely absent from onyxsh.terminal.
4. TerminalManager and WindowActions have no remaining references or methods for copy mode.
5. Selection mode (Ctrl+Shift+S) remains fully functional and intact.
6. The window.css stylesheet retains selection-mode styles while removing copy-mode styles.
"""

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock

from onyxsh.settings.config import DefaultSettings
from onyxsh.settings.manager import SettingsManager
from onyxsh.terminal.manager import TerminalManager
from onyxsh.ui.actions import WindowActions


class TestKeyboardCopyModeRemoved(unittest.TestCase):
    def test_legacy_settings_with_copy_mode_purges_cleanly(self):
        """Ensure loading legacy settings with toggle-keyboard-copy-mode auto-purges it."""
        with tempfile.TemporaryDirectory() as tmpdir:
            settings_path = Path(tmpdir) / "settings.json"
            legacy_data = {
                "metadata": {
                    "version": "0.13.0",
                    "created_at": 1728000000.0,
                    "modified_at": 1728000000.0,
                    "checksum": "",
                },
                "settings": {
                    "shortcuts": {
                        "toggle-selection-mode": "<Control><Shift>s",
                        "toggle-keyboard-copy-mode": "<Control><Shift>space",
                        "quit": "<Control><Shift>q",
                    }
                },
            }
            with open(settings_path, "w", encoding="utf-8") as f:
                json.dump(legacy_data, f)

            manager = SettingsManager(settings_file=settings_path)
            self.assertTrue(manager.wait_for_pending_saves())

            # toggle-keyboard-copy-mode must be purged
            self.assertNotIn("toggle-keyboard-copy-mode", manager._settings["shortcuts"])
            # Selection mode and other shortcuts must be intact
            self.assertEqual(
                manager._settings["shortcuts"].get("toggle-selection-mode"),
                "<Control><Shift>s",
            )
            self.assertEqual(
                manager._settings["shortcuts"].get("quit"),
                "<Control><Shift>q",
            )

    def test_defaults_do_not_contain_keyboard_copy_mode(self):
        """Ensure default shortcuts do not have toggle-keyboard-copy-mode."""
        defaults = DefaultSettings.get_defaults()
        shortcuts = defaults.get("shortcuts", {})
        self.assertNotIn("toggle-keyboard-copy-mode", shortcuts)
        # Verify <Control><Shift>space is free
        self.assertNotIn("<Control><Shift>space", shortcuts.values())
        # Verify toggle-selection-mode is still present
        self.assertEqual(shortcuts.get("toggle-selection-mode"), "<Control><Shift>s")

    def test_copy_mode_module_is_removed(self):
        """Ensure importing copy_mode raises ModuleNotFoundError."""
        with self.assertRaises(ModuleNotFoundError):
            import onyxsh.terminal.copy_mode  # noqa: F401

    def test_terminal_manager_and_actions_have_no_copy_mode_methods(self):
        """Ensure TerminalManager and WindowActions have no copy mode methods."""
        tm = TerminalManager.__new__(TerminalManager)
        self.assertFalse(hasattr(tm, "get_terminal_copy_mode_controller"))
        self.assertFalse(hasattr(tm, "set_terminal_keyboard_copy_mode"))
        self.assertFalse(hasattr(tm, "toggle_terminal_keyboard_copy_mode"))

        mock_window = MagicMock()
        actions = WindowActions(mock_window)
        self.assertFalse(hasattr(actions, "toggle_keyboard_copy_mode"))

        actions.setup_actions()
        registered_action_names = [
            call.args[0].get_name()
            for call in mock_window.add_action.call_args_list
        ]
        self.assertNotIn("toggle-keyboard-copy-mode", registered_action_names)
        self.assertIn("toggle-selection-mode", registered_action_names)

    def test_no_copy_mode_leftovers_in_src(self):
        """Ensure no python files in src/onyxsh reference copy_mode or toggle-keyboard-copy-mode."""
        src_root = Path(__file__).resolve().parent.parent / "src" / "onyxsh"
        forbidden_tokens = [
            "toggle-keyboard-copy-mode",
            "KeyboardCopyModeController",
            "copy_mode_btn",
        ]
        violations = []
        for py_file in src_root.rglob("*.py"):
            content = py_file.read_text(encoding="utf-8")
            for token in forbidden_tokens:
                if token in content:
                    violations.append(f"{py_file.name}: contains {token}")
        self.assertEqual(violations, [])

    def test_window_css_retains_selection_mode_and_removes_copy_mode(self):
        """Ensure window.css keeps selection-mode-active and drops copy-mode-active / copy-mode-pill."""
        css_path = (
            Path(__file__).resolve().parent.parent
            / "src"
            / "onyxsh"
            / "data"
            / "styles"
            / "window.css"
        )
        css_content = css_path.read_text(encoding="utf-8")
        self.assertIn(".terminal-pane.selection-mode-active", css_content)
        self.assertNotIn("copy-mode-active", css_content)
        self.assertNotIn("terminal-copy-mode-active", css_content)
        self.assertNotIn("copy-mode-pill", css_content)


if __name__ == "__main__":
    unittest.main()
