# tests/test_terminal_error_suggestions.py
"""Unit tests for Proactive Terminal Error Suggestions & Quick Actions."""

import unittest
from unittest.mock import MagicMock, patch

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
gi.require_version("Vte", "3.91")
from gi.repository import Adw, Gio, GLib, Gtk, Vte

from onyxsh.agent.error_matcher import (
    ErrorCategory,
    ErrorMatch,
    TerminalErrorMatcher,
    get_terminal_error_matcher,
)
from onyxsh.terminal.manager import TerminalManager
from onyxsh.terminal.semantic_tracker import SemanticCommand


class TestTerminalErrorMatcher(unittest.TestCase):
    def setUp(self):
        self.matcher = get_terminal_error_matcher()

    def test_exit_code_zero_returns_none(self):
        result = self.matcher.match("ls -la", 0, "total 0")
        self.assertIsNone(result)

    def test_permission_denied_without_sudo(self):
        output = "apt: E: Could not open lock file /var/lib/dpkg/lock-frontend - open (13: Permission denied)"
        match = self.matcher.match("apt update", 100, output)
        self.assertIsNotNone(match)
        self.assertEqual(match.category, ErrorCategory.PERMISSION_DENIED)
        self.assertTrue(match.has_quick_fix)
        self.assertEqual(match.quick_fix_command, "sudo apt update")
        self.assertIn("sudo", match.quick_fix_label.lower())

    def test_permission_denied_portuguese(self):
        output = "bash: /etc/shadow: Permissão negada"
        match = self.matcher.match("cat /etc/shadow", 1, output)
        self.assertIsNotNone(match)
        self.assertEqual(match.category, ErrorCategory.PERMISSION_DENIED)
        self.assertEqual(match.quick_fix_command, "sudo cat /etc/shadow")

    def test_permission_denied_already_sudo(self):
        output = "sudo: /root/secret: Permission denied"
        match = self.matcher.match("sudo cat /root/secret", 1, output)
        self.assertIsNotNone(match)
        self.assertEqual(match.category, ErrorCategory.PERMISSION_DENIED)
        # Should not prefix sudo again
        self.assertIsNone(match.quick_fix_command)

    def test_command_not_found_extraction(self):
        output = "bash: htop: command not found"
        match = self.matcher.match("htop", 127, output)
        self.assertIsNotNone(match)
        self.assertEqual(match.category, ErrorCategory.COMMAND_NOT_FOUND)
        self.assertEqual(match.extracted_target, "htop")
        self.assertEqual(match.quick_fix_command, "apt search htop")

    def test_command_not_found_zsh(self):
        output = "zsh: command not found: ripgrep"
        match = self.matcher.match("ripgrep pattern", 127, output)
        self.assertIsNotNone(match)
        self.assertEqual(match.category, ErrorCategory.COMMAND_NOT_FOUND)
        self.assertEqual(match.extracted_target, "ripgrep")
        self.assertEqual(match.quick_fix_command, "apt search ripgrep")

    def test_port_already_in_use_extraction(self):
        output = "listen tcp :8080: bind: address already in use"
        match = self.matcher.match("docker run -p 8080:80 nginx", 1, output)
        self.assertIsNotNone(match)
        self.assertEqual(match.category, ErrorCategory.PORT_IN_USE)
        self.assertEqual(match.extracted_target, "8080")
        self.assertEqual(match.quick_fix_command, "lsof -i :8080")

    def test_port_in_use_from_command(self):
        output = "Error: bind: Address already in use"
        match = self.matcher.match("python3 -m http.server 3000", 1, output)
        self.assertIsNotNone(match)
        self.assertEqual(match.category, ErrorCategory.PORT_IN_USE)
        self.assertEqual(match.extracted_target, "3000")
        self.assertEqual(match.quick_fix_command, "lsof -i :3000")

    def test_no_space_left_on_device(self):
        output = "cp: error writing '/var/backup.tar': No space left on device"
        match = self.matcher.match("cp backup.tar /var/", 1, output)
        self.assertIsNotNone(match)
        self.assertEqual(match.category, ErrorCategory.NO_SPACE_LEFT)
        self.assertEqual(match.quick_fix_command, "df -h")

    def test_python_missing_module(self):
        output = "Traceback (most recent call last):\nModuleNotFoundError: No module named 'requests'"
        match = self.matcher.match("python3 app.py", 1, output)
        self.assertIsNotNone(match)
        self.assertEqual(match.category, ErrorCategory.PACKAGE_MISSING)
        self.assertEqual(match.extracted_target, "requests")
        self.assertEqual(match.quick_fix_command, "pip install requests")

    def test_node_missing_module(self):
        output = "Error: Cannot find module 'express'\nRequire stack:\n- /app/server.js"
        match = self.matcher.match("node server.js", 1, output)
        self.assertIsNotNone(match)
        self.assertEqual(match.category, ErrorCategory.PACKAGE_MISSING)
        self.assertEqual(match.extracted_target, "express")
        self.assertEqual(match.quick_fix_command, "npm install express")

    def test_file_not_found(self):
        output = "cat: /tmp/missing.txt: No such file or directory"
        match = self.matcher.match("cat /tmp/missing.txt", 1, output)
        self.assertIsNotNone(match)
        self.assertEqual(match.category, ErrorCategory.FILE_NOT_FOUND)
        self.assertEqual(match.quick_fix_command, "ls -la")

    def test_permission_denied_heuristic_without_output(self):
        """Test cat /etc/shadow without output triggers permission denied via root target heuristic."""
        match = self.matcher.match("cat /etc/shadow", 1, "")
        self.assertIsNotNone(match)
        self.assertEqual(match.category, ErrorCategory.PERMISSION_DENIED)
        self.assertEqual(match.quick_fix_command, "sudo cat /etc/shadow")

    def test_python_missing_module_traceback_multiline(self):
        """Test python multiline traceback matching ModuleNotFoundError."""
        output = (
            "Traceback (most recent call last):\n"
            '  File "<string>", line 1, in <module>\n'
            "ModuleNotFoundError: No module named 'modulo_inexistente_xyz'"
        )
        match = self.matcher.match('python3 -c "import modulo_inexistente_xyz"', 1, output)
        self.assertIsNotNone(match)
        self.assertEqual(match.category, ErrorCategory.PACKAGE_MISSING)
        self.assertEqual(match.extracted_target, "modulo_inexistente_xyz")
        self.assertEqual(match.quick_fix_command, "pip install modulo_inexistente_xyz")

    def test_python_missing_module_command_heuristic_without_output(self):
        """Test python -c 'import X' failing without output triggers missing package heuristic."""
        match = self.matcher.match('python3 -c "import modulo_inexistente_xyz"', 1, "")
        self.assertIsNotNone(match)
        self.assertEqual(match.category, ErrorCategory.PACKAGE_MISSING)
        self.assertEqual(match.extracted_target, "modulo_inexistente_xyz")
        self.assertEqual(match.quick_fix_command, "pip install modulo_inexistente_xyz")

    def test_connection_refused_curl_failed_to_connect_no_explicit_refused(self):
        """Test curl (7) Failed to connect / Couldn't connect to server without explicit 'Connection refused'."""
        output = "curl: (7) Failed to connect to localhost port 9999 after 0 ms: Couldn't connect to server"
        match = self.matcher.match("curl -I http://localhost:9999", 7, output)
        self.assertIsNotNone(match)
        self.assertEqual(match.category, ErrorCategory.CONNECTION_REFUSED)
        self.assertEqual(match.extracted_target, "9999")
        self.assertIsNotNone(match.quick_fix_command)
        self.assertIn("9999", match.quick_fix_command)

    def test_connection_refused(self):
        output = "curl: (7) Failed to connect to localhost port 9000: Connection refused"
        match = self.matcher.match("curl localhost:9000", 7, output)
        self.assertIsNotNone(match)
        self.assertEqual(match.category, ErrorCategory.CONNECTION_REFUSED)
        self.assertIn("Connection refused", match.ai_prompt_hint)

    def test_git_error(self):
        output = "error: Your local changes to the following files would be overwritten by merge:\n  file.py"
        match = self.matcher.match("git pull origin main", 1, output)
        self.assertIsNotNone(match)
        self.assertEqual(match.category, ErrorCategory.GIT_ERROR)
        self.assertEqual(match.quick_fix_command, "git status")

    def test_generic_error_fallback(self):
        output = "Compilation failed: syntax error at line 42"
        match = self.matcher.match("make", 2, output)
        self.assertIsNotNone(match)
        self.assertEqual(match.category, ErrorCategory.GENERIC_ERROR)
        self.assertFalse(match.has_quick_fix)
        self.assertIn("make", match.ai_prompt_hint)


