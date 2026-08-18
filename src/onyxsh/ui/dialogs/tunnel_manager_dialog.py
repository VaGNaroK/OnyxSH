# onyxsh/ui/dialogs/tunnel_manager_dialog.py
"""
Visual SSH Tunnel Manager & Port Forwarding Dialog for OnyxSH.
Allows users to monitor, start, stop, configure, and manage all background
SSH port forwarding tunnels and SOCKS5 proxies in real time.
"""

from typing import Any, Dict, List, Optional

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, GLib, Gtk, Pango

from ...sessions.models import SessionItem
from ...terminal.tunnel_manager import SSHTunnel, get_ssh_tunnel_manager
from ...utils.logger import get_logger
from ...utils.translation_utils import _
from .tunnel_edit_dialog import TunnelEditDialog


class TunnelManagerDialog(Adw.Window):
    """
    Management interface for viewing and controlling SSH tunnels.
    """

    def __init__(
        self,
        parent_window: Optional[Gtk.Window],
        session_store=None,
    ) -> None:
        super().__init__()
        self.logger = get_logger("onyxsh.ui.dialogs.tunnel_manager")
        self.session_store = session_store
        self.tunnel_manager = get_ssh_tunnel_manager()
        self._signal_handlers: List[int] = []

        self.set_title(_("SSH Tunnel Manager"))
        self.set_default_size(720, 580)
        self.set_modal(False)
        self.set_transient_for(parent_window)

        self._setup_ui()
        self._connect_manager_signals()
        self._populate_tunnels()
        self.connect("close-request", self._on_close)

    def _setup_ui(self) -> None:
        toolbar_view = Adw.ToolbarView()
        self.set_content(toolbar_view)

        # Header Bar
        header = Adw.HeaderBar()
        toolbar_view.add_top_bar(header)

        # Add Tunnel Button
        add_btn = Gtk.Button(
            icon_name="list-add-symbolic",
            tooltip_text=_("Add New Tunnel"),
            css_classes=["suggested-action"],
        )
        add_btn.connect("clicked", self._on_add_tunnel_clicked)
        header.pack_start(add_btn)

        # Stop All Button
        stop_all_btn = Gtk.Button(
            icon_name="media-playback-stop-symbolic",
            tooltip_text=_("Stop All Active Tunnels"),
            css_classes=["flat"],
        )
        stop_all_btn.connect("clicked", lambda _: self.tunnel_manager.stop_all_tunnels())
        header.pack_end(stop_all_btn)

        # Search Bar
        search_bar = Gtk.SearchBar()
        self.search_entry = Gtk.SearchEntry(
            placeholder_text=_("Search tunnels by name, port or host..."),
            hexpand=True,
        )
        self.search_entry.connect("search-changed", lambda _: self._filter_tunnels())
        search_bar.set_child(self.search_entry)
        search_bar.set_key_capture_widget(self)
        search_bar.set_search_mode(True)
        toolbar_view.add_top_bar(search_bar)

        # Content
        scrolled = Gtk.ScrolledWindow()
        scrolled.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        toolbar_view.set_content(scrolled)

        main_box = Gtk.Box(
            orientation=Gtk.Orientation.VERTICAL,
            spacing=16,
            margin_start=18,
            margin_end=18,
            margin_top=16,
            margin_bottom=18,
        )
        scrolled.set_child(main_box)

        # Tunnel List Container
        self.tunnel_list = Gtk.ListBox()
        self.tunnel_list.set_selection_mode(Gtk.SelectionMode.NONE)
        self.tunnel_list.add_css_class("boxed-list")
        main_box.append(self.tunnel_list)

        # Placeholder when empty
        self.status_page = Adw.StatusPage(
            icon_name="network-workgroup-symbolic",
            title=_("No SSH Tunnels"),
            description=_(
                "Create a Local (-L), Remote (-R) or Dynamic SOCKS5 (-D) tunnel to forward network ports through SSH."
            ),
        )
        self.status_page.set_vexpand(True)
        main_box.append(self.status_page)

    def _connect_manager_signals(self) -> None:
        h1 = self.tunnel_manager.connect(
            "tunnel-status-changed", self._on_tunnel_status_changed
        )
        h2 = self.tunnel_manager.connect("tunnel-added", lambda *_: self._populate_tunnels())
        h3 = self.tunnel_manager.connect("tunnel-removed", lambda *_: self._populate_tunnels())
        self._signal_handlers = [h1, h2, h3]

    def _on_close(self, _window: Gtk.Window) -> bool:
        for handler_id in self._signal_handlers:
            if GObject.signal_handler_is_connected(self.tunnel_manager, handler_id):
                self.tunnel_manager.disconnect(handler_id)
        self._signal_handlers.clear()
        return False

    def _on_tunnel_status_changed(
        self, _manager: Any, tunnel_id: str, status: str
    ) -> None:
        GLib.idle_add(self._update_tunnel_row_status, tunnel_id, status)

    def _update_tunnel_row_status(self, tunnel_id: str, status: str) -> None:
        child = self.tunnel_list.get_first_child()
        while child:
            if getattr(child, "_tunnel_id", None) == tunnel_id:
                tunnel = self.tunnel_manager.get_tunnel(tunnel_id)
                if tunnel and hasattr(child, "_status_dot"):
                    self._apply_status_styling(child._status_dot, child._switch, tunnel)
                break
            child = child.get_next_sibling()

    def _apply_status_styling(
        self, status_dot: Gtk.Widget, switch: Gtk.Switch, tunnel: SSHTunnel
    ) -> None:
        status_dot.remove_css_class("success")
        status_dot.remove_css_class("error")
        status_dot.remove_css_class("warning")
        status_dot.remove_css_class("dim-label")

        switch.handler_block_by_func(self._on_switch_toggled)

        if tunnel.status == "active":
            status_dot.add_css_class("success")
            status_dot.set_tooltip_text(_("Active (Listening on {src})").format(src=tunnel.get_display_source()))
            switch.set_active(True)
        elif tunnel.status == "starting":
            status_dot.add_css_class("warning")
            status_dot.set_tooltip_text(_("Connecting..."))
            switch.set_active(True)
        elif tunnel.status == "error":
            status_dot.add_css_class("error")
            status_dot.set_tooltip_text(f"{_('Error')}: {tunnel.error_message}")
            switch.set_active(False)
        else:
            status_dot.add_css_class("dim-label")
            status_dot.set_tooltip_text(_("Stopped"))
            switch.set_active(False)

        switch.handler_unblock_by_func(self._on_switch_toggled)

    def _populate_tunnels(self) -> None:
        # Clear list
        child = self.tunnel_list.get_first_child()
        while child:
            next_child = child.get_next_sibling()
            self.tunnel_list.remove(child)
            child = next_child

        tunnels = self.tunnel_manager.get_all_tunnels()

        # If no tunnels in manager, scan session_store to auto-import saved tunnels
        if not tunnels and self.session_store:
            for i in range(self.session_store.get_n_items()):
                item = self.session_store.get_item(i)
                if isinstance(item, SessionItem) and item.port_forwardings:
                    for pf in item.port_forwardings:
                        t = SSHTunnel(
                            name=pf.get("name") or _("Tunnel"),
                            session_name=item.name,
                            session_item=item,
                            type=pf.get("type", "local"),
                            local_host=pf.get("local_host", "127.0.0.1"),
                            local_port=int(pf.get("local_port", 8080)),
                            remote_host=pf.get("remote_host", item.host),
                            remote_port=int(pf.get("remote_port", 80)),
                            auto_start=bool(pf.get("auto_start", True)),
                        )
                        self.tunnel_manager.register_tunnel(t)
            tunnels = self.tunnel_manager.get_all_tunnels()

        has_items = len(tunnels) > 0
        self.status_page.set_visible(not has_items)
        self.tunnel_list.set_visible(has_items)

        for tunnel in tunnels:
            row = self._create_tunnel_row(tunnel)
            self.tunnel_list.append(row)

    def _create_tunnel_row(self, tunnel: SSHTunnel) -> Adw.ActionRow:
        row = Adw.ActionRow()
        row._tunnel_id = tunnel.id
        row._tunnel_data = tunnel

        # Left prefix: status dot
        status_dot = Gtk.Image.new_from_icon_name("media-record-symbolic")
        status_dot.set_pixel_size(12)
        row.add_prefix(status_dot)
        row._status_dot = status_dot

        # Title & Subtitle
        type_badge = f"[{tunnel.type.upper()}]"
        row.set_title(f"<b>{tunnel.name}</b>  <small>{type_badge}</small>")
        row.set_use_markup(True)

        display_flow = f"{tunnel.get_display_source()}  ➔  {tunnel.get_display_target()}"
        if tunnel.session_name:
            display_flow += f"  ({tunnel.session_name})"
        row.set_subtitle(display_flow)

        # Suffix: Quick Copy Button
        copy_btn = Gtk.Button(
            icon_name="edit-copy-symbolic",
            valign=Gtk.Align.CENTER,
            tooltip_text=_("Copy local address (localhost:{port})").format(port=tunnel.local_port),
            css_classes=["flat", "circular"],
        )
        copy_btn.connect("clicked", lambda _, p=tunnel.local_port: self._copy_address(p))
        row.add_suffix(copy_btn)

        # Suffix: Edit Button
        edit_btn = Gtk.Button(
            icon_name="document-edit-symbolic",
            valign=Gtk.Align.CENTER,
            tooltip_text=_("Edit Tunnel"),
            css_classes=["flat", "circular"],
        )
        edit_btn.connect("clicked", lambda _, tid=tunnel.id: self._on_edit_tunnel_clicked(tid))
        row.add_suffix(edit_btn)

        # Suffix: Delete Button
        del_btn = Gtk.Button(
            icon_name="user-trash-symbolic",
            valign=Gtk.Align.CENTER,
            tooltip_text=_("Delete Tunnel"),
            css_classes=["flat", "circular"],
        )
        del_btn.connect("clicked", lambda _, tid=tunnel.id: self._on_delete_tunnel_clicked(tid))
        row.add_suffix(del_btn)

        # Suffix: Toggle Switch
        switch = Gtk.Switch(valign=Gtk.Align.CENTER)
        switch.connect("notify::active", self._on_switch_toggled, tunnel.id)
        row.add_suffix(switch)
        row._switch = switch

        self._apply_status_styling(status_dot, switch, tunnel)
        return row

    def _copy_address(self, port: int) -> None:
        display = Gdk.Display.get_default()
        if display:
            clipboard = display.get_clipboard()
            clipboard.set(f"localhost:{port}")
            self.logger.info(f"Copied localhost:{port} to clipboard")

    def _on_switch_toggled(self, switch: Gtk.Switch, _param: Any, tunnel_id: str) -> None:
        is_active = switch.get_active()
        if is_active:
            self.tunnel_manager.start_tunnel(tunnel_id)
        else:
            self.tunnel_manager.stop_tunnel(tunnel_id)

    def _on_add_tunnel_clicked(self, _btn: Gtk.Button) -> None:
        # Default to first SSH session if available
        first_ssh = None
        if self.session_store:
            for i in range(self.session_store.get_n_items()):
                item = self.session_store.get_item(i)
                if isinstance(item, SessionItem) and item.is_ssh():
                    first_ssh = item
                    break

        def on_save(data: Dict[str, Any]):
            t = SSHTunnel(
                name=data.get("name", "Tunnel"),
                session_name=first_ssh.name if first_ssh else "",
                session_item=first_ssh,
                type=data.get("type", "local"),
                local_host=data.get("local_host", "127.0.0.1"),
                local_port=int(data.get("local_port", 8080)),
                remote_host=data.get("remote_host", first_ssh.host if first_ssh else "localhost"),
                remote_port=int(data.get("remote_port", 80)),
                auto_start=bool(data.get("auto_start", True)),
            )
            self.tunnel_manager.register_tunnel(t)

        dialog = TunnelEditDialog(
            parent_window=self,
            tunnel_data=None,
            on_save=on_save,
            session_host=first_ssh.host if first_ssh else "",
        )
        dialog.present()

    def _on_edit_tunnel_clicked(self, tunnel_id: str) -> None:
        tunnel = self.tunnel_manager.get_tunnel(tunnel_id)
        if not tunnel:
            return

        tunnel_dict = {
            "name": tunnel.name,
            "type": tunnel.type,
            "local_host": tunnel.local_host,
            "local_port": tunnel.local_port,
            "remote_host": tunnel.remote_host,
            "remote_port": tunnel.remote_port,
            "auto_start": tunnel.auto_start,
        }

        def on_save(data: Dict[str, Any]):
            tunnel.name = data.get("name", tunnel.name)
            tunnel.type = data.get("type", tunnel.type)
            tunnel.local_host = data.get("local_host", tunnel.local_host)
            tunnel.local_port = int(data.get("local_port", tunnel.local_port))
            tunnel.remote_host = data.get("remote_host", tunnel.remote_host)
            tunnel.remote_port = int(data.get("remote_port", tunnel.remote_port))
            tunnel.auto_start = bool(data.get("auto_start", tunnel.auto_start))
            if tunnel.status == "active":
                self.tunnel_manager.restart_tunnel(tunnel_id)
            else:
                self._populate_tunnels()

        dialog = TunnelEditDialog(
            parent_window=self,
            tunnel_data=tunnel_dict,
            on_save=on_save,
            session_host=tunnel.remote_host,
        )
        dialog.present()

    def _on_delete_tunnel_clicked(self, tunnel_id: str) -> None:
        self.tunnel_manager.unregister_tunnel(tunnel_id)

    def _filter_tunnels(self) -> None:
        query = self.search_entry.get_text().strip().lower()
        child = self.tunnel_list.get_first_child()
        visible_count = 0

        while child:
            tunnel: SSHTunnel = getattr(child, "_tunnel_data", None)
            if tunnel:
                matched = (
                    not query
                    or query in tunnel.name.lower()
                    or query in tunnel.session_name.lower()
                    or query in str(tunnel.local_port)
                    or query in str(tunnel.remote_port)
                    or query in tunnel.remote_host.lower()
                    or query in tunnel.type.lower()
                )
                child.set_visible(matched)
                if matched:
                    visible_count += 1
            child = child.get_next_sibling()

        self.status_page.set_visible(visible_count == 0)
        self.tunnel_list.set_visible(visible_count > 0)
