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
            mock_prev_call.assert_called_once_with(item_prev, "/home/user")

        # Down / J to navigate next
        item_next = FileItem("next.txt", "-rw-r--r--", 20, datetime.now(), "u", "g")
        self.mock_nav_cb.return_value = (item_next, "/home/user")
        with patch.object(self.dialog, "preview_item") as mock_prev_call:
            res = self.dialog._on_key_pressed(None, Gdk.KEY_Down, 0, 0)
            self.assertEqual(res, Gdk.EVENT_STOP)
            self.mock_nav_cb.assert_called_with(1)
            mock_prev_call.assert_called_once_with(item_next, "/home/user")

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


if __name__ == "__main__":
    unittest.main()
