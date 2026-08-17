from pathlib import Path

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gio, Gtk

from ...utils.icons import icon_button
from ...utils.translation_utils import _
from .base_dialog import BaseDialog


class TftpServerDialog(BaseDialog):
    """Dialog for configuring and starting the embedded TFTP server."""

    def __init__(self, parent_window, settings_manager, on_start):
        super().__init__(
            parent_window,
            _("Start TFTP Server"),
            auto_setup_toolbar=True,
            default_width=560,
            default_height=360,
        )
        self.settings_manager = settings_manager
        self._on_start = on_start

        start_button = Gtk.Button(label=_("Start"))
        start_button.add_css_class("suggested-action")
        start_button.connect("clicked", self._on_start_clicked)
        self.add_header_button(start_button)

        self.port_row = Adw.SpinRow.new_with_range(0, 65535, 1)
        self.port_row.set_title(_("TFTP Port"))
        self.port_row.set_value(self.settings_manager.get("tftp_server_port", 6969))

        upload_dir = self.settings_manager.get(
            "tftp_server_upload_dir", str(Path.home())
        )
        download_dir = self.settings_manager.get(
            "tftp_server_download_dir", str(Path.home())
        )
        self.upload_row = self._create_directory_row(
            _("Upload Directory"),
            _("Files requested by TFTP clients are read from this directory"),
            upload_dir,
        )
        self.download_row = self._create_directory_row(
            _("Download Directory"),
            _("Files sent by TFTP clients are written to this directory"),
            download_dir,
        )

        group = Adw.PreferencesGroup(
            title=_("TFTP Server"),
            description=_(
                "The TFTP server uses local directories for uploading and downloading files."
            ),
        )
        group.add(self.port_row)
        group.add(self.upload_row)
        group.add(self.download_row)

        page = Adw.PreferencesPage()
        page.add(group)
        self.set_body_content(page)

    def _create_directory_row(
        self, title: str, subtitle: str, path: str
    ) -> Adw.ActionRow:
        row = Adw.ActionRow(title=title, subtitle=subtitle)
        entry = Gtk.Entry(text=path, hexpand=True)
        entry.set_valign(Gtk.Align.CENTER)
        browse_button = icon_button("folder-open-symbolic")
        browse_button.set_tooltip_text(_("Select Directory"))
        browse_button.connect("clicked", self._on_browse_clicked, entry)
        row.add_suffix(entry)
        row.add_suffix(browse_button)
        row.set_activatable_widget(entry)
        row.entry = entry
        return row

    def _on_browse_clicked(self, _button, entry: Gtk.Entry) -> None:
        chooser = Gtk.FileChooserDialog(
            title=_("Open Directory"),
            transient_for=self,
            modal=True,
            action=Gtk.FileChooserAction.SELECT_FOLDER,
        )
        chooser.add_css_class("onyxsh-dialog")
        chooser.add_button(_("Cancel"), Gtk.ResponseType.CANCEL)
        chooser.add_button(_("Select"), Gtk.ResponseType.ACCEPT)
        try:
            current = entry.get_text().strip()
            if current:
                chooser.set_current_folder(Gio.File.new_for_path(current))
        except Exception:
            pass

        def on_response(dialog, response_id):
            try:
                if response_id == Gtk.ResponseType.ACCEPT:
                    folder = dialog.get_file()
                    if folder and folder.get_path():
                        entry.set_text(folder.get_path())
            finally:
                dialog.destroy()

        chooser.connect("response", on_response)
        chooser.present()

    def _on_start_clicked(self, _button) -> None:
        upload_dir = self.upload_row.entry.get_text().strip()
        download_dir = self.download_row.entry.get_text().strip()
        upload_path = Path(upload_dir).expanduser()
        download_path = Path(download_dir).expanduser()

        if not upload_path.is_dir() or not download_path.is_dir():
            self._show_error_dialog(
                _("Warning"),
                _("Please select a valid directory!"),
            )
            return

        port = int(self.port_row.get_value())
        self.settings_manager.set("tftp_server_port", port)
        self.settings_manager.set("tftp_server_upload_dir", str(upload_path))
        self.settings_manager.set("tftp_server_download_dir", str(download_path))
        self._on_start(port, str(upload_path), str(download_path))
        self.close()
