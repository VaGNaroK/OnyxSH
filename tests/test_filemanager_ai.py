# tests/test_filemanager_ai.py
import os
import tempfile
import unittest
from unittest.mock import MagicMock

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk

from onyxsh.filemanager.manager import FileManager
from onyxsh.filemanager.models import FileItem
from onyxsh.filemanager.quick_look import QuickLookDialog


class TestFileManagerAI(unittest.TestCase):

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.dir_path = self.temp_dir.name

        # Create mock filemanager
        self.fm = FileManager.__new__(FileManager)
        self.fm.current_path = self.dir_path
        self.fm.operations = None
        self.fm._show_toast = MagicMock()
        self.mock_window = MagicMock()
        self.mock_window.ui_builder = MagicMock()
        self.fm._parent_window_ref = lambda: self.mock_window
        self.fm._clipboard_items = []
        self.fm._clipboard_operation = None
        self.fm._is_remote_session = lambda: False

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_read_text_file_sample(self):
        file_path = os.path.join(self.dir_path, "script.py")
        with open(file_path, "w", encoding="utf-8") as f:
            f.write("def main():\n    print('Hello World')\n")

        item = FileItem(
            name="script.py",
            perms="-rwxr-xr-x",
            size=35,
            date="2026-08-22 00:00:00",
            owner="user",
            group="group",
        )

        content, is_binary, err = self.fm._read_file_sample_for_ai(item)
        self.assertIsNone(err)
        self.assertFalse(is_binary)
        self.assertIn("def main():", content)
        self.assertIn("Hello World", content)

    def test_read_binary_file_sample(self):
        file_path = os.path.join(self.dir_path, "binary.bin")
        with open(file_path, "wb") as f:
            f.write(b"\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00")

        item = FileItem(
            name="binary.bin",
            perms="-rwxr-xr-x",
            size=16,
            date="2026-08-22 00:00:00",
            owner="user",
            group="group",
        )

        content, is_binary, err = self.fm._read_file_sample_for_ai(item)
        self.assertIsNone(err)
        self.assertTrue(is_binary)
        self.assertIsNone(content)

    def test_read_log_file_tail(self):
        file_path = os.path.join(self.dir_path, "test.log")
        lines = [f"2026-08-22 00:00:{i:02d} [INFO] Line {i}" for i in range(200)]
        lines.append("2026-08-22 00:01:00 [ERROR] Critical exception occurred")
        with open(file_path, "w", encoding="utf-8") as f:
            f.write("\n".join(lines))

        item = FileItem(
            name="test.log",
            perms="-rw-r--r--",
            size=5000,
            date="2026-08-22 00:00:00",
            owner="user",
            group="group",
        )

        content, is_binary, err = self.fm._read_file_sample_for_ai(
            item, tail_mode=True, tail_lines=50
        )
        self.assertIsNone(err)
        self.assertFalse(is_binary)
        self.assertIn("[ERROR] Critical exception occurred", content)

    def test_read_directory_sample(self):
        subfile = os.path.join(self.dir_path, "test.txt")
        with open(subfile, "w") as f:
            f.write("hello")

        item = FileItem(
            name=os.path.basename(self.dir_path),
            perms="drwxr-xr-x",
            size=4096,
            date="2026-08-22 00:00:00",
            owner="user",
            group="group",
        )

        self.fm.current_path = os.path.dirname(self.dir_path)
        content, is_binary, err = self.fm._read_file_sample_for_ai(item)
        self.assertIsNone(err)
        self.assertFalse(is_binary)
        self.assertIn("test.txt", content)

    def test_send_to_ai_chat(self):
        prompt = "Explain this script"
        self.fm._send_to_ai_chat(prompt)
        self.mock_window.ui_builder.show_ai_panel.assert_called_once_with(
            initial_text=prompt, auto_send=True
        )

    def test_context_menu_model_includes_ai_actions(self):
        item = FileItem(
            name="app.py",
            perms="-rwxr-xr-x",
            size=1024,
            date="2026-08-22 00:00:00",
            owner="user",
            group="group",
        )
        menu_model = self.fm._create_context_menu_model([item])
        self.assertIsNotNone(menu_model)

        self.fm._init_context_action_group()
        self.assertTrue(self.fm.context_action_group.has_action("ai_explain"))
        self.assertTrue(self.fm.context_action_group.has_action("ai_diagnose"))
        self.assertTrue(self.fm.context_action_group.has_action("ai_audit_security"))

    def test_quick_look_ai_explain_callback(self):
        item = FileItem(
            name="test.py",
            perms="-rwxr-xr-x",
            size=100,
            date="2026-08-22 00:00:00",
            owner="user",
            group="group",
        )
        mock_callback = MagicMock()
        parent_window = Gtk.Window()

        ql = QuickLookDialog(
            parent_window=parent_window,
            on_ai_explain=mock_callback,
        )
        ql.current_item = item
        ql.current_folder = "/home/user"
        ql._on_ai_explain_clicked(None)

        mock_callback.assert_called_once_with(item, "/home/user")


if __name__ == "__main__":
    unittest.main()
