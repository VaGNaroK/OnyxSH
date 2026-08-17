# tests/test_desktop_notifier.py
import unittest
from unittest.mock import MagicMock

from onyxsh.settings.manager import SettingsManager
from onyxsh.terminal.desktop_notifier import DesktopNotifier
from onyxsh.terminal.semantic_tracker import SemanticCommand


class DummySettings(SettingsManager):
    def __init__(self, initial_data=None):
        self._data = initial_data or {}

    def get(self, key, default=None):
        return self._data.get(key, default)

    def set(self, key, value):
        self._data[key] = value


class TestDesktopNotifier(unittest.TestCase):
    def setUp(self):
        self.settings = DummySettings({
            "notify_long_commands": True,
            "notify_long_commands_threshold": 10,
            "notify_long_commands_condition": "unfocused",
            "notify_long_commands_sound": False,
        })
        self.notifier = DesktopNotifier(settings_manager=self.settings)
        self.dummy_terminal = MagicMock()
        self.dummy_terminal.terminal_id = 42

    def test_should_not_notify_when_disabled(self):
        self.settings.set("notify_long_commands", False)
        cmd = SemanticCommand(
            command_id="c1",
            command_text="sleep 20",
            duration=20.0,
            exit_code=0,
        )
        self.assertFalse(self.notifier.should_notify(self.dummy_terminal, cmd))

    def test_should_not_notify_when_below_threshold(self):
        cmd = SemanticCommand(
            command_id="c2",
            command_text="ls -la",
            duration=3.5,
            exit_code=0,
        )
        self.assertFalse(self.notifier.should_notify(self.dummy_terminal, cmd))

    def test_should_notify_when_always_condition(self):
        self.settings.set("notify_long_commands_condition", "always")
        cmd = SemanticCommand(
            command_id="c3",
            command_text="make build",
            duration=15.0,
            exit_code=0,
        )
        self.assertTrue(self.notifier.should_notify(self.dummy_terminal, cmd))

    def test_should_notify_when_window_unfocused(self):
        window = MagicMock()
        window.is_active.return_value = False
        window.tab_manager.get_selected_terminal.return_value = self.dummy_terminal

        cmd = SemanticCommand(
            command_id="c4",
            command_text="docker build .",
            duration=25.0,
            exit_code=0,
        )
        self.assertTrue(
            self.notifier.should_notify(self.dummy_terminal, cmd, window=window)
        )

    def test_should_notify_when_tab_inactive(self):
        window = MagicMock()
        window.is_active.return_value = True
        other_terminal = MagicMock()
        other_terminal.terminal_id = 99
        window.tab_manager.get_selected_terminal.return_value = other_terminal

        cmd = SemanticCommand(
            command_id="c5",
            command_text="npm test",
            duration=18.0,
            exit_code=0,
        )
        self.assertTrue(
            self.notifier.should_notify(self.dummy_terminal, cmd, window=window)
        )

    def test_should_not_notify_when_window_and_tab_focused(self):
        window = MagicMock()
        window.is_active.return_value = True
        window.get_mapped.return_value = True
        self.dummy_terminal.has_focus.return_value = True
        window.tab_manager.get_selected_terminal.return_value = self.dummy_terminal

        cmd = SemanticCommand(
            command_id="c6",
            command_text="sleep 12",
            duration=12.0,
            exit_code=0,
        )
        self.assertFalse(
            self.notifier.should_notify(self.dummy_terminal, cmd, window=window)
        )


if __name__ == "__main__":
    unittest.main()
