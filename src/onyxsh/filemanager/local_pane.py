"""Local file browser pane component for OnyxSH Dual-Pane mode."""

from __future__ import annotations

import grp
import os
import pwd
import shutil
import stat
import time
from datetime import datetime
from pathlib import Path
from typing import Callable, Dict, List, Optional, Tuple

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gdk, Gio, GLib, GObject, Gtk, Pango

from ..core.tasks import AsyncTaskManager
from ..utils.icons import icon_button, icon_image
from ..utils.logger import get_logger
from ..utils.tooltip_helper import get_tooltip_helper
from ..utils.translation_utils import _
from .models import FileItem

_LOCAL_UID_CACHE: Dict[int, str] = {}
_LOCAL_GID_CACHE: Dict[int, str] = {}


def _resolve_user_name(uid: int) -> str:
    if uid not in _LOCAL_UID_CACHE:
        try:
            _LOCAL_UID_CACHE[uid] = pwd.getpwuid(uid).pw_name
        except Exception:
            _LOCAL_UID_CACHE[uid] = str(uid)
    return _LOCAL_UID_CACHE[uid]


def _resolve_group_name(gid: int) -> str:
    if gid not in _LOCAL_GID_CACHE:
        try:
            _LOCAL_GID_CACHE[gid] = grp.getgrgid(gid).gr_name
        except Exception:
            _LOCAL_GID_CACHE[gid] = str(gid)
    return _LOCAL_GID_CACHE[gid]


