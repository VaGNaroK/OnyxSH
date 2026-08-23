# tests/test_filemanager_bookmarks.py
import unittest
import os
from unittest.mock import MagicMock

from onyxsh.settings.manager import SettingsManager
from onyxsh.filemanager.manager import FileManager


class TestFileManagerBookmarks(unittest.TestCase):

    def setUp(self):
        self.settings = SettingsManager()
        self.settings.set("file_manager_bookmarks", [])

    def test_add_and_remove_bookmark(self):
        path = "/tmp/test_folder"
        self.assertFalse(self.settings.is_bookmarked(path))

        res = self.settings.add_bookmark(path, name="Test Folder")
        self.assertTrue(res)
        self.assertTrue(self.settings.is_bookmarked(path))

        bookmarks = self.settings.get_bookmarks()
        self.assertEqual(len(bookmarks), 1)
        self.assertEqual(bookmarks[0]["path"], os.path.normpath(path))
        self.assertEqual(bookmarks[0]["name"], "Test Folder")

        # Duplicate add should return False
        self.assertFalse(self.settings.add_bookmark(path))

        # Remove bookmark
        rem_res = self.settings.remove_bookmark(path)
        self.assertTrue(rem_res)
        self.assertFalse(self.settings.is_bookmarked(path))

    def test_get_active_git_root(self):
        fm = FileManager.__new__(FileManager)
        fm.current_path = os.getcwd()
        git_root = fm._get_active_git_root()
        self.assertIsNotNone(git_root)
        self.assertTrue(os.path.exists(git_root))

    def test_escape_key_clears_search_and_closes_panel(self):
        import gi
        gi.require_version("Gtk", "4.0")
        gi.require_version("Gdk", "4.0")
        from gi.repository import Gdk, Gtk

        fm = FileManager.__new__(FileManager)
        fm.search_entry = Gtk.SearchEntry()
        fm.search_entry.set_text("query")
        fm.selection_model = None

        mock_parent = MagicMock()
        mock_parent.file_manager_button.get_active.return_value = True
        fm._parent_window_ref = lambda: mock_parent
        fm._terminal_manager_ref = lambda: None

        # 1. When text is present, Escape clears the text
        res = fm._on_search_key_pressed(None, Gdk.KEY_Escape, 0, 0)
        self.assertEqual(res, Gdk.EVENT_STOP)
        self.assertEqual(fm.search_entry.get_text(), "")
        mock_parent.file_manager_button.set_active.assert_not_called()

        # 2. When text is empty, Escape closes the file manager
        res = fm._on_search_key_pressed(None, Gdk.KEY_Escape, 0, 0)
        self.assertEqual(res, Gdk.EVENT_STOP)
        mock_parent.file_manager_button.set_active.assert_called_once_with(False)


if __name__ == "__main__":
    unittest.main()
