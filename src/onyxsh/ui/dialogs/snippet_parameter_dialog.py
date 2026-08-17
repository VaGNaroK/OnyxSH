# onyxsh/ui/dialogs/snippet_parameter_dialog.py
"""
Interactive Parameter Substitution Dialog for Command Snippets.
Displays customizable parameters, pre-fills defaults, resolves system variables,
and provides a live syntax-highlighted preview of the resulting command.
"""

from typing import Any, Callable, Dict, List, Optional

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, GLib, GObject, Gtk

from ...data.snippet_resolver import SnippetVariable, get_snippet_resolver
from ...utils.logger import get_logger
from ...utils.syntax_utils import get_bash_pango_markup
from ...utils.translation_utils import _


class SnippetParameterDialog(Adw.Window):
    """
    Modal dialog for configuring and interpolating parameterized command snippets.
    """

    __gsignals__ = {
        "snippet-ready": (GObject.SignalFlags.RUN_FIRST, None, (str, bool)),
    }

    def __init__(
        self,
        parent: Gtk.Window,
        snippet_name: str,
        template: str,
        description: str = "",
        current_terminal: Optional[Any] = None,
        on_ready_callback: Optional[Callable[[str, bool], None]] = None,
    ):
        super().__init__(
            transient_for=parent,
            modal=True,
            default_width=560,
            default_height=420,
        )
        self.logger = get_logger(
            "onyxsh.ui.dialogs.snippet_parameter_dialog"
        )
        self.snippet_name = snippet_name
        self.template = template
        self.description = description
        self.current_terminal = current_terminal
        self.on_ready_callback = on_ready_callback

        self.resolver = get_snippet_resolver()
        self.custom_vars = self.resolver.get_custom_variables(template)
        self._entry_widgets: Dict[str, Gtk.Entry] = {}

        self.add_css_class("onyxsh-dialog")
        self.set_title(snippet_name or _("Command Snippet"))

        self._setup_ui()
        self._setup_key_controller()
        self._update_preview()

    def _setup_ui(self) -> None:
        toolbar_view = Adw.ToolbarView()
        self.set_content(toolbar_view)

        # Header Bar
        header = Adw.HeaderBar()
        header.set_show_end_title_buttons(True)
        toolbar_view.add_top_bar(header)

        # Main Layout Container
        main_box = Gtk.Box(
            orientation=Gtk.Orientation.VERTICAL, spacing=16
        )
        main_box.set_margin_top(16)
        main_box.set_margin_bottom(16)
        main_box.set_margin_start(20)
        main_box.set_margin_end(20)

        # Header Info Card
        info_card = Gtk.Box(
            orientation=Gtk.Orientation.VERTICAL, spacing=6
        )
        title_label = Gtk.Label(
            label=self.snippet_name,
            xalign=0.0,
            css_classes=["title-3", "heading"],
        )
        info_card.append(title_label)

        if self.description:
            desc_label = Gtk.Label(
                label=self.description,
                xalign=0.0,
                wrap=True,
                css_classes=["dim-label", "caption"],
            )
            info_card.append(desc_label)

        main_box.append(info_card)

        # Parameters Form Group (Scrollable if many variables)
        form_scroll = Gtk.ScrolledWindow()
        form_scroll.set_policy(
            Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC
        )
        form_scroll.set_min_content_height(140)
        form_scroll.set_vexpand(True)

        pref_group = Adw.PreferencesGroup()
        pref_group.set_title(_("Parameters"))
        pref_group.set_description(
            _("Fill in the values for the template variables below:")
        )

        for var in self.custom_vars:
            row = Adw.EntryRow()
            row.set_title(var.display_label)
            row.set_text(var.default_value)
            if var.default_value:
                row.set_placeholder_text(
                    _("Default: {}").format(var.default_value)
                )
            else:
                row.set_placeholder_text(_("Enter {}").format(var.name))

            row.connect("changed", self._on_field_changed)
            row.connect("entry-activated", self._on_execute_clicked)
            self._entry_widgets[var.name] = row
            pref_group.add(row)

        form_scroll.set_child(pref_group)
        main_box.append(form_scroll)

        # Live Command Preview Box
        preview_group = Gtk.Box(
            orientation=Gtk.Orientation.VERTICAL, spacing=6
        )
        preview_header = Gtk.Box(
            orientation=Gtk.Orientation.HORIZONTAL, spacing=8
        )
        preview_title = Gtk.Label(
            label=_("Live Preview:"),
            xalign=0.0,
            css_classes=["heading", "dim-label"],
            hexpand=True,
        )
        copy_btn = Gtk.Button(
            icon_name="edit-copy-symbolic",
            tooltip_text=_("Copy to clipboard"),
            css_classes=["flat", "circular"],
        )
        copy_btn.connect("clicked", self._on_copy_clicked)
        preview_header.append(preview_title)
        preview_header.append(copy_btn)
        preview_group.append(preview_header)

        # Preview Frame / Label
        preview_frame = Gtk.Frame()
        preview_frame.add_css_class("view")
        preview_frame.add_css_class("card")

        self.preview_label = Gtk.Label(
            xalign=0.0,
            wrap=True,
            selectable=True,
            css_classes=["monospace"],
        )
        self.preview_label.set_margin_top(10)
        self.preview_label.set_margin_bottom(10)
        self.preview_label.set_margin_start(12)
        self.preview_label.set_margin_end(12)
        preview_frame.set_child(self.preview_label)
        preview_group.append(preview_frame)

        main_box.append(preview_group)

        # Bottom Action Bar
        action_bar = Gtk.Box(
            orientation=Gtk.Orientation.HORIZONTAL, spacing=10
        )
        action_bar.set_halign(Gtk.Align.END)
        action_bar.set_margin_top(6)

        cancel_btn = Gtk.Button(label=_("Cancel"))
        cancel_btn.connect("clicked", lambda _: self.close())
        action_bar.append(cancel_btn)

        insert_btn = Gtk.Button(
            label=_("Insert into Prompt"),
            tooltip_text=_("Insert command without executing (Shift+Enter)"),
            css_classes=["suggested-action"],
        )
        insert_btn.connect("clicked", self._on_insert_clicked)
        action_bar.append(insert_btn)

        execute_btn = Gtk.Button(
            label=_("Execute"),
            tooltip_text=_("Run command immediately in terminal (Enter)"),
            css_classes=["accent"],
        )
        execute_btn.connect("clicked", self._on_execute_clicked)
        action_bar.append(execute_btn)

        main_box.append(action_bar)
        toolbar_view.set_content(main_box)

        # Auto-focus first input field
        if self.custom_vars and self.custom_vars[0].name in self._entry_widgets:
            GLib.idle_add(
                self._entry_widgets[self.custom_vars[0].name].grab_focus
            )

    def _setup_key_controller(self) -> None:
        key_controller = Gtk.EventControllerKey.new()
        key_controller.connect("key-pressed", self._on_key_pressed)
        self.add_controller(key_controller)

    def _on_key_pressed(
        self, controller, keyval, keycode, state
    ) -> bool:
        if keyval == Gdk.KEY_Escape:
            self.close()
            return True

        if keyval in (Gdk.KEY_Return, Gdk.KEY_KP_Enter):
            if state & Gdk.ModifierType.SHIFT_MASK:
                self._on_insert_clicked(None)
            else:
                self._on_execute_clicked(None)
            return True

        return False

    def _collect_values(self) -> Dict[str, str]:
        values: Dict[str, str] = {}
        for var_name, entry in self._entry_widgets.items():
            text = entry.get_text().strip()
            values[var_name] = text
        return values

    def _get_resolved_command(self) -> str:
        values = self._collect_values()
        return self.resolver.resolve_template(
            template=self.template,
            user_values=values,
            terminal=self.current_terminal,
        )

    def _update_preview(self) -> None:
        cmd = self._get_resolved_command()
        markup = get_bash_pango_markup(cmd)
        self.preview_label.set_markup(markup)

    def _on_field_changed(self, entry) -> None:
        self._update_preview()

    def _on_copy_clicked(self, button) -> None:
        cmd = self._get_resolved_command()
        clipboard = self.get_clipboard()
        if clipboard:
            clipboard.set(cmd)
            button.set_icon_name("object-select-symbolic")
            GLib.timeout_add(
                1200,
                lambda: button.set_icon_name("edit-copy-symbolic"),
            )

    def _on_insert_clicked(self, _btn) -> None:
        cmd = self._get_resolved_command()
        self.emit("snippet-ready", cmd, False)
        if self.on_ready_callback:
            self.on_ready_callback(cmd, False)
        self.close()

    def _on_execute_clicked(self, _btn) -> None:
        cmd = self._get_resolved_command()
        self.emit("snippet-ready", cmd, True)
        if self.on_ready_callback:
            self.on_ready_callback(cmd, True)
        self.close()
