"""Unit tests for File Manager Dual-Pane mode (TODO 5.7)."""

import os
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, GLib, GObject, Gtk

from onyxsh.filemanager.dual_pane import DualPaneTransferBar, RemoteDiffHelper
from onyxsh.filemanager.local_pane import LocalFileBrowserPane
from onyxsh.filemanager.models import FileItem
from onyxsh.settings.config import DefaultSettings


class TestLocalFileBrowserPane(unittest.TestCase):
    """Tests for LocalFileBrowserPane directory navigation and models."""

    @classmethod
    def setUpClass(cls):
        cls.test_dir = tempfile.mkdtemp(prefix="onyxsh_test_local_pane_")
        # Create test hierarchy
        cls.sub_dir = os.path.join(cls.test_dir, "subdir")
        os.makedirs(cls.sub_dir, exist_ok=True)

        cls.file_a = os.path.join(cls.test_dir, "alpha.txt")
        with open(cls.file_a, "w") as f:
            f.write("Alpha content line 1\nAlpha content line 2\n")

        cls.file_b = os.path.join(cls.test_dir, "beta.log")
        with open(cls.file_b, "w") as f:
            f.write("Beta log output\n")

        cls.file_hidden = os.path.join(cls.test_dir, ".hidden_config")
        with open(cls.file_hidden, "w") as f:
            f.write("secret=true\n")

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.test_dir, ignore_errors=True)

    def test_initialization_and_path(self):
        """Validates pane initialization with explicit and fallback paths."""
        pane = LocalFileBrowserPane(initial_path=self.test_dir)
        self.assertEqual(pane.current_path, self.test_dir)
        self.assertIsNotNone(pane.get_widget())
        self.assertIsInstance(pane.get_widget(), Gtk.Box)

        # Fallback on nonexistent path
        pane_invalid = LocalFileBrowserPane(initial_path="/nonexistent/directory/path/123")
        self.assertEqual(pane_invalid.current_path, str(Path.home()))

    def test_size_formatting(self):
        """Validates human-readable file size formatting."""
        self.assertEqual(LocalFileBrowserPane._format_size(500), "500 B")
        self.assertEqual(LocalFileBrowserPane._format_size(2048), "2.0 KB")
        self.assertEqual(LocalFileBrowserPane._format_size(5 * 1024 * 1024), "5.0 MB")
        self.assertEqual(LocalFileBrowserPane._format_size(3 * 1024 * 1024 * 1024), "3.0 GB")

    def test_sorting_and_filtering(self):
        """Tests filter and sort functions of LocalFileBrowserPane."""
        pane = LocalFileBrowserPane(initial_path=self.test_dir, show_hidden=False)

        item_parent = FileItem("..", "drwxr-xr-x", 4096, None, "user", "group")
        item_dir = FileItem("subdir/", "drwxr-xr-x", 4096, None, "user", "group")
        item_file_a = FileItem("alpha.txt", "-rw-r--r--", 100, None, "user", "group")
        item_file_b = FileItem("beta.log", "-rw-r--r--", 200, None, "user", "group")
        item_hidden = FileItem(".hidden_config", "-rw-r--r--", 50, None, "user", "group")

        # 1. ".." is always first
        self.assertEqual(pane._sort_func(item_parent, item_dir), -1)
        self.assertEqual(pane._sort_func(item_dir, item_parent), 1)

        # 2. Directories before files
        self.assertEqual(pane._sort_func(item_dir, item_file_a), -1)
        self.assertEqual(pane._sort_func(item_file_a, item_dir), 1)

        # 3. Alphabetical order between files
        self.assertEqual(pane._sort_func(item_file_a, item_file_b), -1)
        self.assertEqual(pane._sort_func(item_file_b, item_file_a), 1)

        # 4. Hidden files filter
        self.assertTrue(pane._filter_func(item_parent))
        self.assertTrue(pane._filter_func(item_file_a))
        self.assertFalse(pane._filter_func(item_hidden))

        # Show hidden enabled
        pane.set_show_hidden(True)
        self.assertTrue(pane._filter_func(item_hidden))

        # Search filter
        pane.search_entry.set_text("alpha")
        self.assertTrue(pane._filter_func(item_file_a))
        self.assertFalse(pane._filter_func(item_file_b))

    def test_navigation_and_breadcrumbs(self):
        """Validates navigate_to, go_up, and go_home methods."""
        pane = LocalFileBrowserPane(initial_path=self.test_dir)

        path_changed_events = []
        pane.connect("path-changed", lambda _, p: path_changed_events.append(p))

        # Navigate into subdir
        pane.navigate_to(self.sub_dir)
        self.assertEqual(pane.current_path, self.sub_dir)
        self.assertIn(self.sub_dir, path_changed_events)

        # Navigate up
        pane.go_up()
        self.assertEqual(pane.current_path, self.test_dir)

        # Navigate home
        pane.go_home()
        self.assertEqual(pane.current_path, str(Path.home()))

    def test_get_selected_items_and_paths(self):
        """Verifies selected items and paths extraction."""
        pane = LocalFileBrowserPane(initial_path=self.test_dir)
        pane.store.remove_all()

        item1 = FileItem("file1.txt", "-rw-r--r--", 10, None, "user", "group")
        item2 = FileItem("file2.txt", "-rw-r--r--", 20, None, "user", "group")
        pane.store.append(item1)
        pane.store.append(item2)

        # Select first item
        pane.selection_model.select_item(0, True)
        selected = pane.get_selected_items()
        self.assertEqual(len(selected), 1)

        paths = pane.get_selected_paths()
        self.assertEqual(len(paths), 1)
        self.assertEqual(paths[0].name, "file1.txt")


