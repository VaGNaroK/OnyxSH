# tests/test_quick_look.py
"""Comprehensive unit tests for QuickLookDialog in onyxsh.filemanager.quick_look."""

import tempfile
import unittest
from datetime import datetime
from pathlib import Path
from unittest.mock import MagicMock, patch

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gdk, Gtk

from onyxsh.filemanager.models import FileItem
from onyxsh.filemanager.quick_look import QuickLookDialog


class TestQuickLook(unittest.TestCase):
    """Tests covering QuickLook previewing, syntax rendering, hex formatting, and navigation."""

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.test_dir = Path(self.temp_dir.name)
        self.parent_window = Gtk.Window()
        self.mock_editor_cb = MagicMock()
        self.mock_nav_cb = MagicMock()
        self.mock_ai_cb = MagicMock()
        self.mock_checksum_cb = MagicMock()

        self.dialog = QuickLookDialog(
            parent_window=self.parent_window,
            on_open_editor=self.mock_editor_cb,
            on_navigate=self.mock_nav_cb,
            on_ai_explain=self.mock_ai_cb,
            on_calculate_checksum=self.mock_checksum_cb,
        )

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_quick_look_init_dialog(self):
        self.assertIsNotNone(self.dialog.stack)
        self.assertIsNotNone(self.dialog.text_view)
        self.assertIsNotNone(self.dialog.hex_label)
        self.assertIsNotNone(self.dialog.title_label)
        self.assertIsNotNone(self.dialog.subtitle_label)

    def test_syntax_tags_initialization(self):
        buffer = Gtk.TextBuffer()
        self.dialog._init_syntax_tags(buffer)
        tag_table = buffer.get_tag_table()

        for tag_name in ["kw", "str", "num", "comment", "fn", "op", "err", "warn", "info", "date"]:
            tag = tag_table.lookup(tag_name)
            self.assertIsNotNone(tag, f"Tag '{tag_name}' was not created in tag table")

    def test_apply_syntax_highlighting_python(self):
        code = "def hello():\n    print('world')\n"
        self.dialog.text_buffer.set_text("")
        self.dialog._apply_syntax_highlighting("test.py", code)
        text = self.dialog.text_buffer.get_text(
            self.dialog.text_buffer.get_start_iter(),
            self.dialog.text_buffer.get_end_iter(),
            True,
        )
        self.assertIn("def hello():", text)

    def test_apply_syntax_highlighting_log_levels(self):
        log_content = "2026-08-25 12:00:00 [ERROR] Connection lost\n2026-08-25 12:00:01 [WARN] Retrying\n"
        self.dialog.text_buffer.set_text("")
        self.dialog._apply_syntax_highlighting("app.log", log_content)
        text = self.dialog.text_buffer.get_text(
            self.dialog.text_buffer.get_start_iter(),
            self.dialog.text_buffer.get_end_iter(),
            True,
        )
        self.assertIn("[ERROR]", text)
        self.assertIn("[WARN]", text)

    def test_render_text_preview(self):
        item = FileItem("script.sh", "-rwxr-xr-x", 120, datetime.now(), "u", "g")
        raw_bytes = b"#!/bin/bash\necho 'running tests'\n"
        self.dialog._render_text_preview(item, raw_bytes, is_truncated=False)

        self.assertFalse(self.dialog.truncated_banner.get_revealed())
        self.assertEqual(self.dialog.stack.get_visible_child_name(), "text")
        info_text = self.dialog.text_info_label.get_text()
        self.assertTrue("lines" in info_text or "linhas" in info_text)
        self.assertIn("120 B", info_text)

    def test_render_binary_preview_hex_header(self):
        item = FileItem("program.bin", "-rwxr-xr-x", 512, datetime.now(), "u", "g")
        raw_bytes = b"\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x03\x00>\x00"
        self.dialog._render_binary_preview(item, raw_bytes)

        self.assertEqual(self.dialog.stack.get_visible_child_name(), "binary")
        self.assertIn("Hex Header", self.dialog.hex_label.get_text())
        self.assertIn("7f 45 4c 46", self.dialog.hex_label.get_text().lower())

    def test_keyboard_navigation_shortcuts(self):
        # Escape / Q to close
        with patch.object(self.dialog, "close") as mock_close:
            res = self.dialog._on_key_pressed(None, Gdk.KEY_Escape, 0, 0)
            self.assertEqual(res, Gdk.EVENT_STOP)
            mock_close.assert_called_once()

        # Up / K to navigate previous
        item_prev = FileItem("prev.txt", "-rw-r--r--", 10, datetime.now(), "u", "g")
        self.mock_nav_cb.return_value = (item_prev, "/home/user")
        with patch.object(self.dialog, "preview_item") as mock_prev_call:
            res = self.dialog._on_key_pressed(None, Gdk.KEY_Up, 0, 0)
            self.assertEqual(res, Gdk.EVENT_STOP)
            self.mock_nav_cb.assert_called_with(-1)
            mock_prev_call.assert_called_once_with(item_prev, "/home/user", None)

        # Down / J to navigate next
        item_next = FileItem("next.txt", "-rw-r--r--", 20, datetime.now(), "u", "g")
        self.mock_nav_cb.return_value = (item_next, "/home/user")
        with patch.object(self.dialog, "preview_item") as mock_prev_call:
            res = self.dialog._on_key_pressed(None, Gdk.KEY_Down, 0, 0)
            self.assertEqual(res, Gdk.EVENT_STOP)
            self.mock_nav_cb.assert_called_with(1)
            mock_prev_call.assert_called_once_with(item_next, "/home/user", None)

    def test_action_callbacks(self):
        item = FileItem("main.py", "-rw-r--r--", 100, datetime.now(), "u", "g")
        self.dialog.current_item = item
        self.dialog.current_folder = "/home/user"

        # AI Explain
        self.dialog._on_ai_explain_clicked(None)
        self.mock_ai_cb.assert_called_once_with(item, "/home/user")

        # Open in Editor
        self.dialog._on_open_editor_clicked(None)
        self.mock_editor_cb.assert_called_once_with(item, "/home/user")

        # Checksum
        self.dialog._on_checksum_clicked(None)
        self.mock_checksum_cb.assert_called_once_with(item, "/home/user")

    def test_edit_mode_toggle(self):
        self.assertFalse(self.dialog.is_editing)
        self.assertFalse(self.dialog.text_view.get_editable())

        # Toggle on
        self.dialog._toggle_edit_mode(True)
        self.assertTrue(self.dialog.is_editing)
        self.assertTrue(self.dialog.text_view.get_editable())
        self.assertTrue(self.dialog.save_btn.get_visible())
        self.assertTrue(self.dialog.save_sudo_btn.get_visible())
        self.assertIn("editing", self.dialog.text_view.get_css_classes())

        # Toggle off without dirty
        self.dialog._toggle_edit_mode(False)
        self.assertFalse(self.dialog.is_editing)
        self.assertFalse(self.dialog.text_view.get_editable())
        self.assertFalse(self.dialog.save_btn.get_visible())
        self.assertFalse(self.dialog.save_sudo_btn.get_visible())

    def test_dirty_state_tracking(self):
        item = FileItem("config.yaml", "-rw-r--r--", 50, datetime.now(), "u", "g")
        self.dialog.current_item = item
        self.dialog.current_folder = str(self.test_dir)
        self.dialog._render_text_preview(item, b"port: 8080\n", is_truncated=False, full_path="/tmp/config.yaml")

        self.assertFalse(self.dialog.is_dirty)
        self.assertFalse(self.dialog.save_btn.get_sensitive())

        # Insert modification
        self.dialog.text_buffer.set_text("port: 9090\n")
        self.assertTrue(self.dialog.is_dirty)
        self.assertTrue(self.dialog.save_btn.get_sensitive())
        self.assertIn("●", self.dialog.title_label.get_text())

    def test_editor_keyboard_shortcuts(self):
        # Ctrl+E toggles edit mode
        with patch.object(self.dialog.edit_toggle_btn, "set_active") as mock_toggle:
            self.dialog._is_binary = False
            self.dialog._is_image = False
            res = self.dialog._on_key_pressed(None, Gdk.KEY_e, 0, Gdk.ModifierType.CONTROL_MASK)
            self.assertEqual(res, Gdk.EVENT_STOP)
            mock_toggle.assert_called_once()

        # Ctrl+S saves
        with patch.object(self.dialog, "_on_save_clicked") as mock_save:
            res = self.dialog._on_key_pressed(None, Gdk.KEY_s, 0, Gdk.ModifierType.CONTROL_MASK)
            self.assertEqual(res, Gdk.EVENT_STOP)
            mock_save.assert_called_once_with(as_sudo=False)

        # Ctrl+Shift+S saves as root
        with patch.object(self.dialog, "_on_save_clicked") as mock_save_sudo:
            state = Gdk.ModifierType.CONTROL_MASK | Gdk.ModifierType.SHIFT_MASK
            res = self.dialog._on_key_pressed(None, Gdk.KEY_s, 0, state)
            self.assertEqual(res, Gdk.EVENT_STOP)
            mock_save_sudo.assert_called_once_with(as_sudo=True)

    def test_save_file_content_operations_local(self):
        from onyxsh.filemanager.operations import FileOperations
        from onyxsh.sessions.models import SessionItem

        session = SessionItem(name="local", host="localhost", session_type="local")
        ops = FileOperations(session)

        test_file = self.test_dir / "test_write.txt"
        test_file.write_text("initial content", encoding="utf-8")

        success, msg = ops.save_file_content(str(test_file), "updated content", as_sudo=False)
        self.assertTrue(success)
        self.assertEqual(msg, "OK")
        self.assertEqual(test_file.read_text(encoding="utf-8"), "updated content")


if __name__ == "__main__":
    unittest.main()

