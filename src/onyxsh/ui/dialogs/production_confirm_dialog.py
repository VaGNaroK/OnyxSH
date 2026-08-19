# onyxsh/ui/dialogs/production_confirm_dialog.py
"""
Production Guard Confirmation Dialog for Libadwaita / GTK4.
Requires explicit hostname confirmation before permitting high-risk
destructive operations in production terminals.
"""

from typing import Callable, Optional

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, GLib, Gtk

from ...terminal.production_guard import GuardViolation
from ...utils.logger import get_logger
from ...utils.translation_utils import _


class ProductionConfirmDialog(Adw.Window):
    """
    Modal confirmation dialog requiring strict hostname matching
    before executing dangerous commands in production.
    """

    def __init__(
        self,
        parent_window: Optional[Gtk.Window],
        violation: GuardViolation,
        target_name: str,
        on_confirmed: Callable[[bool], None],
    ) -> None:
        super().__init__()
        self.logger = get_logger("onyxsh.ui.dialogs.production_confirm")
        self.violation = violation
        self.target_name = target_name.strip() or "production"
        self.on_confirmed = on_confirmed
        self._decision_made = False

        self.set_title(_("Production Guard Safety Confirmation"))
        self.set_modal(True)
        self.set_transient_for(parent_window)
        self.set_default_size(520, 440)
        self.set_resizable(False)

        self._setup_ui()
        self.connect("close-request", self._on_close_request)

    def _setup_ui(self) -> None:
        toolbar_view = Adw.ToolbarView()
        self.set_content(toolbar_view)

        header = Adw.HeaderBar()
        header.set_show_end_title_buttons(False)
        header.set_show_start_title_buttons(False)
        toolbar_view.add_top_bar(header)

        main_box = Gtk.Box(
            orientation=Gtk.Orientation.VERTICAL,
            spacing=16,
            margin_start=24,
            margin_end=24,
            margin_top=12,
            margin_bottom=20,
        )
        toolbar_view.set_content(main_box)

        # 1. Warning Icon & Title
        title_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        warn_icon = Gtk.Image.new_from_icon_name("dialog-warning-symbolic")
        warn_icon.set_pixel_size(32)
        warn_icon.add_css_class("error")
        title_box.append(warn_icon)

        title_vbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        heading = Gtk.Label(
            label=_("High-Risk Command in Production"),
            xalign=0.0,
            css_classes=["heading-2", "error"],
        )
        sub_heading = Gtk.Label(
            label=_("This terminal is connected to an active production environment."),
            xalign=0.0,
            css_classes=["dim-label", "caption"],
        )
        title_vbox.append(heading)
        title_vbox.append(sub_heading)
        title_box.append(title_vbox)
        main_box.append(title_box)

        # 2. Risk Info Card
        card = Gtk.Box(
            orientation=Gtk.Orientation.VERTICAL,
            spacing=8,
            css_classes=["card"],
            margin_top=4,
            margin_bottom=4,
        )

        risk_header = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        risk_tag = Gtk.Label(
            label=f"⚠️ {self.violation.category}: {self.violation.risk_summary}",
            xalign=0.0,
            css_classes=["heading-4", "warning"],
            margin_start=12,
            margin_top=10,
            margin_end=12,
        )
        card.append(risk_tag)

        cmd_scroll = Gtk.ScrolledWindow()
        cmd_scroll.set_policy(Gtk.PolicyType.AUTOMATIC, Gtk.PolicyType.NEVER)
        cmd_scroll.set_margin_start(12)
        cmd_scroll.set_margin_end(12)
        cmd_scroll.set_margin_bottom(12)

        cmd_label = Gtk.Label(
            label=self.violation.command,
            xalign=0.0,
            selectable=True,
            css_classes=["monospace", "caption"],
        )
        cmd_scroll.set_child(cmd_label)
        card.append(cmd_scroll)
        main_box.append(card)

        # 3. Verification Challenge Input
        challenge_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        instruction_label = Gtk.Label(
            label=_("To confirm and execute, type the target name: <b>{target}</b>").format(
                target=self.target_name
            ),
            use_markup=True,
            xalign=0.0,
            css_classes=["body"],
        )
        challenge_box.append(instruction_label)

        self.confirm_entry = Gtk.Entry(
            placeholder_text=_("Type '{target}' to confirm...").format(
                target=self.target_name
            ),
            hexpand=True,
        )
        self.confirm_entry.connect("changed", self._on_entry_changed)
        self.confirm_entry.connect("activate", self._on_entry_activated)
        challenge_box.append(self.confirm_entry)
        main_box.append(challenge_box)

        # 4. Action Buttons
        btn_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        btn_box.set_halign(Gtk.Align.END)
        btn_box.set_margin_top(8)

        self.cancel_btn = Gtk.Button(
            label=_("Cancel & Abort (Esc)"),
            css_classes=["flat"],
        )
        self.cancel_btn.connect("clicked", lambda _: self._finish(False))
        btn_box.append(self.cancel_btn)

        self.exec_btn = Gtk.Button(
            label=_("Execute in Production"),
            css_classes=["destructive-action"],
            sensitive=False,
        )
        self.exec_btn.connect("clicked", lambda _: self._finish(True))
        btn_box.append(self.exec_btn)

        main_box.append(btn_box)

        # Escape key shortcut controller (does not intercept text typing)
        esc_shortcut = Gtk.Shortcut.new(
            Gtk.ShortcutTrigger.parse_string("Escape"),
            Gtk.CallbackAction.new(lambda *_: self._finish(False) or True),
        )
        shortcut_ctrl = Gtk.ShortcutController.new()
        shortcut_ctrl.set_scope(Gtk.ShortcutScope.LOCAL)
        shortcut_ctrl.add_shortcut(esc_shortcut)
        self.add_controller(shortcut_ctrl)

        # Focus entry once on dialog open
        def _focus_entry() -> bool:
            self.confirm_entry.grab_focus()
            return GLib.SOURCE_REMOVE

        GLib.idle_add(_focus_entry)

    def _on_entry_changed(self, entry: Gtk.Entry) -> None:
        typed = entry.get_text().strip()
        matched = typed == self.target_name
        self.exec_btn.set_sensitive(matched)

    def _on_entry_activated(self, _entry: Gtk.Entry) -> None:
        if self.exec_btn.get_sensitive():
            self._finish(True)

    def _finish(self, confirmed: bool) -> None:
        if self._decision_made:
            return
        self._decision_made = True
        self.close()
        try:
            self.on_confirmed(confirmed)
        except Exception as e:
            self.logger.error(f"Error in on_confirmed callback: {e}")

    def _on_close_request(self, _window: Gtk.Window) -> bool:
        if not self._decision_made:
            self._decision_made = True
            try:
                self.on_confirmed(False)
            except Exception as e:
                self.logger.error(f"Error in close on_confirmed callback: {e}")
        return False
