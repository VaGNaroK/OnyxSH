# tests/test_settings_shortcuts_repair.py
"""Unit tests for keyboard shortcuts validation, deduplication, and repair."""

import json
import tempfile
import unittest
from pathlib import Path

from onyxsh.settings.config import DefaultSettings
from onyxsh.settings.manager import SettingsManager, SettingsValidator


class TestSettingsShortcutsRepair(unittest.TestCase):
    """Test suite covering shortcut validation, deduplication, and automatic repair."""

    def setUp(self):
        self.validator = SettingsValidator()

    def test_validate_shortcuts_valid(self):
        """Test that valid shortcuts dictionary without duplicates passes validation."""
        valid_shortcuts = {
            "copy": "<Control><Shift>c",
            "paste": "<Control><Shift>v",
            "quit": "<Control><Shift>q",
            "unassigned": "",
        }
        errors = self.validator.validate_shortcuts(valid_shortcuts)
        self.assertEqual(errors, [])

    def test_validate_shortcuts_duplicate_detection(self):
        """Test that duplicate shortcuts are detected with descriptive error reporting."""
        duplicate_shortcuts = {
            "export-terminal-buffer": "<Control><Shift>s",
            "toggle-selection-mode": "<Control><Shift>s",
            "quit": "<Control><Shift>q",
        }
        errors = self.validator.validate_shortcuts(duplicate_shortcuts)
        self.assertTrue(any("Duplicate keyboard shortcuts detected" in e for e in errors))
        self.assertTrue(any("<Control><Shift>s" in e for e in errors))
        self.assertTrue(any("export-terminal-buffer" in e and "toggle-selection-mode" in e for e in errors))

    def test_validate_shortcuts_invalid_accelerator(self):
        """Test that invalid accelerator syntax is reported."""
        invalid_shortcuts = {
            "copy": "InvalidKeyCombo123456",
        }
        errors = self.validator.validate_shortcuts(invalid_shortcuts)
        self.assertTrue(any("Invalid shortcut for action 'copy'" in e for e in errors))

    def test_repair_shortcuts_removes_obsolete_actions(self):
        """Test that obsolete actions not in DefaultSettings are purged."""
        with tempfile.TemporaryDirectory() as tmpdir:
            settings_file = Path(tmpdir) / "settings.json"
            manager = SettingsManager(settings_file=settings_file)
            manager._settings["shortcuts"]["obsolete-action-foo"] = "<Control><Shift>z"
            manager._settings["shortcuts"]["export-terminal-buffer"] = "<Control><Shift>s"

            repairs = manager._repair_shortcuts()
            self.assertGreater(repairs, 0)
            self.assertNotIn("obsolete-action-foo", manager._settings["shortcuts"])
            self.assertNotIn("export-terminal-buffer", manager._settings["shortcuts"])

    def test_repair_shortcuts_deduplicates_colliding_accelerators(self):
        """Test that colliding accelerators are deduplicated cleanly."""
        with tempfile.TemporaryDirectory() as tmpdir:
            settings_file = Path(tmpdir) / "settings.json"
            manager = SettingsManager(settings_file=settings_file)

            # Force two valid actions to share the same accelerator
            manager._settings["shortcuts"]["copy"] = "<Control><Shift>k"
            manager._settings["shortcuts"]["close-pane"] = "<Control><Shift>k"  # default for close-pane

            repairs = manager._repair_shortcuts()
            self.assertGreater(repairs, 0)

            # close-pane should retain its default <Control><Shift>k, while copy should be reassigned or unassigned
            self.assertEqual(manager._settings["shortcuts"]["close-pane"], "<Control><Shift>k")
            self.assertNotEqual(manager._settings["shortcuts"]["copy"], "<Control><Shift>k")

            # Validate that no duplicates remain
            errors = manager.validator.validate_shortcuts(manager._settings["shortcuts"])
            self.assertEqual(errors, [])

    def test_merge_with_defaults_purges_obsolete_conflicts(self):
        """
        Simulate user scenario: legacy settings file has 'export-terminal-buffer': '<Control><Shift>s',
        and defaults introduce 'toggle-selection-mode': '<Control><Shift>s'.
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            settings_file = Path(tmpdir) / "settings.json"
            manager = SettingsManager(settings_file=settings_file)

            # Prepare a settings state without toggle-selection-mode and with obsolete export-terminal-buffer
            manager._settings["shortcuts"].pop("toggle-selection-mode", None)
            manager._settings["shortcuts"]["export-terminal-buffer"] = "<Control><Shift>s"

            manager._merge_with_defaults()

            # The obsolete key should have been purged, and toggle-selection-mode assigned
            self.assertNotIn("export-terminal-buffer", manager._settings["shortcuts"])
            self.assertEqual(manager._settings["shortcuts"]["toggle-selection-mode"], "<Control><Shift>s")

            errors = manager.validator.validate_shortcuts(manager._settings["shortcuts"])
            self.assertEqual(errors, [])

    def test_load_legacy_settings_with_duplicate_shortcuts_auto_heals(self):
        """
        Ensure loading an existing settings.json with duplicate shortcuts
        auto-repairs on initialization and saves without errors.
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            settings_file = Path(tmpdir) / "settings.json"

            # Create an initial settings.json replicating user's exact state
            legacy_content = {
                "metadata": {
                    "version": "0.13.0",
                    "created_at": 1728000000.0,
                    "modified_at": 1728000000.0,
                    "checksum": "",
                },
                "settings": {
                    "color_scheme": 0,
                    "transparency": 0,
                    "font": "Monospace 12",
                    "sidebar_visible": True,
                    "auto_hide_sidebar": False,
                    "scroll_on_output": False,
                    "scroll_on_keystroke": True,
                    "mouse_autohide": False,
                    "bell_sound": False,
                    "log_to_file": False,
                    "ai_assistant_enabled": False,
                    "shortcuts": {
                        "export-terminal-buffer": "<Control><Shift>s",
                        "copy": "<Control><Shift>Insert",
                        "paste": "<Control><Shift>v",
                    },
                },
            }
            with open(settings_file, "w", encoding="utf-8") as f:
                json.dump(legacy_content, f)

            # Initialize manager from file
            manager = SettingsManager(settings_file=settings_file)

            # Should be clean of duplicates
            errors = manager.validator.validate_settings_structure(
                manager._settings, len(manager.get_scheme_order())
            )
            self.assertEqual(errors, [])
            self.assertNotIn("export-terminal-buffer", manager._settings["shortcuts"])
            self.assertEqual(manager._settings["shortcuts"]["toggle-selection-mode"], "<Control><Shift>s")

    def test_set_shortcut_unbinds_previous_action(self):
        """Test that set_shortcut unassigns any duplicate from another action."""
        with tempfile.TemporaryDirectory() as tmpdir:
            settings_file = Path(tmpdir) / "settings.json"
            manager = SettingsManager(settings_file=settings_file)

            # Assign shortcut of 'quit' (<Control><Shift>q) to 'copy'
            manager.set_shortcut("copy", "<Control><Shift>q")

            # 'copy' should have it, and 'quit' should be unassigned ("")
            self.assertEqual(manager.get_shortcut("copy"), "<Control><Shift>q")
            self.assertEqual(manager.get_shortcut("quit"), "")

            errors = manager.validator.validate_shortcuts(manager._settings["shortcuts"])
            self.assertEqual(errors, [])
            import time
            time.sleep(0.05)


if __name__ == "__main__":
    unittest.main()