class TestDualPaneTransferBar(unittest.TestCase):
    """Tests for the Central Transfer Bar buttons and actions."""

    def test_transfer_bar_actions(self):
        """Validates action button callbacks when clicked."""
        upload_called = []
        download_called = []
        diff_called = []

        bar = DualPaneTransferBar(
            on_upload=lambda: upload_called.append(True),
            on_download=lambda: download_called.append(True),
            on_diff=lambda: diff_called.append(True),
        )

        self.assertIsNotNone(bar.upload_button)
        self.assertIsNotNone(bar.download_button)
        self.assertIsNotNone(bar.diff_button)

        # Trigger button callbacks
        bar.upload_button.emit("clicked")
        self.assertEqual(len(upload_called), 1)

        bar.download_button.emit("clicked")
        self.assertEqual(len(download_called), 1)

        bar.diff_button.emit("clicked")
        self.assertEqual(len(diff_called), 1)


class TestRemoteDiffHelper(unittest.TestCase):
    """Tests for diff comparison and validation logic."""

    def setUp(self):
        self.tmp_dir = tempfile.mkdtemp(prefix="onyxsh_test_diff_")
        self.local_file = Path(self.tmp_dir) / "test.py"
        self.local_file.write_text("def hello():\n    return 'world'\n")

    def tearDown(self):
        shutil.rmtree(self.tmp_dir, ignore_errors=True)

    def test_identical_files_diff(self):
        """Validates that identical local and remote files trigger on_identical."""
        mock_operations = MagicMock()

        def mock_download(remote_path, local_dest, session_override=None):
            Path(local_dest).write_text("def hello():\n    return 'world'\n")
            return True

        mock_operations.download_file_sync.side_effect = mock_download
        mock_window = MagicMock()
        mock_session = MagicMock()

        identical_called = []

        with patch("onyxsh.core.tasks.AsyncTaskManager.get") as mock_atm:
            # Execute synchronously
            mock_atm.return_value.submit_io.side_effect = lambda f: f()
            with patch("gi.repository.GLib.idle_add", side_effect=lambda f, *args: f(*args)):
                RemoteDiffHelper.compare_files_async(
                    parent_window=mock_window,
                    local_path=self.local_file,
                    remote_path="/var/www/test.py",
                    operations=mock_operations,
                    session_item=mock_session,
                    on_identical=lambda: identical_called.append(True),
                )

        self.assertEqual(len(identical_called), 1)

    def test_different_files_diff(self):
        """Validates that differences open the DiffReviewDialog."""
        mock_operations = MagicMock()

        def mock_download(remote_path, local_dest, session_override=None):
            Path(local_dest).write_text("def hello():\n    return 'remote'\n")
            return True

        mock_operations.download_file_sync.side_effect = mock_download
        mock_window = MagicMock()
        mock_session = MagicMock()

        with patch("onyxsh.core.tasks.AsyncTaskManager.get") as mock_atm:
            mock_atm.return_value.submit_io.side_effect = lambda f: f()
            with patch("gi.repository.GLib.idle_add", side_effect=lambda f, *args: f(*args)):
                with patch("onyxsh.filemanager.dual_pane.DiffReviewDialog") as mock_dialog:
                    RemoteDiffHelper.compare_files_async(
                        parent_window=mock_window,
                        local_path=self.local_file,
                        remote_path="/var/www/test.py",
                        operations=mock_operations,
                        session_item=mock_session,
                    )
                    self.assertTrue(mock_dialog.called)
                    call_args = mock_dialog.call_args[1]
                    self.assertIn("diff_text", call_args)
                    self.assertIn("-    return 'world'", call_args["diff_text"])
                    self.assertIn("+    return 'remote'", call_args["diff_text"])


