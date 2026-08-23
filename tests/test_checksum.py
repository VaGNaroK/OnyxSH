# tests/test_checksum.py
import hashlib
import os
import tempfile
import threading
import unittest
from unittest.mock import MagicMock

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk

from onyxsh.ui.dialogs.checksum_dialog import ChecksumDialog
from onyxsh.utils.checksum_utils import (
    calculate_file_hashes,
    compare_hash,
    detect_hash_type,
    format_checksum_report,
)


class TestChecksumUtils(unittest.TestCase):

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.test_file = os.path.join(self.temp_dir.name, "sample.txt")
        self.content = b"Hello, OnyxSH Checksum System!\n"
        with open(self.test_file, "wb") as f:
            f.write(self.content)

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_calculate_file_hashes_all_algorithms(self):
        hashes = calculate_file_hashes(self.test_file)
        self.assertIn("sha256", hashes)
        self.assertIn("sha512", hashes)
        self.assertIn("md5", hashes)
        self.assertIn("sha1", hashes)

        expected_sha256 = hashlib.sha256(self.content).hexdigest()
        expected_md5 = hashlib.md5(self.content).hexdigest()
        expected_sha1 = hashlib.sha1(self.content).hexdigest()
        expected_sha512 = hashlib.sha512(self.content).hexdigest()

        self.assertEqual(hashes["sha256"], expected_sha256)
        self.assertEqual(hashes["md5"], expected_md5)
        self.assertEqual(hashes["sha1"], expected_sha1)
        self.assertEqual(hashes["sha512"], expected_sha512)

    def test_calculate_empty_file_hashes(self):
        empty_file = os.path.join(self.temp_dir.name, "empty.txt")
        with open(empty_file, "wb") as f:
            pass

        progress_called = False

        def progress_cb(read_b, total_b, pct):
            nonlocal progress_called
            progress_called = True

        hashes = calculate_file_hashes(empty_file, progress_callback=progress_cb)
        self.assertEqual(
            hashes["sha256"],
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        )
        self.assertTrue(progress_called)

    def test_calculate_file_hashes_progress_callback(self):
        progress_records = []

        def progress_cb(read_b, total_b, pct):
            progress_records.append((read_b, total_b, pct))

        calculate_file_hashes(self.test_file, chunk_size=8, progress_callback=progress_cb)
        self.assertTrue(len(progress_records) > 0)
        final_read, final_total, final_pct = progress_records[-1]
        self.assertEqual(final_read, len(self.content))
        self.assertEqual(final_pct, 1.0)

    def test_calculate_file_hashes_cancel_event(self):
        cancel_evt = threading.Event()
        cancel_evt.set()

        with self.assertRaises(InterruptedError):
            calculate_file_hashes(self.test_file, cancel_event=cancel_evt)

    def test_calculate_file_not_found(self):
        with self.assertRaises(FileNotFoundError):
            calculate_file_hashes("/nonexistent/path/file.dat")

    def test_calculate_unsupported_algorithm(self):
        with self.assertRaises(ValueError):
            calculate_file_hashes(self.test_file, algorithms=("crc32",))

    def test_detect_hash_type(self):
        md5_sample = "5d41402abc4b2a76b9719d911017c592"
        sha1_sample = "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12"
        sha256_sample = (
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        )
        sha512_sample = "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"

        self.assertEqual(detect_hash_type(md5_sample), "md5")
        self.assertEqual(detect_hash_type(sha1_sample), "sha1")
        self.assertEqual(detect_hash_type(sha256_sample), "sha256")
        self.assertEqual(detect_hash_type(sha512_sample), "sha512")
        self.assertEqual(detect_hash_type(f'  "{sha256_sample}"  '), "sha256")

        # Invalid strings
        self.assertIsNone(detect_hash_type("not_a_hex_string_12345"))
        self.assertIsNone(detect_hash_type("1234567890abcdef"))  # 16 chars

    def test_compare_hash(self):
        computed = {
            "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "md5": "d41d8cd98f00b204e9800998ecf8427e",
        }

        # Match sha256 lowercase
        match, algo = compare_hash(
            computed, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        )
        self.assertTrue(match)
        self.assertEqual(algo, "sha256")

        # Match sha256 UPPERCASE with quotes and spaces
        match, algo = compare_hash(
            computed,
            '  "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855"  ',
        )
        self.assertTrue(match)
        self.assertEqual(algo, "sha256")

        # Match md5
        match, algo = compare_hash(computed, "d41d8cd98f00b204e9800998ecf8427e")
        self.assertTrue(match)
        self.assertEqual(algo, "md5")

        # Mismatch
        match, algo = compare_hash(
            computed, "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
        )
        self.assertFalse(match)
        self.assertIsNone(algo)

    def test_format_checksum_report(self):
        hashes = {
            "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "md5": "d41d8cd98f00b204e9800998ecf8427e",
        }
        report = format_checksum_report("test.iso", "/path/test.iso", "4.2 GB", hashes)
        self.assertIn("OnyxSH Checksum Report", report)
        self.assertIn("test.iso", report)
        self.assertIn("4.2 GB", report)
        self.assertIn("SHA256", report)
        self.assertIn(hashes["sha256"], report)
        self.assertIn(hashes["md5"], report)


class TestChecksumDialog(unittest.TestCase):

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.test_file = os.path.join(self.temp_dir.name, "checksum_test.bin")
        with open(self.test_file, "wb") as f:
            f.write(b"OnyxSH Checksum Dialog Test Content\n")

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_dialog_instantiation(self):
        parent_window = Gtk.Window()
        mock_terminal = MagicMock()

        dialog = ChecksumDialog(
            parent_window=parent_window,
            file_path=self.test_file,
            file_name="checksum_test.bin",
            file_size_str="36 B",
            bound_terminal=mock_terminal,
        )

        self.assertIsNotNone(dialog)
        self.assertEqual(dialog.file_name, "checksum_test.bin")
        self.assertEqual(dialog.file_size_str, "36 B")
        self.assertIn("sha256", dialog._hash_labels)
        self.assertIn("md5", dialog._hash_labels)

        # Test insert terminal command
        dialog._on_insert_terminal_clicked(None)
        mock_terminal.feed_child.assert_called_once()
        self.assertIn(b"sha256sum", mock_terminal.feed_child.call_args[0][0])

        dialog.close()


if __name__ == "__main__":
    unittest.main()
