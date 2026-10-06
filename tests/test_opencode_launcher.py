# tests/test_opencode_launcher.py

import unittest
from unittest.mock import MagicMock, patch

from onyxsh.settings.config import DefaultSettings
from onyxsh.terminal.spawner import ProcessTracker
from onyxsh.ui.actions import WindowActions
from onyxsh.ui.dialogs.command_palette_dialog import CommandPaletteDialog, CommandPaletteItem
from onyxsh.ui.dialogs.shortcuts_dialog import SHORTCUT_DATA


class TestOpenCodeLauncher(unittest.TestCase):
    """Test suite for OpenCode TUI launcher, tab/split integration, and process tracking."""

    def setUp(self):
        self.mock_window = MagicMock()
        self.mock_window.settings_manager = MagicMock()
        self.mock_window.tab_manager = MagicMock()
        self.mock_window.terminal_manager = MagicMock()
        self.mock_window.toast_overlay = MagicMock()

    def test_opencode_launcher_action_registered(self):
        actions = WindowActions(self.mock_window)
        # Check method exists on WindowActions
        self.assertTrue(hasattr(actions, "opencode_launcher"))
        self.assertTrue(callable(actions.opencode_launcher))

        # Check default shortcut configured
        defaults = DefaultSettings.get_defaults()
        self.assertIn("opencode-launcher", defaults["shortcuts"])
        self.assertEqual(defaults["shortcuts"]["opencode-launcher"], "<Control><Shift>o")

        # Check present in shortcuts help dialog data
        all_shortcuts = []
        for group in SHORTCUT_DATA:
            for item in group["shortcuts"]:
                all_shortcuts.append(item[0])
        self.assertIn("opencode-launcher", all_shortcuts)

    @patch("onyxsh.utils.opencode_utils.is_opencode_available", return_value=False)
    @patch("onyxsh.utils.opencode_utils.get_opencode_install_command", return_value="curl -fsSL https://opencode.ai/install | bash")
    def test_opencode_launcher_when_not_available_shows_toast(self, _mock_cmd, _mock_avail):
        actions = WindowActions(self.mock_window)
        actions.opencode_launcher()

        # Should show toast message and not launch
        self.mock_window.toast_overlay.add_toast.assert_called_once()
        self.mock_window.tab_manager.split_vertical.assert_not_called()
        self.mock_window.tab_manager.create_local_tab.assert_not_called()

    @patch("onyxsh.utils.opencode_utils.get_opencode_binary_path", return_value="opencode")
    @patch("onyxsh.utils.opencode_utils.is_opencode_available", return_value=True)
    def test_opencode_launcher_split_with_osc7_pwd(self, _mock_avail, _mock_bin):
        mock_terminal = MagicMock()
        mock_terminal.get_current_directory_uri.return_value = "file:///home/developer/workspace/project"
        self.mock_window.tab_manager.get_selected_terminal.return_value = mock_terminal

        actions = WindowActions(self.mock_window)
        actions.opencode_launcher(mode="split")

        # Should split vertically with detected PWD and execute_command='opencode'
        self.mock_window.tab_manager.split_vertical.assert_called_once_with(
            mock_terminal,
            working_directory="/home/developer/workspace/project",
            execute_command="opencode",
            title="OpenCode",
        )

    @patch("onyxsh.utils.opencode_utils.get_opencode_binary_path", return_value="opencode")
    @patch("onyxsh.utils.opencode_utils.is_opencode_available", return_value=True)
    def test_opencode_launcher_new_tab_fallback_when_no_active_terminal(self, _mock_avail, _mock_bin):
        self.mock_window.tab_manager.get_selected_terminal.return_value = None

        actions = WindowActions(self.mock_window)
        actions.opencode_launcher(mode="split")

        # Fallback to create_local_tab
        self.mock_window.tab_manager.create_local_tab.assert_called_once_with(
            title="OpenCode",
            working_directory=None,
            execute_command="opencode",
        )

    @patch("onyxsh.utils.opencode_utils.get_opencode_binary_path", return_value="opencode")
    @patch("onyxsh.utils.opencode_utils.is_opencode_available", return_value=True)
    def test_opencode_launcher_tab_mode_explicit(self, _mock_avail, _mock_bin):
        mock_terminal = MagicMock()
        mock_terminal.get_current_directory_uri.return_value = "file:///home/developer/docs"
        self.mock_window.tab_manager.get_selected_terminal.return_value = mock_terminal

        actions = WindowActions(self.mock_window)
        actions.opencode_launcher(mode="tab")

        self.mock_window.tab_manager.create_local_tab.assert_called_once_with(
            title="OpenCode",
            working_directory="/home/developer/docs",
            execute_command="opencode",
        )

    def test_command_palette_catalog_and_dispatch(self):
        actions = WindowActions(self.mock_window)
        self.mock_window.action_handler = actions

        dialog = CommandPaletteDialog.__new__(CommandPaletteDialog)
        dialog.parent_window = self.mock_window
        dialog.logger = MagicMock()
        dialog._items = []

        with patch("onyxsh.utils.opencode_utils.is_opencode_available", return_value=True):
            with patch.object(actions, "opencode_launcher") as mock_launcher:
                item = CommandPaletteItem(
                    item_id="opencode-launcher",
                    title="OpenCode",
                    category="Assistente de IA",
                    icon_name="utilities-terminal-symbolic",
                    action_name="opencode-launcher",
                )
                dialog._execute_item(item)
                mock_launcher.assert_called_once()

    def test_process_tracker_multi_instance_resilience(self):
        tracker = ProcessTracker()
        pid1 = 20001
        pid2 = 20002

        tracker.register_process(pid1, {"name": "OpenCode-Tab1", "type": "local"})
        tracker.register_process(pid2, {"name": "OpenCode-Tab2", "type": "local"})

        self.assertIn(pid1, tracker._processes)
        self.assertIn(pid2, tracker._processes)

        # Terminate pid1 gracefully
        with patch("os.kill") as mock_kill:
            tracker.terminate_process(pid1)
            mock_kill.assert_called_once()

        self.assertNotIn(pid1, tracker._processes)
        self.assertIn(pid2, tracker._processes)

        # Unregister pid2
        self.assertTrue(tracker.unregister_process(pid2))
        self.assertEqual(len(tracker._processes), 0)


if __name__ == "__main__":
    unittest.main()