class LocalFileBrowserPane(GObject.Object):
    """Encapsulates a self-contained local filesystem browser pane.

    Provides fast directory browsing, search filtering, multi-selection,
    breadcrumb navigation, and Drag & Drop capabilities for Dual-Pane mode.
    """

    __gsignals__ = {
        "path-changed": (GObject.SignalFlags.RUN_FIRST, None, (str,)),
        "selection-changed": (GObject.SignalFlags.RUN_FIRST, None, ()),
        "item-activated": (GObject.SignalFlags.RUN_FIRST, None, (object,)),
        "files-dropped": (GObject.SignalFlags.RUN_FIRST, None, (object,)),
    }

    def __init__(self, initial_path: Optional[str] = None, show_hidden: bool = False):
        super().__init__()
        self.logger = get_logger("onyxsh.filemanager.local_pane")
        self.tooltip_helper = get_tooltip_helper()

        # State
        if initial_path and os.path.exists(initial_path) and os.path.isdir(initial_path):
            self.current_path = os.path.abspath(initial_path)
        else:
            self.current_path = str(Path.home())

        self.show_hidden = show_hidden
        self._request_counter: int = 0
        self._dir_cache: Dict[str, Tuple[float, List[FileItem]]] = {}
        self._DIR_CACHE_TTL: float = 3.0
        self._DIR_CACHE_MAX: int = 30

        # Model & Stores
        self.store = Gio.ListStore.new(FileItem)
        self.filter = Gtk.CustomFilter.new(self._filter_func)
        self.filtered_store = Gtk.FilterListModel.new(self.store, self.filter)

        self.sorter = Gtk.CustomSorter.new(self._sort_func)
        self.sorted_store = Gtk.SortListModel.new(self.filtered_store, self.sorter)

        self.selection_model = Gtk.MultiSelection.new(self.sorted_store)
        self.selection_model.connect("selection-changed", self._on_selection_changed)

        # Build UI
        self._build_ui()

        # Initial listing
        self.refresh()

    def get_widget(self) -> Gtk.Widget:
        """Returns the root widget for this pane."""
        return self.main_box

    def _build_ui(self):
        self.main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        self.main_box.add_css_class("local-browser-pane")
        self.main_box.set_hexpand(True)
        self.main_box.set_vexpand(True)

        # ── 1. Top Navigation & Header Bar ────────────────────────────────────
        self.header_bar = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        self.header_bar.add_css_class("dual-pane-header")
        self.header_bar.set_margin_start(6)
        self.header_bar.set_margin_end(6)
        self.header_bar.set_margin_top(4)
        self.header_bar.set_margin_bottom(4)

        # Title / Identifier Chip
        self.title_chip = Gtk.Label(label=_("💻 Local"))
        self.title_chip.add_css_class("pane-title-badge")
        self.title_chip.add_css_class("caption")
        self.header_bar.append(self.title_chip)

        # Navigation Buttons (Home, Up, Refresh)
        nav_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=2)
        nav_box.add_css_class("linked")

        self.home_btn = icon_button("go-home-symbolic")
        self.tooltip_helper.add_tooltip(self.home_btn, _("Home Directory"))
        self.home_btn.connect("clicked", lambda _: self.go_home())
        nav_box.append(self.home_btn)

        self.up_btn = icon_button("go-up-symbolic")
        self.tooltip_helper.add_tooltip(self.up_btn, _("Up One Level"))
        self.up_btn.connect("clicked", lambda _: self.go_up())
        nav_box.append(self.up_btn)

        self.refresh_btn = icon_button("view-refresh-symbolic")
        self.tooltip_helper.add_tooltip(self.refresh_btn, _("Refresh"))
        self.refresh_btn.connect("clicked", lambda _: self.refresh())
        nav_box.append(self.refresh_btn)

        self.header_bar.append(nav_box)

        # Breadcrumbs Box
        self.breadcrumb_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=2)
        self.breadcrumb_box.set_hexpand(True)
        self.breadcrumb_box.add_css_class("breadcrumb-trail")
        self.header_bar.append(self.breadcrumb_box)

        # Search Entry for filtering
        self.search_entry = Gtk.SearchEntry()
        self.search_entry.set_placeholder_text(_("Filter local..."))
        self.search_entry.set_max_width_chars(12)
        self.search_entry.connect("search-changed", self._on_search_changed)
        self.header_bar.append(self.search_entry)

        self.main_box.append(self.header_bar)

        # ── 2. Files Column View ──────────────────────────────────────────────
        self.scrolled_window = Gtk.ScrolledWindow()
        self.scrolled_window.set_hexpand(True)
        self.scrolled_window.set_vexpand(True)
        self.scrolled_window.add_css_class("background")

        self.column_view = Gtk.ColumnView.new(self.selection_model)
        self.column_view.add_css_class("file-manager-column-view")
        self.column_view.set_show_column_separators(False)
        self.column_view.set_show_row_separators(True)
        self.column_view.connect("activate", self._on_row_activated)

        # Setup Columns
        self._setup_columns()

        self.scrolled_window.set_child(self.column_view)
        self.main_box.append(self.scrolled_window)

        # ── 3. Drag and Drop ──────────────────────────────────────────────────
        self._setup_drag_and_drop()

        # ── 4. Bottom Status Bar ──────────────────────────────────────────────
        self.status_bar = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        self.status_bar.add_css_class("file-manager-status-bar")
        self.status_bar.set_margin_start(8)
        self.status_bar.set_margin_end(8)
        self.status_bar.set_margin_top(2)
        self.status_bar.set_margin_bottom(2)

        self.status_label = Gtk.Label(label="", xalign=0.0)
        self.status_label.add_css_class("file-manager-status-label")
        self.status_label.set_hexpand(True)
        self.status_bar.append(self.status_label)

        self.main_box.append(self.status_bar)

    def _setup_columns(self):
        # Column 1: Name & Icon
        factory_name = Gtk.SignalListItemFactory()
        factory_name.connect("setup", self._on_name_factory_setup)
        factory_name.connect("bind", self._on_name_factory_bind)
        col_name = Gtk.ColumnViewColumn.new(_("Name"), factory_name)
        col_name.set_expand(True)
        self.column_view.append_column(col_name)

        # Column 2: Size
        factory_size = Gtk.SignalListItemFactory()
        factory_size.connect("setup", self._on_size_factory_setup)
        factory_size.connect("bind", self._on_size_factory_bind)
        col_size = Gtk.ColumnViewColumn.new(_("Size"), factory_size)
        col_size.set_fixed_width(85)
        self.column_view.append_column(col_size)

        # Column 3: Date
        factory_date = Gtk.SignalListItemFactory()
        factory_date.connect("setup", self._on_date_factory_setup)
        factory_date.connect("bind", self._on_date_factory_bind)
        col_date = Gtk.ColumnViewColumn.new(_("Modified"), factory_date)
        col_date.set_fixed_width(120)
        self.column_view.append_column(col_date)

        # Column 4: Permissions
        factory_perms = Gtk.SignalListItemFactory()
        factory_perms.connect("setup", self._on_perms_factory_setup)
        factory_perms.connect("bind", self._on_perms_factory_bind)
        col_perms = Gtk.ColumnViewColumn.new(_("Permissions"), factory_perms)
        col_perms.set_fixed_width(95)
        self.column_view.append_column(col_perms)

    # ── Column Factories ──────────────────────────────────────────────────────

    def _on_name_factory_setup(self, factory, list_item):
        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        box.set_margin_start(4)
        box.set_margin_end(4)

        icon = Gtk.Image()
        icon.set_pixel_size(16)
        label = Gtk.Label()
        label.set_ellipsize(Pango.EllipsizeMode.END)
        label.set_xalign(0.0)

        box.append(icon)
        box.append(label)
        list_item.set_child(box)

    def _on_name_factory_bind(self, factory, list_item):
        box = list_item.get_child()
        icon = box.get_first_child()
        label = icon.get_next_sibling()
        item: FileItem = list_item.get_item()

        if item:
            icon.set_from_icon_name(item.icon_name or "text-x-generic")
            label.set_text(item.name)
            if item.is_directory_like:
                label.add_css_class("heading")
            else:
                label.remove_css_class("heading")

    def _on_size_factory_setup(self, factory, list_item):
        label = Gtk.Label(xalign=1.0)
        label.add_css_class("dim-label")
        list_item.set_child(label)

    def _on_size_factory_bind(self, factory, list_item):
        label = list_item.get_child()
        item: FileItem = list_item.get_item()
        if item:
            if item.is_directory_like:
                label.set_text("—")
            else:
                label.set_text(self._format_size(item.size))

    def _on_date_factory_setup(self, factory, list_item):
        label = Gtk.Label(xalign=0.0)
        label.add_css_class("dim-label")
        list_item.set_child(label)

    def _on_date_factory_bind(self, factory, list_item):
        label = list_item.get_child()
        item: FileItem = list_item.get_item()
        if item and hasattr(item, "date"):
            if isinstance(item.date, datetime):
                label.set_text(item.date.strftime("%Y-%m-%d %H:%M"))
            else:
                label.set_text(str(item.date)[:16])

    def _on_perms_factory_setup(self, factory, list_item):
        label = Gtk.Label(xalign=0.0)
        label.add_css_class("dim-label")
        label.add_css_class("monospace")
        list_item.set_child(label)

    def _on_perms_factory_bind(self, factory, list_item):
        label = list_item.get_child()
        item: FileItem = list_item.get_item()
        if item:
            label.set_text(item.permissions or "")

    # ── Drag and Drop ─────────────────────────────────────────────────────────

    def _setup_drag_and_drop(self):
        # 1. Drag Source (Export local files to remote or other apps)
        drag_source = Gtk.DragSource.new()
        drag_source.set_actions(Gdk.DragAction.COPY)
        drag_source.connect("prepare", self._on_drag_prepare)
        self.column_view.add_controller(drag_source)

        # 2. Drop Target (Accept external files or files dropped onto this pane)
        drop_target = Gtk.DropTarget.new(Gdk.FileList, Gdk.DragAction.COPY)
        drop_target.connect("enter", self._on_drop_enter)
        drop_target.connect("leave", self._on_drop_leave)
        drop_target.connect("drop", self._on_drop_performed)
        self.scrolled_window.add_controller(drop_target)

    def _on_drag_prepare(self, source, x, y):
        selected_paths = self.get_selected_paths()
        if not selected_paths:
            return None

        gio_files = [Gio.File.new_for_path(str(p)) for p in selected_paths]
        try:
            file_list = Gdk.FileList.new_from_list(gio_files)
            val = GObject.Value(Gdk.FileList, file_list)
            return Gdk.ContentProvider.new_for_value(val)
        except Exception as e:
            self.logger.warning(f"Failed to create DragSource ContentProvider: {e}")
            return None

    def _on_drop_enter(self, target, x, y):
        self.scrolled_window.add_css_class("drop-target")
        return Gdk.DragAction.COPY

    def _on_drop_leave(self, target):
        self.scrolled_window.remove_css_class("drop-target")

    def _on_drop_performed(self, drop_target, value, x, y):
        self.scrolled_window.remove_css_class("drop-target")
        if isinstance(value, Gdk.FileList):
            dropped_paths = []
            for gio_file in value.get_files():
                if p := gio_file.get_path():
                    dropped_paths.append(Path(p))
            if dropped_paths:
                self.emit("files-dropped", dropped_paths)
                return True
        return False

    # ── Filtering & Sorting ───────────────────────────────────────────────────

    def _filter_func(self, item: FileItem, user_data=None) -> bool:
        if not item:
            return False
        # Parent directory ".." is always shown
        if item.name == "..":
            return True

        # Hidden files
        if not self.show_hidden and item.name.startswith("."):
            return False

        # Search query filter
        search_text = self.search_entry.get_text().strip().lower()
        if search_text:
            return search_text in item.name.lower()
        return True

    def _sort_func(self, a: FileItem, b: FileItem, user_data=None) -> int:
        # ".." always stays first
        if a.name == "..":
            return -1
        if b.name == "..":
            return 1

        # Directories always come before regular files
        if a.is_directory_like and not b.is_directory_like:
            return -1
        if not a.is_directory_like and b.is_directory_like:
            return 1

        # Alphabetical case-insensitive sort
        name_a = a.name.lower()
        name_b = b.name.lower()
        if name_a < name_b:
            return -1
        elif name_a > name_b:
            return 1
        return 0

    def _on_search_changed(self, entry):
        self.filter.changed(Gtk.FilterChange.DIFFERENT)
        self._update_status_bar()

    def set_show_hidden(self, show_hidden: bool):
        if self.show_hidden != show_hidden:
            self.show_hidden = show_hidden
            self.filter.changed(Gtk.FilterChange.DIFFERENT)
            self._update_status_bar()

    # ── Navigation & Listing ──────────────────────────────────────────────────

    def navigate_to(self, path: str):
        target = os.path.abspath(path)
        if not os.path.exists(target) or not os.path.isdir(target):
            self.logger.warning(f"Local path does not exist: {target}")
            return
        self.current_path = target
        self._update_breadcrumbs()
        self.refresh()
        self.emit("path-changed", self.current_path)

    def go_home(self):
        self.navigate_to(str(Path.home()))

    def go_up(self):
        parent = os.path.dirname(self.current_path)
        if parent and parent != self.current_path:
            self.navigate_to(parent)

    def refresh(self):
        self._request_counter += 1
        req_id = self._request_counter
        self._update_breadcrumbs()

        # Check cache
        now = time.monotonic()
        cached = self._dir_cache.get(self.current_path)
        if cached and (now - cached[0] <= self._DIR_CACHE_TTL):
            self._populate_store(cached[1], self.current_path, req_id)
            return

        target_path = self.current_path
        AsyncTaskManager.get().submit_io(self._list_directory_worker, target_path, req_id)

    def _list_directory_worker(self, target_path: str, req_id: int):
        try:
            items: List[FileItem] = []
            parent_path = os.path.dirname(target_path)
            if target_path != "/" and parent_path:
                try:
                    st_p = os.stat(parent_path)
                    dt_p = datetime.fromtimestamp(st_p.st_mtime)
                    items.append(
                        FileItem(
                            "..",
                            stat.filemode(st_p.st_mode),
                            st_p.st_size,
                            dt_p,
                            _resolve_user_name(st_p.st_uid),
                            _resolve_group_name(st_p.st_gid),
                        )
                    )
                except Exception:
                    items.append(
                        FileItem("..", "drwxr-xr-x", 4096, datetime.now(), "user", "group")
                    )

            with os.scandir(target_path) as entries:
                for entry in entries:
                    try:
                        st = entry.stat(follow_symlinks=False)
                        perms = stat.filemode(st.st_mode)
                        is_link = entry.is_symlink()
                        link_target = ""
                        if is_link:
                            try:
                                link_target = os.readlink(entry.path)
                            except Exception:
                                pass
                        dt = datetime.fromtimestamp(st.st_mtime)
                        is_dir = entry.is_dir(follow_symlinks=False)
                        name = entry.name + ("/" if is_dir else "")

                        item = FileItem(
                            name,
                            perms,
                            st.st_size,
                            dt,
                            _resolve_user_name(st.st_uid),
                            _resolve_group_name(st.st_gid),
                            is_link=is_link,
                            link_target=link_target,
                        )
                        items.append(item)
                    except (PermissionError, FileNotFoundError):
                        continue

            # Update cache
            if len(self._dir_cache) >= self._DIR_CACHE_MAX:
                oldest_key = min(self._dir_cache, key=lambda k: self._dir_cache[k][0])
                del self._dir_cache[oldest_key]
            self._dir_cache[target_path] = (time.monotonic(), items)

            GLib.idle_add(self._populate_store, items, target_path, req_id)
        except Exception as e:
            self.logger.error(f"Error reading local directory {target_path}: {e}")
            GLib.idle_add(self._populate_store, [], target_path, req_id)

    def _populate_store(
        self, items: List[FileItem], target_path: str, req_id: int
    ) -> bool:
        if getattr(self, "_is_destroyed", False):
            return False
        if req_id != self._request_counter or target_path != self.current_path:
            return False
        if not hasattr(self, "store") or self.store is None:
            return False
        self.store.remove_all()
        for item in items:
            self.store.append(item)
        self._update_status_bar()
        return False

    def _update_breadcrumbs(self):
        # Remove existing buttons
        while child := self.breadcrumb_box.get_first_child():
            self.breadcrumb_box.remove(child)

        path = Path(self.current_path)
        parts = ["/"] + [p for p in path.parts if p and p != "/"]

        accumulated = "/"
        for i, part in enumerate(parts):
            if i > 0:
                accumulated = os.path.join(accumulated, part)

            btn = Gtk.Button(label=part if part != "/" else "/")
            btn.add_css_class("flat")
            btn.add_css_class("breadcrumb-item")
            target = accumulated
            btn.connect("clicked", lambda _, p=target: self.navigate_to(p))
            self.breadcrumb_box.append(btn)

            if i < len(parts) - 1:
                sep = Gtk.Label(label="›")
                sep.add_css_class("dim-label")
                self.breadcrumb_box.append(sep)

    def _on_row_activated(self, column_view, position: int):
        item: Optional[FileItem] = self.sorted_store.get_item(position)
        if not item:
            return

        if item.name == "..":
            self.go_up()
        elif item.is_directory_like:
            dir_name = item.name.rstrip("/")
            new_path = os.path.join(self.current_path, dir_name)
            self.navigate_to(new_path)
        else:
            self.emit("item-activated", item)

    def _on_selection_changed(self, model, position, n_items):
        self._update_status_bar()
        self.emit("selection-changed")

    def _update_status_bar(self):
        total_items = self.filtered_store.get_n_items()
        selected_count = 0
        bitset = self.selection_model.get_selection()
        if bitset:
            selected_count = bitset.get_size()

        free_bytes = 0
        try:
            usage = shutil.disk_usage(self.current_path)
            free_bytes = usage.free
        except Exception:
            pass

        free_str = self._format_size(free_bytes)
        if selected_count > 0:
            text = _("{selected} of {total} items selected • {free} free").format(
                selected=selected_count, total=total_items, free=free_str
            )
        else:
            text = _("{total} items • {free} free").format(
                total=total_items, free=free_str
            )
        self.status_label.set_text(text)

    # ── Public Getters ────────────────────────────────────────────────────────

    def get_selected_items(self) -> List[FileItem]:
        """Returns list of currently selected FileItems (excluding '..')."""
        selected_items = []
        bitset = self.selection_model.get_selection()
        if not bitset:
            return selected_items

        size = bitset.get_size()
        for i in range(size):
            pos = bitset.get_nth(i)
            item: Optional[FileItem] = self.sorted_store.get_item(pos)
            if item and item.name != "..":
                selected_items.append(item)
        return selected_items

    def get_selected_paths(self) -> List[Path]:
        """Returns list of Paths for selected items."""
        paths = []
        for item in self.get_selected_items():
            name = item.name.rstrip("/")
            paths.append(Path(os.path.join(self.current_path, name)))
        return paths

    @staticmethod
    def _format_size(size_bytes: int) -> str:
        if size_bytes < 1024:
            return f"{size_bytes} B"
        elif size_bytes < 1024 * 1024:
            return f"{size_bytes / 1024:.1f} KB"
        elif size_bytes < 1024 * 1024 * 1024:
            return f"{size_bytes / (1024 * 1024):.1f} MB"
        else:
            return f"{size_bytes / (1024 * 1024 * 1024):.1f} GB"

    def destroy(self) -> None:
        """Explicitly releases GTK models and clear references to prevent leaks."""
        self._is_destroyed = True
        if hasattr(self, "column_view") and self.column_view is not None:
            try:
                self.column_view.set_model(None)
            except Exception:
                pass
            self.column_view = None
        if hasattr(self, "selection_model"):
            self.selection_model = None
        if hasattr(self, "sorted_store"):
            self.sorted_store = None
        if hasattr(self, "filtered_store"):
            self.filtered_store = None
        if hasattr(self, "store") and self.store is not None:
            try:
                self.store.remove_all()
            except Exception:
                pass
            self.store = None
        self._dir_cache.clear()
