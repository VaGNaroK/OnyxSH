# onyxsh/ui/dialogs/checksum_dialog.py
"""
Modern Libadwaita Dialog for cryptographic checksum calculation and hash comparison.
Provides async multialgorithm hashing (SHA-256, SHA-512, MD5, SHA-1), live matching,
clipboard copying, and direct terminal command injection.
"""

import shlex
import threading
from pathlib import Path
from typing import Dict, Optional

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, GLib, Gtk, Pango

from ...utils.checksum_utils import (
    calculate_file_hashes,
    compare_hash,
    detect_hash_type,
    format_checksum_report,
)
from ...utils.logger import get_logger
from ...utils.translation_utils import _
from .base_dialog import BaseDialog


class ChecksumDialog(BaseDialog):
    """Modern Libadwaita Dialog for File Checksum Calculation and Hash Verification."""

    def __init__(
        self,
        parent_window: Gtk.Window,
        file_path: str,
        file_name: Optional[str] = None,
        file_size_str: Optional[str] = None,
        bound_terminal=None,
    ) -> None:
        super().__init__(
            parent_window=parent_window,
            dialog_title=_("Checksums & Integrity Verification"),
            auto_setup_toolbar=True,
            default_width=720,
            default_height=600,
        )
        self.logger = get_logger("onyxsh.ui.dialogs.checksum_dialog")
        self.file_path = file_path
        self.file_name = file_name or Path(file_path).name
        self.file_size_str = file_size_str or self._get_formatted_size()
        self.bound_terminal = bound_terminal

        self._computed_hashes: Dict[str, str] = {}
        self._cancel_event = threading.Event()
        self._is_calculating = False
        self._hash_labels: Dict[str, Gtk.Label] = {}
        self._hash_rows: Dict[str, Adw.ActionRow] = {}

        self._setup_ui()
        self.connect("close-request", self._on_dialog_close)

        # Start async hash calculation
        self._start_hash_calculation()

    def _get_formatted_size(self) -> str:
        """Helper to format file size in human-readable form."""
        try:
            sz = Path(self.file_path).stat().st_size
            for unit in ("B", "KB", "MB", "GB", "TB"):
                if sz < 1024.0 or unit == "TB":
                    return f"{sz:.1f} {unit}" if unit != "B" else f"{sz} B"
                sz /= 1024.0
        except Exception:
            return _("Unknown size")
        return _("Unknown size")

    def _setup_ui(self) -> None:
        """Constructs the Libadwaita dialog layout."""
        self.add_css_class("checksum-dialog")

        # Outer Scrolled Window
        scrolled = Gtk.ScrolledWindow()
        scrolled.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        scrolled.set_vexpand(True)

        clamp = Adw.Clamp()
        clamp.set_maximum_size(680)
        clamp.set_tightening_threshold(540)

        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=18)
        main_box.set_margin_top(18)
        main_box.set_margin_bottom(18)
        main_box.set_margin_start(18)
        main_box.set_margin_end(18)

        # 1. File Information Header Card
        file_group = Adw.PreferencesGroup()
        file_row = Adw.ActionRow()
        file_row.set_title(self.file_name)
        file_row.set_subtitle(f"{self.file_size_str} • {self.file_path}")
        file_row.add_prefix(Gtk.Image.new_from_icon_name("document-properties-symbolic"))
        file_group.add(file_row)
        main_box.append(file_group)

        # 2. Progress Indicator (during calculation)
        self.progress_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        self.progress_bar = Gtk.ProgressBar()
        self.progress_bar.set_show_text(True)
        self.progress_bar.set_text(_("Calculating hashes..."))
        self.progress_box.append(self.progress_bar)
        main_box.append(self.progress_box)

        # 3. Hashes Display Preferences Group
        self.hashes_group = Adw.PreferencesGroup()
        self.hashes_group.set_title(_("Cryptographic Hashes"))
        self.hashes_group.set_description(
            _("Standard digests calculated across the file content:")
        )

        algo_descriptions = {
            "sha256": (_("SHA-256"), _("Modern standard for integrity and downloads")),
            "sha512": (_("SHA-512"), _("High security 512-bit cryptographic digest")),
            "md5": (_("MD5"), _("Legacy checksum for fast verification")),
            "sha1": (_("SHA-1"), _("Git and legacy repository compatibility")),
        }

        for algo, (title, desc) in algo_descriptions.items():
            row = Adw.ActionRow()
            row.set_title(f"{title} ({algo.upper()})")
            row.set_subtitle(desc)

            # Monospace hash label in subtitle or custom widget
            hash_label = Gtk.Label(label=_("Calculating..."))
            hash_label.set_selectable(True)
            hash_label.add_css_class("monospace")
            hash_label.set_xalign(0)
            hash_label.set_ellipsize(Pango.EllipsizeMode.MIDDLE)

            copy_btn = Gtk.Button(icon_name="edit-copy-symbolic")
            copy_btn.set_tooltip_text(_("Copy hash to clipboard"))
            copy_btn.add_css_class("flat")
            copy_btn.connect("clicked", self._on_copy_single_hash, algo)

            row.add_suffix(hash_label)
            row.add_suffix(copy_btn)

            self._hash_labels[algo] = hash_label
            self._hash_rows[algo] = row
            self.hashes_group.add(row)

        main_box.append(self.hashes_group)

        # 4. Hash Matcher & Comparison Group
        matcher_group = Adw.PreferencesGroup()
        matcher_group.set_title(_("Integrity Matcher & Comparison"))
        matcher_group.set_description(
            _("Paste the expected hash provided by the author to verify authenticity:")
        )

        self.match_entry = Adw.EntryRow()
        self.match_entry.set_title(_("Expected Hash"))
        self.match_entry.connect("changed", self._on_expected_hash_changed)
        matcher_group.add(self.match_entry)

        # Match status feedback banner card
        self.status_banner_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        self.status_banner_box.set_margin_top(8)
        self.status_banner_box.set_margin_bottom(4)
        self.status_banner_box.add_css_class("card")
        self.status_banner_box.set_margin_start(2)
        self.status_banner_box.set_margin_end(2)

        self.status_icon = Gtk.Image.new_from_icon_name("dialog-information-symbolic")
        self.status_icon.set_margin_start(12)
        self.status_icon.set_margin_top(10)
        self.status_icon.set_margin_bottom(10)

        self.status_label = Gtk.Label(
            label=_(
                "Paste an MD5, SHA-256 or SHA-512 hash above to verify authenticity."
            )
        )
        self.status_label.set_wrap(True)
        self.status_label.set_xalign(0)
        self.status_label.set_margin_end(12)
        self.status_label.set_margin_top(10)
        self.status_label.set_margin_bottom(10)

        self.status_banner_box.append(self.status_icon)
        self.status_banner_box.append(self.status_label)
        matcher_group.add(self.status_banner_box)

        main_box.append(matcher_group)

        # 5. Bottom Action Buttons Bar
        actions_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        actions_box.set_halign(Gtk.Align.END)
        actions_box.set_margin_top(12)

        self.copy_report_btn = Gtk.Button()
        copy_content = Adw.ButtonContent(
            icon_name="edit-copy-symbolic",
            label=_("Copy Full Report"),
        )
        self.copy_report_btn.set_child(copy_content)
        self.copy_report_btn.set_tooltip_text(_("Copy Full Report"))
        self.copy_report_btn.connect("clicked", self._on_copy_report_clicked)
        actions_box.append(self.copy_report_btn)

        if self.bound_terminal:
            self.terminal_btn = Gtk.Button()
            terminal_content = Adw.ButtonContent(
                icon_name="utilities-terminal-symbolic",
                label=_("Insert sha256sum into Terminal"),
            )
            self.terminal_btn.set_child(terminal_content)
            self.terminal_btn.set_tooltip_text(_("Insert sha256sum into Terminal"))
            self.terminal_btn.connect("clicked", self._on_insert_terminal_clicked)
            actions_box.append(self.terminal_btn)

        main_box.append(actions_box)

        clamp.set_child(main_box)
        scrolled.set_child(clamp)

        if self._toolbar_view:
            self._toolbar_view.set_content(scrolled)
        else:
            self.set_content(scrolled)

    def _start_hash_calculation(self) -> None:
        """Dispatches async calculation in background thread."""
        self._is_calculating = True
        self._cancel_event.clear()

        def progress_cb(bytes_read: int, total_bytes: int, pct: float):
            GLib.idle_add(self._update_progress_ui, bytes_read, total_bytes, pct)

        def worker():
            try:
                hashes = calculate_file_hashes(
                    self.file_path,
                    progress_callback=progress_cb,
                    cancel_event=self._cancel_event,
                )
                GLib.idle_add(self._on_hashes_computed, hashes)
            except Exception as e:
                self.logger.warning(f"Error computing checksums: {e}")
                GLib.idle_add(self._on_hashes_error, str(e))

        threading.Thread(target=worker, daemon=True).start()

    def _update_progress_ui(self, bytes_read: int, total_bytes: int, pct: float) -> bool:
        """Updates progress bar on main UI thread."""
        if not self._is_calculating:
            return GLib.SOURCE_REMOVE

        self.progress_bar.set_fraction(pct)
        if total_bytes > 0:
            read_mb = bytes_read / (1024 * 1024)
            total_mb = total_bytes / (1024 * 1024)
            self.progress_bar.set_text(
                f"{read_mb:.1f} MB / {total_mb:.1f} MB ({int(pct * 100)}%)"
            )
        return GLib.SOURCE_REMOVE

    def _on_hashes_computed(self, hashes: Dict[str, str]) -> bool:
        """Called when hashing completes successfully."""
        self._is_calculating = False
        self._computed_hashes = hashes

        # Hide progress bar box
        self.progress_box.set_visible(False)

        # Update labels
        for algo, val in hashes.items():
            if algo in self._hash_labels:
                lbl = self._hash_labels[algo]
                lbl.set_text(val)

        # If user already typed an expected hash, evaluate it
        self._on_expected_hash_changed(self.match_entry)
        return GLib.SOURCE_REMOVE

    def _on_hashes_error(self, err_msg: str) -> bool:
        """Called if calculation fails."""
        self._is_calculating = False
        self.progress_box.set_visible(False)

        for lbl in self._hash_labels.values():
            lbl.set_text(_("Error calculating hash"))

        self.status_icon.set_from_icon_name("dialog-error-symbolic")
        self.status_label.set_text(
            _("Failed to calculate checksums: {error}").format(error=err_msg)
        )
        return GLib.SOURCE_REMOVE

    def _on_expected_hash_changed(self, entry: Adw.EntryRow) -> None:
        """Live evaluation of expected hash against computed hashes."""
        text = entry.get_text().strip()

        if not text:
            self.status_icon.set_from_icon_name("dialog-information-symbolic")
            self.status_label.set_text(
                _("Paste an MD5, SHA-256 or SHA-512 hash above to verify authenticity.")
            )
            # Remove any highlight styles
            for row in self._hash_rows.values():
                row.remove_css_class("accent")
            return

        if not self._computed_hashes:
            self.status_icon.set_from_icon_name("dialog-information-symbolic")
            self.status_label.set_text(_("Calculating hashes in progress..."))
            return

        detected_algo = detect_hash_type(text)
        is_match, matched_algo = compare_hash(self._computed_hashes, text)

        for row in self._hash_rows.values():
            row.remove_css_class("accent")

        if is_match and matched_algo:
            algo_upper = matched_algo.upper()
            self.status_icon.set_from_icon_name("emblem-ok-symbolic")
            self.status_label.set_text(
                _("✅ HASH MATCHES ({algo})! The file is authentic and intact.").format(
                    algo=algo_upper
                )
            )
            if matched_algo in self._hash_rows:
                self._hash_rows[matched_algo].add_css_class("accent")
        else:
            hint = f" ({detected_algo.upper()})" if detected_algo else ""
            self.status_icon.set_from_icon_name("dialog-warning-symbolic")
            self.status_label.set_text(
                _(
                    "❌ HASH DOES NOT MATCH! The pasted value{hint} differs from the computed file checksums. File may be corrupted or modified."
                ).format(hint=hint)
            )

    def _on_copy_single_hash(self, _btn: Gtk.Button, algo: str) -> None:
        """Copies single algorithm hash to system clipboard."""
        val = self._computed_hashes.get(algo)
        if not val:
            return

        clipboard = self.get_clipboard()
        clipboard.set(val)
        self._show_toast(_("{algo} hash copied to clipboard").format(algo=algo.upper()))

    def _on_copy_report_clicked(self, _btn: Gtk.Button) -> None:
        """Copies full formatted checksum report to clipboard."""
        if not self._computed_hashes:
            return

        report = format_checksum_report(
            self.file_name, self.file_path, self.file_size_str, self._computed_hashes
        )
        clipboard = self.get_clipboard()
        clipboard.set(report)
        self._show_toast(_("Full checksum report copied to clipboard"))

    def _on_insert_terminal_clicked(self, _btn: Gtk.Button) -> None:
        """Injects sha256sum command into active terminal."""
        if not self.bound_terminal:
            return

        escaped = shlex.quote(self.file_path)
        cmd = f"sha256sum {escaped}\n"
        self.bound_terminal.feed_child(cmd.encode("utf-8"))
        self.bound_terminal.grab_focus()
        self._show_toast(_("sha256sum command sent to terminal"))
        self.close()

    def _show_toast(self, message: str) -> None:
        """Displays non-intrusive toast on parent window."""
        if (
            self.parent_window
            and hasattr(self.parent_window, "toast_overlay")
            and self.parent_window.toast_overlay
        ):
            self.parent_window.toast_overlay.add_toast(Adw.Toast(title=message))
        else:
            self.logger.info(f"Toast: {message}")

    def _on_dialog_close(self, _dialog) -> bool:
        """Signals background thread to cancel on window close."""
        self._cancel_event.set()
        return False