class TestTerminalActionsAndIntegration(unittest.TestCase):
    def setUp(self):
        self.window = MagicMock()
        self.window.toast_overlay = MagicMock()
        self.window.tab_manager = MagicMock()
        self.window.terminal_manager = MagicMock()
        self.window.terminal_manager.settings_manager = MagicMock()
        self.window.ui_builder = MagicMock()

        from onyxsh.ui.actions import WindowActions

        self.actions = WindowActions(self.window)

    def test_apply_terminal_quick_fix_prompt_insertion(self):
        terminal = MagicMock()
        self.actions.apply_terminal_quick_fix(
            terminal=terminal, command_str="sudo apt update", auto_execute=False
        )
        if hasattr(terminal, "feed_child_binary") and terminal.feed_child_binary.called:
            terminal.feed_child_binary.assert_called_once_with(b"sudo apt update")
        else:
            terminal.feed_child.assert_called_once_with(b"sudo apt update")
        self.window.toast_overlay.add_toast.assert_called_once()

    def test_apply_terminal_quick_fix_auto_execute(self):
        terminal = MagicMock()
        self.actions.apply_terminal_quick_fix(
            terminal=terminal, command_str="df -h", auto_execute=True
        )
        if hasattr(terminal, "feed_child_binary") and terminal.feed_child_binary.called:
            terminal.feed_child_binary.assert_called_once_with(b"df -h\n")
        else:
            terminal.feed_child.assert_called_once_with(b"df -h\n")
        self.window.toast_overlay.add_toast.assert_called_once()

    def test_analyze_last_error_with_ai_formats_prompt(self):
        terminal = MagicMock()
        cmd = SemanticCommand(
            command_id="cmd_1",
            command_text="chmod 777 /etc",
            exit_code=1,
            cwd="/home/user",
        )
        self.window.terminal_manager.semantic_tracker.get_last_command.return_value = cmd
        self.window.terminal_manager.semantic_tracker.get_last_output_text.return_value = (
            "chmod: changing permissions of '/etc': Operation not permitted"
        )

        ai_panel = MagicMock()
        self.window.ui_builder.ai_chat_panel = ai_panel

        self.actions.analyze_last_error_with_ai(terminal=terminal)

        self.window.ui_builder.show_ai_panel.assert_called_once()
        ai_panel.send_message.assert_called_once()
        prompt_sent = ai_panel.send_message.call_args[0][0]
        self.assertIn("chmod 777 /etc", prompt_sent)
        self.assertIn("Operation not permitted", prompt_sent)
        self.assertIn("Permissão Negada", prompt_sent)


