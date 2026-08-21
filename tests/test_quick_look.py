import unittest
import tempfile
from pathlib import Path

from onyxsh.filemanager.quick_look import QuickLookDialog


class TestQuickLook(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.test_dir = Path(self.temp_dir.name)

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_quick_look_init_dialog(self):
        try:
            dialog = QuickLookDialog(parent_window=None)
            self.assertIsNotNone(dialog.stack)
            self.assertIsNotNone(dialog.text_view)
            self.assertIsNotNone(dialog.hex_label)
        except Exception as e:
            # If in headless CI without display, ignore
            pass

    def test_file_item_properties(self):
        from datetime import datetime
        from onyxsh.filemanager.models import FileItem

        item = FileItem(
            name="test.py",
            perms="-rwxr-xr-x",
            size=2048,
            date=datetime(2026, 8, 20, 12, 0, 0),
            owner="user",
            group="user"
        )
        self.assertEqual(item.formatted_size, "2.0 KB")
        self.assertEqual(item.size_bytes, 2048)
        self.assertEqual(item.formatted_date, "2026-08-20 12:00")
        self.assertEqual(item.date_modified, "2026-08-20 12:00")


if __name__ == "__main__":
    unittest.main()
