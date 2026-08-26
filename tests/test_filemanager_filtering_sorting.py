# tests/test_filemanager_filtering_sorting.py
"""Unit tests for FileManager sorting, filtering, command builders, and clipboard."""

from datetime import datetime
import unittest
from unittest.mock import MagicMock

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk

from onyxsh.filemanager.manager import FileManager
from onyxsh.filemanager.models import FileItem
from onyxsh.sessions.models import SessionItem


class TestFileManagerFilteringAndSorting(unittest.TestCase):
    """Tests covering FileManager sorting, filtering, search query handling, and clipboard logic."""

    def setUp(self):
        self.fm = FileManager.__new__(FileManager)
        self.fm.hidden_files_toggle = MagicMock()
        self.fm.hidden_files_toggle.get_active.return_value = False
        self.fm.search_entry = Gtk.SearchEntry()
        self.fm.recursive_search_enabled = False
        self.fm._showing_recursive_results = False
        self.fm.current_path = "/home/user"
        self.fm._clipboard_items = []
        self.fm._clipboard_operation = None
        self.fm._clipboard_session_key = None
        self.fm.session_item = SessionItem(
            name="Local", session_type="local", host="localhost", port=22
        )

    def test_filter_files_hidden_toggle(self):
        normal_item = FileItem("notes.txt", "-rw-r--r--", 100, datetime.now(), "u", "g")
        dot_item = FileItem(".secret", "-rw-r--r--", 100, datetime.now(), "u", "g")
        parent_item = FileItem("..", "drwxr-xr-x", 4096, datetime.now(), "u", "g")

        # 1. Hidden files disabled (default)
        self.fm.hidden_files_toggle.get_active.return_value = False
        self.assertTrue(self.fm._filter_files(normal_item))
        self.assertFalse(self.fm._filter_files(dot_item))
        self.assertTrue(self.fm._filter_files(parent_item))

        # 2. Hidden files enabled
        self.fm.hidden_files_toggle.get_active.return_value = True
        self.assertTrue(self.fm._filter_files(normal_item))
        self.assertTrue(self.fm._filter_files(dot_item))
        self.assertTrue(self.fm._filter_files(parent_item))

    def test_filter_files_with_search_query(self):
        item1 = FileItem("report_2026.pdf", "-rw-r--r--", 100, datetime.now(), "u", "g")
        item2 = FileItem("invoice_august.pdf", "-rw-r--r--", 100, datetime.now(), "u", "g")
        dot_item = FileItem(".report_backup", "-rw-r--r--", 100, datetime.now(), "u", "g")
        parent_item = FileItem("..", "drwxr-xr-x", 4096, datetime.now(), "u", "g")

        self.fm.search_entry.set_text("REPORT")
        self.fm.hidden_files_toggle.get_active.return_value = False

        # '..' should always be filtered out when searching
        self.assertFalse(self.fm._filter_files(parent_item))
        # Case-insensitive match
        self.assertTrue(self.fm._filter_files(item1))
        # Non-matching item
        self.assertFalse(self.fm._filter_files(item2))
        # Matching dotfile should still be hidden if hidden toggle is False
        self.assertFalse(self.fm._filter_files(dot_item))

        # When hidden toggle is True
        self.fm.hidden_files_toggle.get_active.return_value = True
        self.assertTrue(self.fm._filter_files(dot_item))

    def test_dolphin_sort_priority_parent_directory_always_first(self):
        parent = FileItem("..", "drwxr-xr-x", 4096, datetime.now(), "u", "g")
        folder = FileItem("aaa_folder", "drwxr-xr-x", 4096, datetime.now(), "u", "g")
        file1 = FileItem("aaa_file.txt", "-rw-r--r--", 100, datetime.now(), "u", "g")

        # '..' vs folder -> '..' comes first (-1)
        self.assertEqual(self.fm._dolphin_sort_priority(parent, folder), -1)
        self.assertEqual(self.fm._dolphin_sort_priority(folder, parent), 1)

        # '..' vs file -> '..' comes first (-1)
        self.assertEqual(self.fm._dolphin_sort_priority(parent, file1), -1)
        self.assertEqual(self.fm._dolphin_sort_priority(file1, parent), 1)

    def test_dolphin_sort_priority_directories_before_files(self):
        folder = FileItem("zzz_folder", "drwxr-xr-x", 4096, datetime.now(), "u", "g")
        file1 = FileItem("aaa_file.txt", "-rw-r--r--", 100, datetime.now(), "u", "g")

        # folder comes before file (a_type 0 < b_type 1 -> return -1)
        self.assertEqual(self.fm._dolphin_sort_priority(folder, file1), -1)
        self.assertEqual(self.fm._dolphin_sort_priority(file1, folder), 1)

    def test_sort_by_name(self):
        f1 = FileItem("alpha.txt", "-rw-r--r--", 100, datetime.now(), "u", "g")
        f2 = FileItem("beta.txt", "-rw-r--r--", 100, datetime.now(), "u", "g")
        self.assertEqual(self.fm._sort_by_name(f1, f2), -1)
        self.assertEqual(self.fm._sort_by_name(f2, f1), 1)
        self.assertEqual(self.fm._sort_by_name(f1, f1), 0)

    def test_sort_by_size(self):
        f_small = FileItem("small.dat", "-rw-r--r--", 10, datetime.now(), "u", "g")
        f_big = FileItem("big.dat", "-rw-r--r--", 1000, datetime.now(), "u", "g")
        self.assertEqual(self.fm._sort_by_size(f_small, f_big), -1)
        self.assertEqual(self.fm._sort_by_size(f_big, f_small), 1)

    def test_sort_by_date(self):
        dt1 = datetime(2026, 8, 1, 10, 0)
        dt2 = datetime(2026, 8, 25, 10, 0)
        f_old = FileItem("old.txt", "-rw-r--r--", 100, dt1, "u", "g")
        f_new = FileItem("new.txt", "-rw-r--r--", 100, dt2, "u", "g")
        self.assertEqual(self.fm._sort_by_date(f_old, f_new), -1)
        self.assertEqual(self.fm._sort_by_date(f_new, f_old), 1)

    def test_sort_by_permissions_owner_group(self):
        f1 = FileItem("f1", "-rwxr-xr-x", 100, datetime.now(), "alice", "admin")
        f2 = FileItem("f2", "-rw-r--r--", 100, datetime.now(), "bob", "users")

        self.assertNotEqual(self.fm._sort_by_permissions(f1, f2), 0)
        self.assertEqual(self.fm._sort_by_owner(f1, f2), -1)
        self.assertEqual(self.fm._sort_by_group(f1, f2), -1)

    def test_build_fd_command(self):
        self.fm._fd_command_name = "fdfind"
        cmd = self.fm._build_fd_command("/home/user/project", "test file.py", show_hidden=False)
        self.assertEqual(cmd[0], "sh")
        self.assertEqual(cmd[1], "-c")
        self.assertIn("fdfind -i", cmd[2])
        self.assertIn("'test file.py'", cmd[2])
        self.assertIn("xargs -0 ls -ld --time-style=long-iso --classify", cmd[2])

        cmd_hidden = self.fm._build_fd_command("/home/user", "config", show_hidden=True)
        self.assertIn("-H", cmd_hidden[2])

    def test_build_find_command(self):
        # Hidden = True
        cmd_hidden = self.fm._build_find_command("/tmp/dir", "app", show_hidden=True)
        self.assertEqual(cmd_hidden[0], "find")
        self.assertEqual(cmd_hidden[1], "/tmp/dir")
        self.assertIn("-iname", cmd_hidden)
        self.assertIn("*app*", cmd_hidden)

        # Hidden = False
        cmd_no_hidden = self.fm._build_find_command("/tmp/dir", "app", show_hidden=False)
        self.assertIn("-not", cmd_no_hidden)
        self.assertIn("*/.*", cmd_no_hidden)

    def test_clipboard_and_can_paste(self):
        # Empty clipboard
        self.assertFalse(self.fm._can_paste())

        # Set clipboard with item and local session
        item = FileItem("test.txt", "-rw-r--r--", 100, datetime.now(), "u", "g")
        self.fm._clipboard_items = [item]
        self.fm._clipboard_operation = "copy"
        self.fm._clipboard_session_key = "local"
        self.assertTrue(self.fm._can_paste())

        # Different session key
        self.fm._clipboard_session_key = "remoteuser@host:22"
        self.assertFalse(self.fm._can_paste())

        # Clear clipboard
        self.fm._clear_clipboard()
        self.assertEqual(self.fm._clipboard_items, [])
        self.assertIsNone(self.fm._clipboard_operation)
        self.assertIsNone(self.fm._clipboard_session_key)
        self.assertFalse(self.fm._can_paste())


if __name__ == "__main__":
    unittest.main()
