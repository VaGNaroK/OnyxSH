# onyxsh/ui/widgets/resource_dashboard.py
"""
Real-time System Resource Metrics Dashboard widget for OnyxSH.

Displays live CPU, RAM, Disk, and Network I/O metrics for local machine and remote SSH sessions
with sparklines, level gauges, and automatic background suspension.
"""

from typing import TYPE_CHECKING, Any, Dict, List, Optional, Tuple

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, GLib, Gtk, Pango

from ...system.metrics import (
    MetricsSnapshot,
    MetricsWorkerThread,
    format_bytes,
    format_rate,
)
from ...utils.icons import icon_button, icon_image
from ...utils.logger import get_logger
from ...utils.translation_utils import _
from .sparkline import SparklineCanvas

if TYPE_CHECKING:
    from ...window import CommTerminalWindow

logger = get_logger("onyxsh.ui.resource_dashboard")


class ResourceDashboardWidget(Gtk.Box):
    """
    Complete resource metrics dashboard panel with host switching,
    speed controls, and live sparklines.
    """

    def __init__(
        self,
        parent_window: Optional["CommTerminalWindow"] = None,
        default_interval: float = 2.0,
    ) -> None:
        super().__init__(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        self.parent_window = parent_window
        self._interval = default_interval
        self._available_sessions: List[Tuple[str, Optional[Any]]] = [
            (_("Local (Este Computador)"), None)
        ]
        self._is_paused_manually = False

        self.add_css_class("resource-dashboard-view")
        self.set_margin_start(12)
        self.set_margin_end(12)
        self.set_margin_top(10)
        self.set_margin_bottom(12)

        # Build UI structure
        self._build_header_controls()
        self._build_metrics_cards()

        # Initialize background worker thread
        self._worker = MetricsWorkerThread(
            callback=self._on_snapshot_received,
            interval=self._interval,
        )
        self._worker.start()

        # Connect lifecycle signals for zero-overhead suspension
        self.connect("map", self._on_widget_mapped)
        self.connect("unmap", self._on_widget_unmapped)

    def _on_widget_mapped(self, widget: Gtk.Widget) -> None:
        """Resumes active sampling when dashboard becomes visible."""
        self._refresh_available_sessions()
        if not self._is_paused_manually:
            self._worker.resume()

    def _on_widget_unmapped(self, widget: Gtk.Widget) -> None:
        """Suspends sampling immediately when dashboard is hidden or closed (0% CPU)."""
        self._worker.pause()

    def destroy_worker(self) -> None:
        """Permanently stops the background worker thread on window teardown."""
        if hasattr(self, "_worker") and self._worker:
            self._worker.stop()

    def _build_header_controls(self) -> None:
        """Constructs the top toolbar with host selector, interval, and pause toggle."""
        toolbar = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        toolbar.add_css_class("resource-dashboard-toolbar")

        # 1. Host DropDown
        host_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        host_icon = icon_image("computer-symbolic", size=16)
        host_box.append(host_icon)

        self.host_string_list = Gtk.StringList()
        self.host_string_list.append(_("Local (Este Computador)"))

        self.host_dropdown = Gtk.DropDown(model=self.host_string_list)
        self.host_dropdown.set_hexpand(True)
        self.host_dropdown.connect("notify::selected", self._on_host_selected)
        host_box.append(self.host_dropdown)
        toolbar.append(host_box)

        # 2. Status Badge
        self.status_badge = Gtk.Label(label=_("🟢 Ativo"))
        self.status_badge.add_css_class("caption")
        self.status_badge.add_css_class("dim-label")
        toolbar.append(self.status_badge)

        # 3. Sampling Interval Dropdown
        interval_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
        interval_label = Gtk.Label(label=_("Taxa:"))
        interval_label.add_css_class("caption")
        interval_label.add_css_class("dim-label")
        interval_box.append(interval_label)

        self.interval_string_list = Gtk.StringList()
        self.interval_string_list.append("1s")
        self.interval_string_list.append("2s")
        self.interval_string_list.append("5s")
        self.interval_dropdown = Gtk.DropDown(model=self.interval_string_list)
        # Default to 2s (index 1)
        self.interval_dropdown.set_selected(1)
        self.interval_dropdown.connect("notify::selected", self._on_interval_selected)
        interval_box.append(self.interval_dropdown)
        toolbar.append(interval_box)

        # 4. Pause / Resume Toggle
        self.pause_button = Gtk.ToggleButton()
        self.pause_button.set_child(icon_image("process-stop-symbolic", size=14))
        self.pause_button.add_css_class("flat")
        self.pause_button.set_tooltip_text(_("Pausar / Retomar atualizações em tempo real"))
        self.pause_button.connect("toggled", self._on_pause_toggled)
        toolbar.append(self.pause_button)

        self.append(toolbar)

    def _build_metrics_cards(self) -> None:
        """Constructs the scrollable area containing the 4 metric cards."""
        scrolled = Gtk.ScrolledWindow()
        scrolled.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        scrolled.set_vexpand(True)

        cards_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        cards_box.set_margin_top(4)
        cards_box.set_margin_bottom(4)

        # 1. CPU Card
        self.cpu_card = self._create_card(
            title=_("Processador (CPU)"),
            icon_name="utilities-terminal-symbolic",
        )
        self.cpu_val_label = Gtk.Label(label="--%")
        self.cpu_val_label.add_css_class("metric-value-title")
        self.cpu_sub_label = Gtk.Label(label=_("Carregando núcleos..."))
        self.cpu_sub_label.add_css_class("caption")
        self.cpu_sub_label.add_css_class("dim-label")

        self.cpu_sparkline = SparklineCanvas(
            max_points=30,
            line_color=(0.22, 0.58, 0.98),  # Azure blue
            fill_alpha=0.22,
            height_request=44,
        )
        self.cpu_sparkline.set_max_value(100.0)

        self.cpu_level_bar = Gtk.LevelBar()
        self.cpu_level_bar.set_min_value(0.0)
        self.cpu_level_bar.set_max_value(100.0)
        self.cpu_level_bar.set_value(0.0)
        self.cpu_level_bar.add_offset_value(Gtk.LEVEL_BAR_OFFSET_LOW, 60.0)
        self.cpu_level_bar.add_offset_value(Gtk.LEVEL_BAR_OFFSET_HIGH, 85.0)

        self.cpu_card.content_box.append(self._make_metric_row(self.cpu_val_label, self.cpu_sub_label))
        self.cpu_card.content_box.append(self.cpu_sparkline)
        self.cpu_card.content_box.append(self.cpu_level_bar)
        cards_box.append(self.cpu_card)

        # 2. RAM Card
        self.ram_card = self._create_card(
            title=_("Memória (RAM)"),
            icon_name="computer-symbolic",
        )
        self.ram_val_label = Gtk.Label(label="--%")
        self.ram_val_label.add_css_class("metric-value-title")
        self.ram_sub_label = Gtk.Label(label=_("Carregando memória..."))
        self.ram_sub_label.add_css_class("caption")
        self.ram_sub_label.add_css_class("dim-label")

        self.ram_sparkline = SparklineCanvas(
            max_points=30,
            line_color=(0.68, 0.35, 0.92),  # Purple
            fill_alpha=0.22,
            height_request=44,
        )
        self.ram_sparkline.set_max_value(100.0)

        self.ram_level_bar = Gtk.LevelBar()
        self.ram_level_bar.set_min_value(0.0)
        self.ram_level_bar.set_max_value(100.0)
        self.ram_level_bar.set_value(0.0)
        self.ram_level_bar.add_offset_value(Gtk.LEVEL_BAR_OFFSET_LOW, 70.0)
        self.ram_level_bar.add_offset_value(Gtk.LEVEL_BAR_OFFSET_HIGH, 90.0)

        self.swap_label = Gtk.Label(label="Swap: --")
        self.swap_label.add_css_class("caption")
        self.swap_label.add_css_class("dim-label")
        self.swap_label.set_halign(Gtk.Align.START)

        self.ram_card.content_box.append(self._make_metric_row(self.ram_val_label, self.ram_sub_label))
        self.ram_card.content_box.append(self.ram_sparkline)
        self.ram_card.content_box.append(self.ram_level_bar)
        self.ram_card.content_box.append(self.swap_label)
        cards_box.append(self.ram_card)

        # 3. Disk Card
        self.disk_card = self._create_card(
            title=_("Armazenamento (Disco)"),
            icon_name="folder-symbolic",
        )
        self.disk_val_label = Gtk.Label(label="--%")
        self.disk_val_label.add_css_class("metric-value-title")
        self.disk_sub_label = Gtk.Label(label=_("Carregando partição..."))
        self.disk_sub_label.add_css_class("caption")
        self.disk_sub_label.add_css_class("dim-label")

        self.disk_level_bar = Gtk.LevelBar()
        self.disk_level_bar.set_min_value(0.0)
        self.disk_level_bar.set_max_value(100.0)
        self.disk_level_bar.set_value(0.0)
        self.disk_level_bar.add_offset_value(Gtk.LEVEL_BAR_OFFSET_LOW, 75.0)
        self.disk_level_bar.add_offset_value(Gtk.LEVEL_BAR_OFFSET_HIGH, 90.0)

        self.disk_card.content_box.append(self._make_metric_row(self.disk_val_label, self.disk_sub_label))
        self.disk_card.content_box.append(self.disk_level_bar)
        cards_box.append(self.disk_card)

        # 4. Network I/O Card
        self.net_card = self._create_card(
            title=_("Tráfego de Rede"),
            icon_name="network-server-symbolic",
        )
        net_row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=16)
        self.net_rx_label = Gtk.Label(label="↓ 0.0 B/s")
        self.net_rx_label.add_css_class("metric-net-rx")
        self.net_tx_label = Gtk.Label(label="↑ 0.0 B/s")
        self.net_tx_label.add_css_class("metric-net-tx")
        net_row.append(self.net_rx_label)
        net_row.append(self.net_tx_label)

        self.net_sparkline = SparklineCanvas(
            max_points=30,
            line_color=(0.18, 0.80, 0.44),  # Emerald green (download)
            fill_alpha=0.20,
            secondary_color=(0.95, 0.60, 0.15),  # Amber orange (upload)
            height_request=44,
        )
        self.net_sparkline.enable_auto_scale()

        self.net_totals_label = Gtk.Label(label=_("Total: ↓ 0 B  ↑ 0 B"))
        self.net_totals_label.add_css_class("caption")
        self.net_totals_label.add_css_class("dim-label")
        self.net_totals_label.set_halign(Gtk.Align.START)

        self.net_card.content_box.append(net_row)
        self.net_card.content_box.append(self.net_sparkline)
        self.net_card.content_box.append(self.net_totals_label)
        cards_box.append(self.net_card)

        scrolled.set_child(cards_box)
        self.append(scrolled)

    @staticmethod
    def _create_card(title: str, icon_name: str) -> Gtk.Box:
        """Constructs an individual styled card container."""
        card = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        card.add_css_class("resource-metric-card")

        header = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        header.append(icon_image(icon_name, size=14))
        lbl = Gtk.Label(label=title)
        lbl.add_css_class("card-title")
        lbl.set_hexpand(True)
        lbl.set_halign(Gtk.Align.START)
        header.append(lbl)
        card.append(header)

        content_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        card.append(content_box)
        card.content_box = content_box
        return card

    @staticmethod
    def _make_metric_row(val_label: Gtk.Label, sub_label: Gtk.Label) -> Gtk.Box:
        """Combines headline value and descriptive subtitle."""
        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        val_label.set_halign(Gtk.Align.START)
        sub_label.set_halign(Gtk.Align.START)
        sub_label.set_hexpand(True)
        box.append(val_label)
        box.append(sub_label)
        return box

    def _refresh_available_sessions(self) -> None:
        """Queries active SSH sessions from the window's terminal registry and populates dropdown."""
        if not self.parent_window or not hasattr(self.parent_window, "terminal_manager"):
            return

        current_selected = self.host_dropdown.get_selected()
        sessions: List[Tuple[str, Optional[Any]]] = [(_("Local (Este Computador)"), None)]

        try:
            tm = self.parent_window.terminal_manager
            all_ids = tm.registry.get_all_terminal_ids()
            seen_hosts = set()
            for tid in all_ids:
                info = tm.registry.get_terminal_info(tid)
                if not info:
                    continue
                ident = info.get("identifier")
                if hasattr(ident, "session_type") and ident.session_type == "ssh":
                    name = getattr(ident, "name", "") or getattr(ident, "host", "SSH")
                    if name not in seen_hosts:
                        seen_hosts.add(name)
                        sessions.append((f"🌐 {name} (SSH)", ident))
        except Exception as e:
            logger.debug(f"Could not refresh active sessions: {e}")

        # Update model if changed
        if len(sessions) != len(self._available_sessions):
            self._available_sessions = sessions
            new_list = Gtk.StringList()
            for title, _item in sessions:
                new_list.append(title)
            self.host_string_list = new_list
            self.host_dropdown.set_model(new_list)
            if current_selected < len(sessions):
                self.host_dropdown.set_selected(current_selected)
            else:
                self.host_dropdown.set_selected(0)

    def _on_host_selected(self, dropdown: Gtk.DropDown, _param: Any) -> None:
        """Handles user selection of host (Local vs Remote SSH)."""
        idx = dropdown.get_selected()
        if 0 <= idx < len(self._available_sessions):
            _name, session = self._available_sessions[idx]
            self._worker.set_target_session(session)
            # Clear sparklines on host change
            self.cpu_sparkline.clear()
            self.ram_sparkline.clear()
            self.net_sparkline.clear()
            self.status_badge.set_text(_("🟡 Conectando..."))

    def _on_interval_selected(self, dropdown: Gtk.DropDown, _param: Any) -> None:
        """Adjusts the sampling frequency."""
        idx = dropdown.get_selected()
        intervals = [1.0, 2.0, 5.0]
        if 0 <= idx < len(intervals):
            self._interval = intervals[idx]
            self._worker.set_interval(self._interval)

    def _on_pause_toggled(self, button: Gtk.ToggleButton) -> None:
        """Pauses or resumes live updates."""
        self._is_paused_manually = button.get_active()
        if self._is_paused_manually:
            self._worker.pause()
            self.status_badge.set_text(_("⏸️ Pausado"))
        else:
            self._worker.resume()
            self.status_badge.set_text(_("🟢 Ativo"))

    def _on_snapshot_received(self, snapshot: MetricsSnapshot) -> bool:
        """Dispatched on GTK main thread to update UI labels, gauges, and sparklines."""
        if not snapshot.is_connected:
            self.status_badge.set_text(f"🔴 {snapshot.error_message or _('Erro')}")
            return GLib.SOURCE_REMOVE

        self.status_badge.set_text(_("🟢 Conectado") if snapshot.is_remote else _("🟢 Ativo"))

        # 1. Update CPU
        if snapshot.cpu:
            self.cpu_val_label.set_text(f"{snapshot.cpu.usage_percent:.1f}%")
            l1, l5, l15 = snapshot.cpu.load_avg
            self.cpu_sub_label.set_text(
                f"{snapshot.cpu.core_count} {_('núcleos')} • {_('Carga')}: {l1:.2f}, {l5:.2f}, {l15:.2f}"
            )
            self.cpu_sparkline.push_value(snapshot.cpu.usage_percent)
            self.cpu_level_bar.set_value(snapshot.cpu.usage_percent)

        # 2. Update RAM
        if snapshot.memory:
            self.ram_val_label.set_text(f"{snapshot.memory.used_percent:.1f}%")
            used_str = format_bytes(snapshot.memory.used_bytes)
            total_str = format_bytes(snapshot.memory.total_bytes)
            free_str = format_bytes(snapshot.memory.available_bytes)
            self.ram_sub_label.set_text(f"{used_str} / {total_str} ({free_str} {_('livres')})")
            self.ram_sparkline.push_value(snapshot.memory.used_percent)
            self.ram_level_bar.set_value(snapshot.memory.used_percent)

            swap_used = format_bytes(snapshot.memory.swap_used_bytes)
            swap_total = format_bytes(snapshot.memory.swap_total_bytes)
            self.swap_label.set_text(
                f"Swap: {swap_used} / {swap_total} ({snapshot.memory.swap_percent:.1f}%)"
            )

        # 3. Update Disk
        if snapshot.disk:
            self.disk_val_label.set_text(f"{snapshot.disk.used_percent:.1f}%")
            d_used = format_bytes(snapshot.disk.used_bytes)
            d_total = format_bytes(snapshot.disk.total_bytes)
            d_free = format_bytes(snapshot.disk.free_bytes)
            self.disk_sub_label.set_text(
                f"{_('Montagem')}: {snapshot.disk.mount_point} • {d_used} / {d_total} ({d_free} {_('livres')})"
            )
            self.disk_level_bar.set_value(snapshot.disk.used_percent)

        # 4. Update Network
        if snapshot.network:
            rx_str = format_rate(snapshot.network.bytes_recv_rate)
            tx_str = format_rate(snapshot.network.bytes_sent_rate)
            self.net_rx_label.set_text(f"↓ {rx_str}")
            self.net_tx_label.set_text(f"↑ {tx_str}")

            self.net_sparkline.push_value(
                snapshot.network.bytes_recv_rate,
                snapshot.network.bytes_sent_rate,
            )

            tot_rx = format_bytes(snapshot.network.total_recv_bytes)
            tot_tx = format_bytes(snapshot.network.total_sent_bytes)
            self.net_totals_label.set_text(f"{_('Total acumulado')}: ↓ {tot_rx}   ↑ {tot_tx}")

        return GLib.SOURCE_REMOVE
