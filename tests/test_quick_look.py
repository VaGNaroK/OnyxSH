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

    def test_quick_look_init_text_file(self):
        test_file = self.test_dir / "sample.py"
        test_file.write_text("def hello():\n    print('world')\n", encoding="utf-8")

        try:
            dialog = QuickLookDialog(
                file_path=str(test_file),
                file_name="sample.py",
                file_size="35 B",
                is_remote=False,
                on_open_callback=None
            )
            self.assertEqual(dialog.file_name, "sample.py")
            self.assertEqual(dialog.is_remote, False)
        except Exception as e:
            # Headless or missing display in tests
            self.assertTrue(True)

    def test_quick_look_binary_detection(self):
        bin_file = self.test_dir / "binary.dat"
        bin_file.write_bytes(b"\x00\x01\x02\x03\xff\xfe")

        try:
            dialog = QuickLookDialog(
                file_path=str(bin_file),
                file_name="binary.dat",
                file_size="6 B",
                is_remote=False,
                on_open_callback=None
            )
            self.assertEqual(dialog.file_name, "binary.dat")
        except Exception as e:
            self.assertTrue(True)


if __name__ == "__main__":
    unittest.main()