class TestDualPaneSettings(unittest.TestCase):
    """Tests that Dual-Pane configuration keys are registered."""

    def test_settings_keys_exist(self):
        defaults = DefaultSettings.get_defaults()
        self.assertIn("file_manager_dual_pane_enabled", defaults)
        self.assertFalse(defaults["file_manager_dual_pane_enabled"])
        self.assertIn("file_manager_local_path", defaults)
        self.assertEqual(defaults["file_manager_local_path"], "")


class TestFileManagerDualPaneIntegration(unittest.TestCase):
    """Integration tests for Dual-Pane mode inside FileManager."""

    def setUp(self):
        from onyxsh.filemanager.manager import FileManager
        from onyxsh.sessions.models import SessionItem

        self.mock_window = MagicMock()
        self.mock_window.toast_overlay = MagicMock()
        self.mock_tm = MagicMock()
        self.mock_settings = MagicMock()
        self.mock_settings.get.side_effect = lambda k, d=None: False if k == "file_manager_dual_pane_enabled" else (d or "")

        self.fm = FileManager(self.mock_window, self.mock_tm, self.mock_settings)
        self.fm.session_item = SessionItem("Remote-SSH", session_type="ssh", host="remote.lan", user="root")

    def tearDown(self):
        if hasattr(self, "fm") and self.fm:
            self.fm.destroy()

    def test_dual_pane_toggle_initialization(self):
        """Validates that dual_pane_toggle is created and responds to remote session."""
        self.assertIsNotNone(self.fm.dual_pane_toggle)
        self.assertFalse(self.fm.dual_pane_toggle.get_active())

        # Update for remote session type
        self.fm._update_action_bar_for_session_type()
        self.assertTrue(self.fm.dual_pane_toggle.get_visible())

    def test_enable_and_disable_dual_pane_mode(self):
        """Validates activating and deactivating dual pane mode."""
        self.assertFalse(self.fm._dual_pane_mode_active)

        # Enable
        self.fm._set_dual_pane_mode(True)
        self.assertTrue(self.fm._dual_pane_mode_active)
        self.assertIsNotNone(self.fm.local_pane)
        self.assertIsNotNone(self.fm.transfer_bar)
        self.assertIsNotNone(self.fm.dual_paned)
        self.mock_settings.set.assert_called_with("file_manager_dual_pane_enabled", True)

        # Disable
        self.fm._set_dual_pane_mode(False)
        self.assertFalse(self.fm._dual_pane_mode_active)
        self.mock_settings.set.assert_called_with("file_manager_dual_pane_enabled", False)

    def test_upload_and_download_actions_with_empty_selection(self):
        """Verifies friendly toasts when upload/download triggered without selection."""
        self.fm._set_dual_pane_mode(True)

        # Empty local selection
        self.fm.local_pane.get_selected_items = MagicMock(return_value=[])
        self.fm._on_dual_pane_upload()
        self.mock_window.toast_overlay.add_toast.assert_called()

        # Empty remote selection
        self.fm.get_selected_items = MagicMock(return_value=[])
        self.fm._on_dual_pane_download()
        self.mock_window.toast_overlay.add_toast.assert_called()

    def test_local_pane_destroy_and_stale_request_protection(self):
        """Validates that local_pane.destroy() cleans models and _populate_store rejects stale requests."""
        pane = LocalFileBrowserPane(initial_path=str(Path.home()))
        pane._request_counter = 5

        # Stale request id should be rejected
        res = pane._populate_store([], str(Path.home()), req_id=4)
        self.assertFalse(res)

        # Mismatched target path should be rejected
        res = pane._populate_store([], "/different/path", req_id=5)
        self.assertFalse(res)

        # Destroy should clean models
        pane.destroy()
        self.assertIsNone(pane.store)
        self.assertIsNone(pane.selection_model)

    def test_download_file_sync_session_override_and_port(self):
        """Validates that download_file_sync respects session_override and custom port."""
        from onyxsh.filemanager.operations import FileOperations
        from onyxsh.sessions.models import SessionItem

        ops = FileOperations(self.fm.session_item)

        custom_session = SessionItem(
            name="CustomSSH",
            session_type="ssh",
            host="custom.server.org",
            user="admin",
            port=2222,
        )

        with patch("subprocess.run") as mock_run:
            mock_run.return_value = MagicMock(returncode=0, stdout="OK", stderr="")
            success, msg = ops.download_file_sync(
                "/var/log/syslog",
                "/tmp/syslog",
                session_override=custom_session,
            )
            self.assertTrue(success)
            self.assertEqual(msg, "Success")
            cmd_args = mock_run.call_args[0][0]
            self.assertIn("-P", cmd_args)
            self.assertIn("2222", cmd_args)
            self.assertIn("admin@custom.server.org:/var/log/syslog", cmd_args)


if __name__ == "__main__":
    unittest.main()

