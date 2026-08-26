# tests/test_filemanager_transfers.py
"""Unit tests for TransferManager, TransferItem, TransferRow and transfer dialogs."""

import json
import os
import tempfile
import time
import unittest
from unittest.mock import MagicMock, patch

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gtk

from onyxsh.filemanager.transfer_dialog import TransferRow
from onyxsh.filemanager.transfer_manager import (
    TransferItem,
    TransferManager,
    TransferStatus,
    TransferType,
)


class TestFileManagerTransfers(unittest.TestCase):
    """Tests covering TransferManager lifecycle, warmup calculations, and persistence."""

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.config_dir = self.temp_dir.name
        self.tm = TransferManager(config_dir=self.config_dir)

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_transfer_item_properties_and_duration(self):
        item = TransferItem(
            id="t1",
            filename="data.tar.gz",
            local_path="/tmp/data.tar.gz",
            remote_path="/home/user/data.tar.gz",
            file_size=1048576,
            transfer_type=TransferType.DOWNLOAD,
            status=TransferStatus.PENDING,
            start_time=100.0,
            end_time=115.5,
        )
        self.assertEqual(item.get_duration(), 15.5)

        # No end time
        item_no_end = TransferItem(
            id="t2",
            filename="file.txt",
            local_path="/tmp/file.txt",
            remote_path="/home/user/file.txt",
            file_size=500,
            transfer_type=TransferType.UPLOAD,
            status=TransferStatus.IN_PROGRESS,
            start_time=100.0,
        )
        self.assertIsNone(item_no_end.get_duration())

    def test_transfer_item_warmup_and_stable_progress(self):
        now = time.time()
        item = TransferItem(
            id="t3",
            filename="big.iso",
            local_path="/tmp/big.iso",
            remote_path="/var/iso/big.iso",
            file_size=1000000,
            transfer_type=TransferType.DOWNLOAD,
            status=TransferStatus.IN_PROGRESS,
            start_time=now,
            warmup_end_time=now + 10.0,  # Warmup period not ended
            first_stable_progress=10.0,
            progress=25.0,
        )
        self.assertFalse(item.is_warmed_up())
        self.assertEqual(item.get_stable_progress(), 0.0)

        # Warmup period ended
        item.warmup_end_time = now - 5.0
        self.assertTrue(item.is_warmed_up())
        # Stable progress should be progress - first_stable_progress
        self.assertEqual(item.get_stable_progress(), 15.0)

    def test_add_and_start_transfer(self):
        started_signals = []
        self.tm.connect("transfer-started", lambda mgr, tid: started_signals.append(tid))

        t_id = self.tm.add_transfer(
            filename="document.pdf",
            local_path="/tmp/document.pdf",
            remote_path="/home/user/document.pdf",
            file_size=2048,
            transfer_type=TransferType.UPLOAD,
            is_cancellable=True,
        )
        self.assertIsNotNone(t_id)
        item = self.tm.get_transfer(t_id)
        self.assertIsNotNone(item)
        self.assertEqual(item.status, TransferStatus.PENDING)
        self.assertTrue(item.is_cancellable)

        self.tm.start_transfer(t_id)
        self.assertEqual(item.status, TransferStatus.IN_PROGRESS)
        self.assertIsNotNone(item.start_time)
        self.assertIn(t_id, started_signals)

    def test_update_progress(self):
        progress_signals = []
        self.tm.connect("transfer-progress", lambda mgr, tid, prog: progress_signals.append((tid, prog)))

        t_id = self.tm.add_transfer(
            filename="data.zip",
            local_path="/tmp/data.zip",
            remote_path="/home/user/data.zip",
            file_size=10000,
            transfer_type=TransferType.DOWNLOAD,
        )
        self.tm.start_transfer(t_id)

        # Update progress
        self.tm.update_progress(t_id, 45.0)
        item = self.tm.get_transfer(t_id)
        self.assertEqual(item.progress, 45.0)
        self.assertIsNotNone(item.warmup_end_time)

    def test_complete_transfer(self):
        completed_signals = []
        self.tm.connect("transfer-completed", lambda mgr, tid: completed_signals.append(tid))

        t_id = self.tm.add_transfer(
            filename="finished.txt",
            local_path="/tmp/finished.txt",
            remote_path="/home/user/finished.txt",
            file_size=500,
            transfer_type=TransferType.DOWNLOAD,
        )
        self.tm.start_transfer(t_id)
        self.tm.complete_transfer(t_id)

        # Removed from active transfers
        self.assertIsNone(self.tm.get_transfer(t_id))
        # Added to history
        self.assertEqual(len(self.tm.history), 1)
        self.assertEqual(self.tm.history[0].id, t_id)
        self.assertEqual(self.tm.history[0].status, TransferStatus.COMPLETED)
        self.assertEqual(self.tm.history[0].progress, 100.0)
        self.assertIn(t_id, completed_signals)

        # Verify history file was saved
        self.assertTrue(os.path.exists(self.tm.history_file))

    def test_fail_transfer_and_cancel_transfer(self):
        failed_signals = []
        cancelled_signals = []
        self.tm.connect("transfer-failed", lambda mgr, tid, err: failed_signals.append((tid, err)))
        self.tm.connect("transfer-cancelled", lambda mgr, tid: cancelled_signals.append(tid))

        # 1. Normal failure
        t_id_fail = self.tm.add_transfer(
            filename="broken.bin",
            local_path="/tmp/broken.bin",
            remote_path="/home/user/broken.bin",
            file_size=100,
            transfer_type=TransferType.UPLOAD,
        )
        self.tm.start_transfer(t_id_fail)
        self.tm.fail_transfer(t_id_fail, "Permission denied")

        self.assertEqual(self.tm.history[0].status, TransferStatus.FAILED)
        self.assertEqual(self.tm.history[0].error_message, "Permission denied")
        self.assertEqual(len(failed_signals), 1)

        # 2. User cancellation error
        t_id_cancel = self.tm.add_transfer(
            filename="cancelled.bin",
            local_path="/tmp/cancelled.bin",
            remote_path="/home/user/cancelled.bin",
            file_size=100,
            transfer_type=TransferType.DOWNLOAD,
            is_cancellable=True,
        )
        self.tm.start_transfer(t_id_cancel)
        self.tm.fail_transfer(t_id_cancel, "Cancelled by user")

        self.assertEqual(self.tm.history[0].status, TransferStatus.CANCELLED)
        self.assertEqual(len(cancelled_signals), 1)

    def test_cancel_transfer_event(self):
        t_id = self.tm.add_transfer(
            filename="job.tar",
            local_path="/tmp/job.tar",
            remote_path="/home/user/job.tar",
            file_size=1000,
            transfer_type=TransferType.UPLOAD,
            is_cancellable=True,
        )
        event = self.tm.get_cancellation_event(t_id)
        self.assertIsNotNone(event)
        self.assertFalse(event.is_set())

        self.tm.cancel_transfer(t_id)
        self.assertTrue(event.is_set())

    def test_save_and_load_history(self):
        # Populate history with items
        for i in range(5):
            self.tm.history.append(
                TransferItem(
                    id=f"hist-{i}",
                    filename=f"file{i}.txt",
                    local_path=f"/tmp/file{i}.txt",
                    remote_path=f"/remote/file{i}.txt",
                    file_size=1024 * (i + 1),
                    transfer_type=TransferType.DOWNLOAD if i % 2 == 0 else TransferType.UPLOAD,
                    status=TransferStatus.COMPLETED,
                    start_time=100.0,
                    end_time=105.0,
                    progress=100.0,
                )
            )
        self.tm._save_history()

        # Create new manager pointing to the same config dir
        new_tm = TransferManager(config_dir=self.config_dir)
        self.assertEqual(len(new_tm.history), 5)
        self.assertEqual(new_tm.history[0].id, "hist-0")
        self.assertEqual(new_tm.history[0].transfer_type, TransferType.DOWNLOAD)
        self.assertEqual(new_tm.history[0].status, TransferStatus.COMPLETED)

    def test_history_trimming_to_50_items(self):
        for i in range(60):
            self.tm.history.append(
                TransferItem(
                    id=f"id-{i}",
                    filename=f"f{i}.dat",
                    local_path="/tmp/f.dat",
                    remote_path="/remote/f.dat",
                    file_size=10,
                    transfer_type=TransferType.UPLOAD,
                    status=TransferStatus.COMPLETED,
                )
            )
        self.tm._save_history()

        new_tm = TransferManager(config_dir=self.config_dir)
        self.assertEqual(len(new_tm.history), 50)

    def test_format_helpers(self):
        # File Size
        self.assertEqual(self.tm._format_file_size(500), "500 B")
        self.assertEqual(self.tm._format_file_size(1536), "1.5 KB")
        self.assertEqual(self.tm._format_file_size(1048576 * 3), "3.0 MB")
        self.assertEqual(self.tm._format_file_size(1048576 * 1024 * 2), "2.0 GB")

        # Speed
        self.assertEqual(self.tm._format_speed(0), "0 B/s")
        self.assertEqual(self.tm._format_speed(500), "500.0 B/s")
        self.assertEqual(self.tm._format_speed(1536), "1.5 KB/s")
        self.assertEqual(self.tm._format_speed(1048576 * 5), "5.0 MB/s")

        # Duration
        self.assertEqual(self.tm._format_duration(45), "45s")
        self.assertEqual(self.tm._format_duration(125), "2m 5s")
        self.assertEqual(self.tm._format_duration(3665), "1h 1m")

    def test_transfer_row_ui_states(self):
        item_pending = TransferItem(
            id="row-1",
            filename="image.png",
            local_path="/tmp/image.png",
            remote_path="/home/user/image.png",
            file_size=2048,
            transfer_type=TransferType.DOWNLOAD,
            status=TransferStatus.PENDING,
        )
        mock_remove = MagicMock()
        row = TransferRow(item_pending, self.tm, mock_remove)
        self.assertEqual(row.filename_label.get_label(), "image.png")
        self.assertTrue(row.cancel_button.get_visible())
        self.assertFalse(row.remove_button.get_visible())

        # Transition to COMPLETED
        item_pending.status = TransferStatus.COMPLETED
        item_pending.start_time = time.time() - 10
        item_pending.end_time = time.time()
        row.update_state()
        self.assertFalse(row.cancel_button.get_visible())
        self.assertTrue(row.remove_button.get_visible())

        # Trigger remove button
        row.remove_button.emit("clicked")
        mock_remove.assert_called_once_with("row-1")


if __name__ == "__main__":
    unittest.main()