class TestTerminalManagerProactiveToast(unittest.TestCase):
    def test_show_error_suggestion_toast_creation_and_click(self):
        window = MagicMock()
        toast_overlay = MagicMock()
        window.toast_overlay = toast_overlay
        action_handler = MagicMock()
        window.action_handler = action_handler

        settings_manager = MagicMock()
        settings_manager.get.side_effect = lambda key, default=None: {
            "ai_proactive_error_suggestions": True,
            "ai_error_suggestion_mode": "toast_and_badge",
            "ai_error_auto_execute_quick_fix": False,
        }.get(key, default)

        tm = TerminalManager.__new__(TerminalManager)
        tm.parent_window = window
        tm.settings_manager = settings_manager
        tm.logger = MagicMock()

        terminal = MagicMock()
        cmd = SemanticCommand(command_id="test_1", command_text="apt update", exit_code=100)
        error_match = ErrorMatch(
            category=ErrorCategory.PERMISSION_DENIED,
            title="Permissão Negada",
            description="Requer root",
            quick_fix_command="sudo apt update",
            quick_fix_label="⚡ Executar com sudo",
        )

        tm._show_error_suggestion_toast(terminal, cmd, error_match)

        toast_overlay.add_toast.assert_called_once()
        created_toast = toast_overlay.add_toast.call_args[0][0]
        self.assertIsInstance(created_toast, Adw.Toast)
        self.assertEqual(created_toast.get_button_label(), "⚡ Executar com sudo")
        self.assertIn("Permissão Negada", created_toast.get_title())

        # Simulate clicking the toast action button
        created_toast.emit("button-clicked")
        action_handler.apply_terminal_quick_fix.assert_called_once_with(
            terminal=terminal,
            command_str="sudo apt update",
            auto_execute=False,
        )

    def test_proactive_toast_suppressed_when_disabled(self):
        window = MagicMock()
        window.toast_overlay = MagicMock()
        settings_manager = MagicMock()
        settings_manager.get.side_effect = lambda key, default=None: {
            "ai_proactive_error_suggestions": False,
            "ai_error_suggestion_mode": "toast_and_badge",
        }.get(key, default)

        tm = TerminalManager.__new__(TerminalManager)
        tm.parent_window = window
        tm.settings_manager = settings_manager
        tm.logger = MagicMock()
        tm.semantic_tracker = MagicMock()
        tm.tab_manager = MagicMock()

        terminal = MagicMock()
        terminal.onyxsh_session = None
        terminal.get_current_directory_uri.return_value = None
        cmd = SemanticCommand(command_id="test_2", command_text="apt update", exit_code=100, duration=0.0, cwd="")
        tm._on_semantic_command_finished(terminal, cmd)

        window.toast_overlay.add_toast.assert_not_called()

    def test_proactive_toast_suppressed_in_badge_only_mode(self):
        window = MagicMock()
        window.toast_overlay = MagicMock()
        settings_manager = MagicMock()
        settings_manager.get.side_effect = lambda key, default=None: {
            "ai_proactive_error_suggestions": True,
            "ai_error_suggestion_mode": "badge_only",
        }.get(key, default)

        tm = TerminalManager.__new__(TerminalManager)
        tm.parent_window = window
        tm.settings_manager = settings_manager
        tm.logger = MagicMock()
        tm.semantic_tracker = MagicMock()
        tm.tab_manager = MagicMock()

        terminal = MagicMock()
        terminal.onyxsh_session = None
        terminal.get_current_directory_uri.return_value = None
        cmd = SemanticCommand(command_id="test_3", command_text="apt update", exit_code=100, duration=0.0, cwd="")
        tm._on_semantic_command_finished(terminal, cmd)

        window.toast_overlay.add_toast.assert_not_called()


