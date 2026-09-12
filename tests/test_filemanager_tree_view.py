# tests/test_filemanager_tree_view.py
"""Comprehensive unit tests for FileManager Hierarchical Tree View in OnyxSH."""

import os
import tempfile
import time
import unittest
from datetime import datetime
from unittest.mock import MagicMock, patch

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gio, GLib, GObject, Gtk

from onyxsh.filemanager.manager import FileManager
from onyxsh.filemanager.models import FileItem
from onyxsh.sessions.models import SessionItem
from onyxsh.utils.translation_utils import _


class TestFileManagerTreeView(unittest.TestCase):
    """Unit tests for Tree View model, widget factory, expansion, and recursive metrics."""

    def setUp(self):
        self.fm = FileManager.__new__(FileManager)
        self.fm.logger = MagicMock()
        self.fm._is_destroyed = False
        self.fm.current_path = "/home/user"
        self.fm.settings_manager = MagicMock()
        self.fm.settings_manager.get.return_value = "tree"
        self.fm.session_item = SessionItem(name="Local", session_type="local")
        self.fm.operations = None
        self.fm.bound_terminal = None
        self.fm._dir_metrics_cache = {}
        self.fm._dir_metrics_calculating = set()
        self.fm.tooltip_helper = MagicMock()

        # Models setup
        self.fm.store = Gio.ListStore.new(FileItem)
        self.fm.filtered_store = Gtk.FilterListModel(model=self.fm.store)
        self.fm._ensure_sorters()
        self.fm.sorted_store = Gtk.SortListModel(
            model=self.fm.filtered_store, sorter=self.fm.name_sorter
        )

        self.fm.tree_view = self.fm._create_tree_view()
        self.fm._current_view_mode = "tree"

    def tearDown(self):
        self.fm._is_destroyed = True

    def test_file_item_tree_properties_and_signal(self):
        dt = datetime(2026, 8, 25, 12, 0, 0)
        file_item = FileItem(
            name="script.py",
            perms="-rwxr-xr-x",
            size=1024,
            date=dt,
            owner="user",
            group="user",
            full_path="/home/user/script.py",
            parent_path="/home/user",
        )

        self.assertEqual(file_item.full_path, "/home/user/script.py")
        self.assertEqual(file_item.parent_path, "/home/user")
        self.assertEqual(file_item.tree_size_summary, "1.0 KB")

        # Test directory item and recursive metrics
        dir_item = FileItem(
            name="projects",
            perms="drwxr-xr-x",
            size=4096,
            date=dt,
            owner="user",
            group="user",
            full_path="/home/user/projects",
            parent_path="/home/user",
        )
        self.assertTrue(dir_item.is_directory)
        self.assertEqual(dir_item.tree_size_summary, _("Calculating..."))

        # Test metrics-updated signal
        updated_event = []
        dir_item.connect("metrics-updated", lambda itm: updated_event.append(itm.recursive_size))

        dir_item.recursive_size = 5242880  # 5.0 MB
        dir_item.item_count = 25
        self.assertEqual(dir_item.formatted_recursive_size, "5.0 MB")
        self.assertIn("5.0 MB", dir_item.tree_size_summary)
        self.assertIn("25", dir_item.tree_size_summary)

        dir_item.emit("metrics-updated")
        self.assertEqual(len(updated_event), 1)
        self.assertEqual(updated_event[0], 5242880)

    def test_tree_view_model_and_widgets_structure(self):
        self.assertIsInstance(self.fm.tree_model, Gtk.TreeListModel)
        self.assertIsInstance(self.fm.tree_selection_model, Gtk.MultiSelection)
        self.assertIsInstance(self.fm.tree_view, Gtk.ListView)
        self.assertEqual(self.fm.tree_model.get_n_items(), 0)

        # Add items to root store
        root_dir = FileItem("src", "drwxr-xr-x", 4096, datetime.now(), "u", "g", full_path="/home/user/src")
        root_file = FileItem("README.md", "-rw-r--r--", 500, datetime.now(), "u", "g", full_path="/home/user/README.md")
        self.fm.store.append(root_dir)
        self.fm.store.append(root_file)

        self.assertEqual(self.fm.tree_model.get_n_items(), 2)

        # First row is a TreeListRow
        row0 = self.fm.tree_model.get_item(0)
        self.assertIsInstance(row0, Gtk.TreeListRow)
        self.assertEqual(row0.get_depth(), 0)

    def test_tree_item_factory_setup_bind_unbind(self):
        factory = Gtk.SignalListItemFactory()
        list_item = Gtk.ListItem()

        self.fm._setup_tree_item(factory, list_item)
        row_box = list_item.get_child()
        self.assertIsInstance(row_box, Gtk.Box)
        expander = row_box.get_first_child()
        self.assertIsInstance(expander, Gtk.TreeExpander)

        dir_item = FileItem("docs", "drwxr-xr-x", 4096, datetime.now(), "u", "g", full_path="/home/user/docs")
        dir_item.recursive_size = 1048576
        dir_item.item_count = 10

        self.fm.store.append(dir_item)
        tree_row = self.fm.tree_model.get_item(0)

        mock_item = MagicMock()
        mock_item.get_child.return_value = row_box
        mock_item.get_item.return_value = tree_row

        self.fm._bind_tree_item(factory, mock_item)

        size_label = expander.get_next_sibling()
        self.assertIn("1.0 MB", size_label.get_text())

        # Unbind test
        self.fm._unbind_tree_item(factory, mock_item)
        self.assertIsNone(getattr(row_box, "_metrics_handler_id", None))

    def test_tree_children_model_creation_local(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            sub_folder = os.path.join(temp_dir, "sub_folder")
            os.makedirs(sub_folder, exist_ok=True)
            test_file = os.path.join(sub_folder, "test_file.txt")
            with open(test_file, "w") as f:
                f.write("hello tree")

            folder_item = FileItem(
                name="sub_folder",
                perms="drwxr-xr-x",
                size=4096,
                date=datetime.now(),
                owner="u",
                group="g",
                full_path=sub_folder,
                parent_path=temp_dir,
            )

            child_model = self.fm._tree_create_children_model(folder_item)
            self.assertIsNotNone(child_model)
            self.assertEqual(child_model.get_n_items(), 1)
            child_item = child_model.get_item(0)
            self.assertEqual(child_item.name, "test_file.txt")
            self.assertEqual(child_item.full_path, test_file)
            self.assertEqual(child_item.parent_path, sub_folder)

    def test_tree_recursive_metrics_calculation_local(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            f1 = os.path.join(temp_dir, "f1.bin")
            with open(f1, "wb") as f:
                f.write(b"A" * 1000)

            sub = os.path.join(temp_dir, "nested")
            os.makedirs(sub, exist_ok=True)
            f2 = os.path.join(sub, "f2.bin")
            with open(f2, "wb") as f:
                f.write(b"B" * 2500)

            folder_item = FileItem(
                name="temp",
                perms="drwxr-xr-x",
                size=4096,
                date=datetime.now(),
                owner="u",
                group="g",
                full_path=temp_dir,
            )

            # Test calculation
            self.fm._calculate_tree_dir_metrics_async(folder_item)

            # Wait for background thread
            timeout = 3.0
            start = time.time()
            while folder_item.recursive_size is None and (time.time() - start) < timeout:
                time.sleep(0.05)

            self.assertEqual(folder_item.recursive_size, 3500)
            self.assertEqual(folder_item.item_count, 3)  # f1.bin, nested, f2.bin

    def test_tree_view_selection_unwrapping(self):
        item1 = FileItem("f1.txt", "-rw-r--r--", 100, datetime.now(), "u", "g", full_path="/home/user/f1.txt")
        item2 = FileItem("f2.txt", "-rw-r--r--", 200, datetime.now(), "u", "g", full_path="/home/user/f2.txt")
        self.fm.store.append(item1)
        self.fm.store.append(item2)

        # Select first item in tree_selection_model
        self.fm.tree_selection_model.select_item(0, True)

        selected = self.fm.get_selected_items()
        self.assertEqual(len(selected), 1)
        self.assertIsInstance(selected[0], FileItem)
        self.assertEqual(selected[0].name, "f1.txt")

    def test_tree_view_keyboard_expand_collapse(self):
        dir_item = FileItem("code", "drwxr-xr-x", 4096, datetime.now(), "u", "g", full_path="/home/user/code")
        self.fm.store.append(dir_item)

        self.fm.tree_selection_model.select_item(0, True)
        tree_row = self.fm.tree_model.get_item(0)
        self.assertFalse(tree_row.get_expanded())

        # Press Right arrow to expand
        res_right = self.fm._on_column_view_key_pressed(None, gi.repository.Gdk.KEY_Right, 0, 0)
        self.assertEqual(res_right, gi.repository.Gdk.EVENT_STOP)
        self.assertTrue(tree_row.get_expanded())

        # Press Left arrow to collapse
        res_left = self.fm._on_column_view_key_pressed(None, gi.repository.Gdk.KEY_Left, 0, 0)
        self.assertEqual(res_left, gi.repository.Gdk.EVENT_STOP)
        self.assertFalse(tree_row.get_expanded())

    def test_on_row_activated_toggles_tree_expansion(self):
        dir_item = FileItem("scripts", "drwxr-xr-x", 4096, datetime.now(), "u", "g", full_path="/home/user/scripts")
        self.fm.store.append(dir_item)

        tree_row = self.fm.tree_model.get_item(0)
        self.assertFalse(tree_row.get_expanded())

        # Calling _on_row_activated on tree_view row 0 should toggle expansion
        self.fm._on_row_activated(self.fm.tree_view, 0)
        self.assertTrue(tree_row.get_expanded())

        self.fm._on_row_activated(self.fm.tree_view, 0)
        self.assertFalse(tree_row.get_expanded())


if __name__ == "__main__":
    unittest.main()
