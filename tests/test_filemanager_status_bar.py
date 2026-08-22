# tests/test_filemanager_status_bar.py
import unittest
from datetime import datetime

from onyxsh.filemanager.models import FileItem
from onyxsh.filemanager.manager import FileManager


class TestFileManagerStatusBarAndBadges(unittest.TestCase):

    def test_file_item_root_owned(self):
        root_item = FileItem(
            name="syslog",
            perms="-rw-r--r--",
            size=1024,
            date=datetime.now(),
            owner="root",
            group="root",
        )
        self.assertTrue(root_item.is_root_owned)

        user_item = FileItem(
            name="test.txt",
            perms="-rw-r--r--",
            size=1024,
            date=datetime.now(),
            owner="user",
            group="user",
        )
        self.assertFalse(user_item.is_root_owned)

    def test_file_item_type_badge(self):
        py_item = FileItem(
            name="script.py",
            perms="-rw-r--r--",
            size=1024,
            date=datetime.now(),
            owner="user",
            group="user",
        )
        badge = py_item.file_type_badge
        self.assertIsNotNone(badge)
        self.assertEqual(badge[0], "PY")
        self.assertEqual(badge[1], "badge-py")

        sh_item = FileItem(
            name="deploy.sh",
            perms="-rwxr-xr-x",
            size=512,
            date=datetime.now(),
            owner="user",
            group="user",
        )
        badge_sh = sh_item.file_type_badge
        self.assertIsNotNone(badge_sh)
        self.assertEqual(badge_sh[0], "SH")
        self.assertEqual(badge_sh[1], "badge-sh")

        log_item = FileItem(
            name="app.log",
            perms="-rw-r--r--",
            size=2048,
            date=datetime.now(),
            owner="user",
            group="user",
        )
        badge_log = log_item.file_type_badge
        self.assertIsNotNone(badge_log)
        self.assertEqual(badge_log[0], "LOG")

    def test_format_bytes(self):
        fm = FileManager.__new__(FileManager)
        self.assertEqual(fm._format_bytes(500), "500 B")
        self.assertEqual(fm._format_bytes(1536), "1.5 KB")
        self.assertEqual(fm._format_bytes(int(1048576 * 2.5)), "2.5 MB")


if __name__ == "__main__":
    unittest.main()
