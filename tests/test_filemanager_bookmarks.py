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


if __name__ == "__main__":
    unittest.main()
