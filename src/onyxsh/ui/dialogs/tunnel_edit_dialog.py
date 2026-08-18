# onyxsh/ui/dialogs/tunnel_edit_dialog.py
"""
Dialog for creating and editing SSH Port Forwarding Tunnels.
Supports Local (-L), Remote (-R), and Dynamic SOCKS5 (-D) configurations.
"""

from typing import Any, Callable, Dict, Optional

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gtk

from ...utils.logger import get_logger
from ...utils.translation_utils import _


class TunnelEditDialog(Adw.Window):
    """
    Modal dialog for configuring an individual SSH port forwarding tunnel.
    """

    def __init__(
        self,
        parent_window: Optional[Gtk.Window],
        tunnel_data: Optional[Dict[str, Any]] = None,
        on_save: Optional[Callable[[Dict[str, Any]], None]] = None,
        session_host: str = "",
    ) -> None:
        super().__init__()
        self.logger = get_logger("onyxsh.ui.dialogs.tunnel_edit")
        self.tunnel_data = dict(tunnel_data) if tunnel_data else {}
        self.on_save = on_save
        self.session_host = session_host
        self.is_new = not bool(tunnel_data)

        self.set_title(_("Add SSH Tunnel") if self.is_new else _("Edit SSH Tunnel"))
        self.set_modal(True)
        self.set_transient_for(parent_window)
        self.set_default_size(520, 560)
        self.set_resizable(False)

        self._setup_ui()

    def _setup_ui(self) -> None:
        toolbar_view = Adw.ToolbarView()
        self.set_content(toolbar_view)

        header = Adw.HeaderBar()
        toolbar_view.add_top_bar(header)

        # Cancel button
        cancel_btn = Gtk.Button(label=_("Cancel"))
        cancel_btn.connect("clicked", lambda _: self.close())
        header.pack_start(cancel_btn)

        # Save button
        self.save_btn = Gtk.Button(label=_("Save"), css_classes=["suggested-action"])
        self.save_btn.connect("clicked", self._on_save_clicked)
        header.pack_end(self.save_btn)

        # Content
        scrolled = Gtk.ScrolledWindow()
        scrolled.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        toolbar_view.set_content(scrolled)

        page = Adw.PreferencesPage()
        scrolled.set_child(page)

        # Group 1: Tunnel Type
        type_group = Adw.PreferencesGroup(title=_("Tunnel Type"))
        self.type_combo = Adw.ComboRow(
            title=_("Forwarding Mode"),
            subtitle=_("Choose between Local, Remote or Dynamic SOCKS5"),
        )
        self.type_combo.set_model(
            Gtk.StringList.new([
                _("Local (-L) • Connect local port to remote service"),
                _("Remote (-R) • Expose local port on remote server"),
                _("Dynamic (-D) • SOCKS5 Proxy through SSH"),
            ])
        )

        current_type = self.tunnel_data.get("type", "local").lower()
        if current_type == "remote":
            self.type_combo.set_selected(1)
        elif current_type == "dynamic":
            self.type_combo.set_selected(2)
        else:
            self.type_combo.set_selected(0)

        self.type_combo.connect("notify::selected", self._on_type_changed)
        type_group.add(self.type_combo)
        page.add(type_group)

        # Group 2: Endpoints & Ports
        self.config_group = Adw.PreferencesGroup(title=_("Endpoints & Ports"))

        self.name_row = Adw.EntryRow(
            title=_("Tunnel Name"),
            text=self.tunnel_data.get("name", ""),
        )
        self.config_group.add(self.name_row)

        self.local_host_row = Adw.EntryRow(
            title=_("Local Interface / Bind IP"),
            text=self.tunnel_data.get("local_host", "127.0.0.1") or "127.0.0.1",
        )
        self.config_group.add(self.local_host_row)

        self.local_port_row = Adw.SpinRow.new_with_range(1024, 65535, 1)
        self.local_port_row.set_title(_("Local Port"))
        self.local_port_row.set_value(float(self.tunnel_data.get("local_port", 8080) or 8080))
        self.config_group.add(self.local_port_row)

        self.remote_host_row = Adw.EntryRow(
            title=_("Destination Host"),
            text=self.tunnel_data.get("remote_host", self.session_host or "localhost"),
        )
        self.config_group.add(self.remote_host_row)

        self.remote_port_row = Adw.SpinRow.new_with_range(1, 65535, 1)
        self.remote_port_row.set_title(_("Destination Port"))
        self.remote_port_row.set_value(float(self.tunnel_data.get("remote_port", 80) or 80))
        self.config_group.add(self.remote_port_row)

        page.add(self.config_group)

        # Group 3: Options
        opt_group = Adw.PreferencesGroup(title=_("Options"))
        self.auto_start_row = Adw.SwitchRow(
            title=_("Auto-start with Session"),
            subtitle=_("Automatically open this tunnel when connecting to the SSH host"),
            active=self.tunnel_data.get("auto_start", True),
        )
        opt_group.add(self.auto_start_row)
        page.add(opt_group)

        self._update_visibility()

    def _on_type_changed(self, _combo: Adw.ComboRow, _param: Any) -> None:
        self._update_visibility()

    def _update_visibility(self) -> None:
        idx = self.type_combo.get_selected()
        is_dynamic = idx == 2
        is_remote = idx == 1

        if is_dynamic:
            self.remote_host_row.set_visible(False)
            self.remote_port_row.set_visible(False)
            self.local_port_row.set_title(_("SOCKS5 Proxy Port"))
            self.local_host_row.set_title(_("SOCKS5 Bind Interface"))
        elif is_remote:
            self.remote_host_row.set_visible(True)
            self.remote_port_row.set_visible(True)
            self.local_port_row.set_title(_("Local Source Port"))
            self.local_host_row.set_title(_("Local Source IP"))
            self.remote_host_row.set_title(_("Remote Bind Host"))
            self.remote_port_row.set_title(_("Remote Exposed Port"))
        else:  # local
            self.remote_host_row.set_visible(True)
            self.remote_port_row.set_visible(True)
            self.local_port_row.set_title(_("Local Listen Port"))
            self.local_host_row.set_title(_("Local Listen IP"))
            self.remote_host_row.set_title(_("Remote Destination Host"))
            self.remote_port_row.set_title(_("Remote Destination Port"))

    def _on_save_clicked(self, _btn: Gtk.Button) -> None:
        idx = self.type_combo.get_selected()
        t_type = "dynamic" if idx == 2 else ("remote" if idx == 1 else "local")

        name = self.name_row.get_text().strip() or f"{t_type.capitalize()} Tunnel"
        local_host = self.local_host_row.get_text().strip() or "127.0.0.1"
        local_port = int(self.local_port_row.get_value())
        remote_host = self.remote_host_row.get_text().strip() if t_type != "dynamic" else ""
        remote_port = int(self.remote_port_row.get_value()) if t_type != "dynamic" else 0
        auto_start = self.auto_start_row.get_active()

        result = {
            "type": t_type,
            "name": name,
            "local_host": local_host,
            "local_port": local_port,
            "remote_host": remote_host,
            "remote_port": remote_port,
            "auto_start": auto_start,
        }

        self.close()
        if self.on_save:
            self.on_save(result)
