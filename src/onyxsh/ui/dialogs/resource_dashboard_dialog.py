# onyxsh/ui/dialogs/resource_dashboard_dialog.py
"""
Non-modal floating window for the Real-Time System Resource Metrics Dashboard in OnyxSH.
Allows the user to monitor host resources side-by-side without interrupting terminal typing.
"""

from typing import Optional

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gtk

from ...utils.logger import get_logger
from ...utils.translation_utils import _
from ..widgets.resource_dashboard import ResourceDashboardWidget

logger = get_logger("onyxsh.ui.dialogs.resource_dashboard")


class ResourceDashboardDialog(Adw.Window):
    """
    Floating, non-modal window displaying real-time CPU, RAM, Disk, and Network metrics.
    """

    _instance: Optional["ResourceDashboardDialog"] = None

    def __init__(self, parent_window: Optional[Gtk.Window] = None) -> None:
        props = {
            "title": _("Monitor de Recursos do Sistema"),
            "modal": False,
            "default_width": 480,
            "default_height": 620,
        }
        if parent_window and isinstance(parent_window, Gtk.Window):
            props["transient_for"] = parent_window

        super().__init__(**props)
        self.add_css_class("resource-dashboard-dialog")
        self.parent_window = parent_window

        # Build main layout
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        self.set_content(main_box)

        # Header Bar
        header = Adw.HeaderBar()
        main_box.append(header)

        # Dashboard Widget
        self.dashboard_widget = ResourceDashboardWidget(parent_window=parent_window)
        main_box.append(self.dashboard_widget)

        # Clean up worker on close
        self.connect("close-request", self._on_close_request)

    def _on_close_request(self, window: Gtk.Window) -> bool:
        """Stops worker thread and cleans singleton instance."""
        if hasattr(self, "dashboard_widget") and self.dashboard_widget:
            self.dashboard_widget.destroy_worker()
        ResourceDashboardDialog._instance = None
        return False  # Allow window destruction

    @classmethod
    def show_dashboard(cls, parent_window: Optional[Gtk.Window] = None) -> "ResourceDashboardDialog":
        """Singleton helper: opens or focuses the resource monitor window."""
        if cls._instance is not None:
            try:
                cls._instance.present()
                return cls._instance
            except Exception:
                cls._instance = None

        dialog = cls(parent_window=parent_window)
        cls._instance = dialog
        dialog.present()
        return dialog
