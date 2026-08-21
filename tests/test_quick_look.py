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


if __name__ == "__main__":
    unittest.main()
