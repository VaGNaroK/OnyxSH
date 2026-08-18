# onyxsh/ui/widgets/production_banner.py
"""
Production Guard Banner widget for Libadwaita / GTK4.
Displays a persistent, high-visibility security banner at the top of terminal tabs
connected to production environments.
"""

from typing import Optional

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gdk, GLib, GObject, Gtk

from ...utils.logger import get_logger
from ...utils.translation_utils import _


class ProductionBanner(Gtk.Box):
    """
    Persistent visual banner for production environments.
    Anchored to the top of the terminal view inside a tab.
    """

    __gtype_name__ = "ProductionBanner"

    def __init__(
        self,
        session_name: str = "",
        host: str = "",
        terminal_id: Optional[int] = None,
    ):
        super().__init__(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        self.logger = get_logger("onyxsh.ui.widgets.production_banner")
        self.session_name = session_name
        self.host = host
        self.terminal_id = terminal_id

        self.add_css_class("production-guard-banner")
        self.set_margin_start(0)
        self.set_margin_end(0)
        self.set_margin_top(0)
        self.set_margin_bottom(0)

        self._setup_ui()

    def _setup_ui(self) -> None:
        # Left container: Icon + Title + Host info
        left_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        left_box.set_hexpand(True)
        left_box.set_margin_start(12)
        left_box.set_margin_top(4)
        left_box.set_margin_bottom(4)

        icon = Gtk.Image.new_from_icon_name("security-high-symbolic")
        icon.set_pixel_size(16)
        icon.add_css_class("production-guard-icon")
        left_box.append(icon)

        title_label = Gtk.Label(
            label=_("PRODUCTION ENVIRONMENT"),
            xalign=0.0,
            css_classes=["production-guard-title", "heading-4"],
        )
        left_box.append(title_label)

        display_target = self.host or self.session_name or _("Remote Host")
        host_label = Gtk.Label(
            label=f"•  {display_target}",
            xalign=0.0,
            css_classes=["production-guard-host", "caption"],
        )
        left_box.append(host_label)

        self.append(left_box)

        # Right container: Shield Badge + Info Button
        right_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        right_box.set_margin_end(12)
        right_box.set_margin_top(4)
        right_box.set_margin_bottom(4)

        guard_badge = Gtk.Label(
            label=_("🛡️ Guard Active"),
            css_classes=["production-guard-badge", "caption"],
        )
        guard_badge.set_tooltip_text(
            _("Destructive commands (rm -rf, dd, mkfs, shutdown, etc.) require hostname confirmation.")
        )
        right_box.append(guard_badge)

        info_btn = Gtk.Button(
            icon_name="help-about-symbolic",
            css_classes=["flat", "circular", "production-guard-info-btn"],
            tooltip_text=_("View Production Guard Policies"),
        )
        info_btn.connect("clicked", self._on_info_clicked)
        right_box.append(info_btn)

        self.append(right_box)

    def _on_info_clicked(self, button: Gtk.Button) -> None:
        """Shows popover explaining production safety policies."""
        popover = Gtk.Popover()
        popover.set_parent(button)
        popover.set_autohide(True)

        box = Gtk.Box(
            orientation=Gtk.Orientation.VERTICAL,
            spacing=8,
            margin_start=12,
            margin_end=12,
            margin_top=10,
            margin_bottom=10,
        )
        box.set_size_request(280, -1)

        header = Gtk.Label(
            label=f"🛡️ {_('Production Guard Policies')}",
            xalign=0.0,
            css_classes=["heading-4"],
        )
        box.append(header)

        desc = Gtk.Label(
            label=_(
                "This terminal is connected to a verified production system.\n\n"
                "• Destructive file deletions (rm -rf) require confirmation.\n"
                "• Disk and filesystem modifications (dd, mkfs) are gated.\n"
                "• Service shutdowns & reboots require host verification.\n"
                "• AI assistant output sharing is redacted and protected."
            ),
            wrap=True,
            xalign=0.0,
            css_classes=["caption", "dim-label"],
        )
        box.append(desc)

        popover.set_child(box)
        popover.popup()
