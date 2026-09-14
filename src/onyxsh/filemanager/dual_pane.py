"""Dual-Pane components and utilities for OnyxSH File Manager."""

from __future__ import annotations

import difflib
import os
import tempfile
from pathlib import Path
from typing import Callable, Optional

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, GLib, GObject, Gtk

from ..core.tasks import AsyncTaskManager
from ..sessions.models import SessionItem
from ..ui.dialogs.diff_review_dialog import DiffReviewDialog
from ..utils.icons import icon_button
from ..utils.logger import get_logger
from ..utils.tooltip_helper import get_tooltip_helper
from ..utils.translation_utils import _


class DualPaneTransferBar(Gtk.Box):
    """Central vertical action bar between Local and Remote panes in Dual-Pane mode."""

    def __init__(
        self,
        on_upload: Optional[Callable[[], None]] = None,
        on_download: Optional[Callable[[], None]] = None,
        on_diff: Optional[Callable[[], None]] = None,
    ):
        super().__init__(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        self.logger = get_logger("onyxsh.filemanager.dual_pane")
        self.tooltip_helper = get_tooltip_helper()

        self.on_upload = on_upload
        self.on_download = on_download
        self.on_diff = on_diff

        self.add_css_class("dual-pane-transfer-bar")
        self.set_valign(Gtk.Align.CENTER)
        self.set_halign(Gtk.Align.CENTER)
        self.set_margin_start(4)
        self.set_margin_end(4)

        # Upload Button [ ➔ ]
        self.upload_button = icon_button("go-next-symbolic")
        self.upload_button.add_css_class("transfer-action-button")
        self.upload_button.add_css_class("upload-button")
        self.tooltip_helper.add_tooltip(
            self.upload_button, _("Upload Selected to Remote Folder (➔)")
        )
        self.upload_button.connect("clicked", self._on_upload_clicked)
        self.append(self.upload_button)

        # Download Button [ ⬅ ]
        self.download_button = icon_button("go-previous-symbolic")
        self.download_button.add_css_class("transfer-action-button")
        self.download_button.add_css_class("download-button")
        self.tooltip_helper.add_tooltip(
            self.download_button, _("Download Selected to Local Folder (⬅)")
        )
        self.download_button.connect("clicked", self._on_download_clicked)
        self.append(self.download_button)

        # Diff Button [ ⇄ ]
        self.diff_button = icon_button("view-dual-symbolic")
        self.diff_button.add_css_class("transfer-action-button")
        self.diff_button.add_css_class("diff-button")
        self.tooltip_helper.add_tooltip(
            self.diff_button, _("Compare Files (Diff Local ⇄ Remote)")
        )
        self.diff_button.connect("clicked", self._on_diff_clicked)
        self.append(self.diff_button)

    def _on_upload_clicked(self, button):
        if self.on_upload:
            self.on_upload()

    def _on_download_clicked(self, button):
        if self.on_download:
            self.on_download()

    def _on_diff_clicked(self, button):
        if self.on_diff:
            self.on_diff()


class RemoteDiffHelper:
    """Helper to safely fetch, compare, and display diffs between Local and Remote files."""

    MAX_DIFF_SIZE = 5 * 1024 * 1024  # 5 MB limit for text diffs

    @classmethod
    def compare_files_async(
        cls,
        parent_window: Gtk.Window,
        local_path: Path,
        remote_path: str,
        operations,
        session_item: SessionItem,
        on_error: Optional[Callable[[str], None]] = None,
        on_identical: Optional[Callable[[], None]] = None,
    ):
        """Asynchronously fetches remote file, compares with local, and displays DiffReviewDialog."""
        logger = get_logger("onyxsh.filemanager.diff")

        def worker():
            try:
                # 1. Validate local file
                if not local_path.exists():
                    msg = _("Local file not found: {}").format(local_path)
                    GLib.idle_add(cls._show_error, parent_window, msg, on_error)
                    return

                if local_path.is_dir():
                    msg = _("Cannot compare directories directly.")
                    GLib.idle_add(cls._show_error, parent_window, msg, on_error)
                    return

                if local_path.stat().st_size > cls.MAX_DIFF_SIZE:
                    msg = _("Local file is too large for text diff (> 5MB).")
                    GLib.idle_add(cls._show_error, parent_window, msg, on_error)
                    return

                # Read local text
                try:
                    local_bytes = local_path.read_bytes()
                    if b"\x00" in local_bytes[:8192]:
                        msg = _("Diff is only supported for text and code files (binary detected).")
                        GLib.idle_add(cls._show_error, parent_window, msg, on_error)
                        return
                    local_text = local_bytes.decode("utf-8", errors="replace")
                except Exception as e:
                    msg = _("Error reading local file: {}").format(e)
                    GLib.idle_add(cls._show_error, parent_window, msg, on_error)
                    return

                # 2. Download remote file to temporary location
                with tempfile.TemporaryDirectory(prefix="onyxsh_diff_") as tmpdir:
                    tmp_remote_file = Path(tmpdir) / local_path.name
                    res = operations.download_file_sync(
                        remote_path,
                        str(tmp_remote_file),
                        session_override=session_item,
                    )
                    if isinstance(res, tuple):
                        success, err_msg = res
                    else:
                        success, err_msg = bool(res), ""

                    if not success or not tmp_remote_file.exists():
                        detail = f": {err_msg}" if err_msg and err_msg != "Success" else f": {remote_path}"
                        msg = _("Failed to download remote file for comparison: {}").format(detail.lstrip(": "))
                        GLib.idle_add(cls._show_error, parent_window, msg, on_error)
                        return

                    if tmp_remote_file.stat().st_size > cls.MAX_DIFF_SIZE:
                        msg = _("Remote file is too large for text diff (> 5MB).")
                        GLib.idle_add(cls._show_error, parent_window, msg, on_error)
                        return

                    remote_bytes = tmp_remote_file.read_bytes()
                    if b"\x00" in remote_bytes[:8192]:
                        msg = _("Diff is only supported for text and code files (binary detected).")
                        GLib.idle_add(cls._show_error, parent_window, msg, on_error)
                        return
                    remote_text = remote_bytes.decode("utf-8", errors="replace")

                # 3. Generate unified diff
                local_lines = local_text.splitlines(keepends=True)
                remote_lines = remote_text.splitlines(keepends=True)

                diff_generator = difflib.unified_diff(
                    local_lines,
                    remote_lines,
                    fromfile=f"local://{local_path.name}",
                    tofile=f"remote://{Path(remote_path).name}",
                    lineterm="",
                )
                diff_lines = list(diff_generator)

                if not diff_lines:
                    # Files are identical!
                    def notify_identical():
                        if on_identical:
                            on_identical()
                        elif hasattr(parent_window, "toast_overlay"):
                            toast = Adw.Toast.new(_("Local and remote files are identical."))
                            toast.set_timeout(3)
                            parent_window.toast_overlay.add_toast(toast)
                        return False

                    GLib.idle_add(notify_identical)
                    return

                diff_text = "\n".join(diff_lines)

                # 4. Display in DiffReviewDialog
                def present_diff():
                    dialog = DiffReviewDialog(
                        parent_window=parent_window,
                        target_path=f"{local_path.name} ⇄ {Path(remote_path).name}",
                        diff_text=diff_text,
                    )
                    dialog.present()
                    return False

                GLib.idle_add(present_diff)

            except Exception as e:
                logger.error(f"Diff execution error: {e}")
                GLib.idle_add(cls._show_error, parent_window, str(e), on_error)

        AsyncTaskManager.get().submit_io(worker)

    @staticmethod
    def _show_error(parent_window, message: str, callback: Optional[Callable[[str], None]] = None):
        if callback:
            callback(message)
        elif hasattr(parent_window, "toast_overlay"):
            toast = Adw.Toast.new(message)
            toast.set_timeout(4)
            parent_window.toast_overlay.add_toast(toast)
        elif hasattr(parent_window, "_show_error_dialog"):
            parent_window._show_error_dialog(_("Diff Comparison Error"), message)
        return False