class TestTabManagerErrorBadge(unittest.TestCase):
    def test_update_semantic_badge_with_error_match(self):
        from onyxsh.terminal.tabs import TabManager

        tab_manager = TabManager.__new__(TabManager)
        terminal = MagicMock()
        page = MagicMock()
        tab_manager.get_page_for_terminal = MagicMock(return_value=page)
        tab_manager._find_pane_for_terminal = MagicMock(return_value=None)

        status_box = MagicMock()
        status_label = MagicMock()
        quick_fix_btn = MagicMock()
        ai_btn = MagicMock()
        copy_btn = MagicMock()

        terminal.semantic_status_box = status_box
        terminal.semantic_status_label = status_label
        terminal.semantic_quick_fix_btn = quick_fix_btn
        terminal.semantic_ai_btn = ai_btn
        terminal.semantic_copy_btn = copy_btn

        cmd = SemanticCommand(command_id="test_4", exit_code=1, duration=0.5)
        error_match = ErrorMatch(
            category=ErrorCategory.PERMISSION_DENIED,
            title="Permissão Negada",
            description="Requer root",
            quick_fix_command="sudo test",
            quick_fix_label="⚡ Executar com sudo",
        )

        tab_manager.update_semantic_badge_for_terminal(terminal, cmd, error_match=error_match)

        status_box.add_css_class.assert_called_with("error")
        status_label.set_tooltip_text.assert_called_with("Permissão Negada: Requer root")
        quick_fix_btn.set_visible.assert_called_with(True)
        ai_btn.set_visible.assert_called_with(True)



class TestSemanticTrackerFallbacks(unittest.TestCase):
    def test_extract_command_output_fallback_to_buffer(self):
        """Test extract_command_output falls back to get_text_format when text range is empty."""
        from onyxsh.terminal.semantic_tracker import SemanticTracker, SemanticCommand

        tracker = SemanticTracker()
        terminal = MagicMock()
        # Mock get_text_range_format returning empty
        terminal.get_text_range_format.return_value = ("", 0)
        terminal.get_cursor_position.return_value = (0, 5)
        # Mock get_text_format returning buffer with command output
        full_text = (
            "vagnarok@vagnarok-Product:~/OnyxSH$ cat /etc/shadow\n"
            "cat: /etc/shadow: Permissão negada\n"
            "vagnarok@vagnarok-Product:~/OnyxSH$ "
        )
        terminal.get_text_format.return_value = full_text

        cmd = SemanticCommand(
            command_id="cmd_fb_1",
            command_text="cat /etc/shadow",
            output_start_row=2,
            output_end_row=2,
        )

        output = tracker.extract_command_output(terminal, cmd)
        self.assertIn("Permissão negada", output)
        self.assertNotIn("cat /etc/shadow", output)


if __name__ == "__main__":
    unittest.main()
