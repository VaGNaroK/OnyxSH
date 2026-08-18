# onyxsh/terminal/completion/popup.py
"""
Interactive Floating Completion Popup widget for GTK4 / Libadwaita.
Displays rich subcommands, flags and command descriptions at the terminal cursor.
"""

from typing import Any, Callable, List, Optional

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gdk, GLib, Gtk, Pango

from ...utils.logger import get_logger
from ...utils.translation_utils import _
from .models import CompletionItem, CompletionType


class CompletionPopup(Gtk.Popover):
    """
    A lightweight, floating popover anchored to the terminal cursor position
    displaying structured completion items with keyboard navigation.
    """

    def __init__(
        self,
        parent_widget: Gtk.Widget,
        on_item_accepted: Optional[Callable[[CompletionItem], None]] = None,
    ) -> None:
        super().__init__()
        self.logger = get_logger("onyxsh.terminal.completion.popup")
        self.set_parent(parent_widget)
        self.on_item_accepted = on_item_accepted

        self.add_css_class("onyxsh-completion-popover")
        self.set_autohide(False)
        self.set_focusable(False)
        self.set_can_focus(False)
        self.set_has_arrow(False)
        self.set_position(Gtk.PositionType.BOTTOM)

        self._items: List[CompletionItem] = []
        self._selected_index: int = 0
        self._row_widgets: List[Gtk.ListBoxRow] = []

        # Main layout
        main_box = Gtk.Box(
            orientation=Gtk.Orientation.VERTICAL,
            spacing=2,
            margin_start=4,
            margin_end=4,
            margin_top=4,
            margin_bottom=4,
        )
        main_box.set_focusable(False)
        main_box.set_can_focus(False)
        main_box.set_size_request(320, -1)

        # Scrolled list
        self.list_box = Gtk.ListBox()
        self.list_box.add_css_class("boxed-list")
        self.list_box.add_css_class("rich-list")
        self.list_box.set_focusable(False)
        self.list_box.set_can_focus(False)
        self.list_box.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self.list_box.connect("row-activated", self._on_row_activated)

        main_box.append(self.list_box)

        # Bottom Hint Bar
        hint_box = Gtk.Box(
            orientation=Gtk.Orientation.HORIZONTAL,
            spacing=8,
            margin_start=6,
            margin_end=6,
            margin_top=4,
            margin_bottom=2,
        )
        hint_box.set_focusable(False)
        hint_box.set_can_focus(False)
        hint_label = Gtk.Label(
            label=_("Tab or → to apply • Esc to dismiss"),
            xalign=0.0,
            hexpand=True,
            css_classes=["dim-label", "caption"],
        )
        hint_box.append(hint_label)
        main_box.append(hint_box)

        self.set_child(main_box)

    def show_completions(
        self,
        items: List[CompletionItem],
        pointing_rect: Optional[Gdk.Rectangle] = None,
    ) -> None:
        """
        Populates the popup with completion items and presents it at the cursor rectangle.
        """
        self._items = items
        self._row_widgets.clear()

        # Clear existing rows
        while True:
            row = self.list_box.get_row_at_index(0)
            if row is None:
                break
            self.list_box.remove(row)

        if not items:
            self.popdown()
            return

        for idx, item in enumerate(items):
            row = Gtk.ListBoxRow()
            row.set_focusable(False)
            row.set_can_focus(False)
            row.add_css_class("completion-row")

            item_box = Gtk.Box(
                orientation=Gtk.Orientation.HORIZONTAL,
                spacing=10,
                margin_start=8,
                margin_end=8,
                margin_top=6,
                margin_bottom=6,
            )

            # Icon
            icon = Gtk.Image.new_from_icon_name(item.get_icon())
            icon.set_pixel_size(16)
            icon.add_css_class("dim-label")
            item_box.append(icon)

            # Text column
            content_box = Gtk.Box(
                orientation=Gtk.Orientation.VERTICAL,
                spacing=2,
                hexpand=True,
            )

            title_label = Gtk.Label(
                label=item.get_display_text(),
                xalign=0.0,
                ellipsize=Pango.EllipsizeMode.END,
                css_classes=["heading-4"],
            )
            content_box.append(title_label)

            desc_text = item.get_description()
            if desc_text:
                desc_label = Gtk.Label(
                    label=desc_text,
                    xalign=0.0,
                    ellipsize=Pango.EllipsizeMode.END,
                    css_classes=["dim-label", "caption"],
                )
                content_box.append(desc_label)

            item_box.append(content_box)

            # Type badge
            type_label = Gtk.Label(
                label=item.completion_type.value.upper(),
                css_classes=["badge", "caption", "dim-label"],
            )
            item_box.append(type_label)

            row.set_child(item_box)
            self.list_box.append(row)
            self._row_widgets.append(row)

        self._selected_index = 0
        if self._row_widgets:
            self.list_box.select_row(self._row_widgets[0])

        if pointing_rect:
            self.set_pointing_to(pointing_rect)

        self.popup()
        parent = self.get_parent()
        if parent and hasattr(parent, "grab_focus"):
            parent.grab_focus()

    def handle_key_event(self, keyval: int, state: Gdk.ModifierType) -> bool:
        """
        Handles keyboard events while popup is active.
        Returns True ONLY if a navigation/acceptance key was consumed.
        """
        if not self.get_visible() or not self._items:
            return False

        # Up and Down navigate the suggestion list
        if keyval in (Gdk.KEY_Down, Gdk.KEY_KP_Down):
            if not (state & (Gdk.ModifierType.CONTROL_MASK | Gdk.ModifierType.ALT_MASK)):
                self._select_next()
                return True
        elif keyval in (Gdk.KEY_Up, Gdk.KEY_KP_Up):
            if not (state & (Gdk.ModifierType.CONTROL_MASK | Gdk.ModifierType.ALT_MASK)):
                self._select_previous()
                return True
        # Tab or Right Arrow accepts the current suggestion
        elif keyval in (Gdk.KEY_Tab, Gdk.KEY_Right, Gdk.KEY_KP_Right):
            if not (state & (Gdk.ModifierType.CONTROL_MASK | Gdk.ModifierType.ALT_MASK)):
                self._accept_selected()
                return True
        elif keyval == Gdk.KEY_Escape:
            self.popdown()
            return True

        return False

    def _select_next(self) -> None:
        """Selects the next item in the list."""
        if not self._items:
            return
        self._selected_index = (self._selected_index + 1) % len(self._items)
        if self._selected_index < len(self._row_widgets):
            self.list_box.select_row(self._row_widgets[self._selected_index])

    def _select_previous(self) -> None:
        """Selects the previous item in the list."""
        if not self._items:
            return
        self._selected_index = (self._selected_index - 1) % len(self._items)
        if self._selected_index < len(self._row_widgets):
            self.list_box.select_row(self._row_widgets[self._selected_index])

    def _accept_selected(self) -> None:
        """Applies the currently highlighted item."""
        if 0 <= self._selected_index < len(self._items):
            item = self._items[self._selected_index]
            self.popdown()
            if self.on_item_accepted:
                self.on_item_accepted(item)

    def _on_row_activated(self, list_box: Gtk.ListBox, row: Gtk.ListBoxRow) -> None:
        """Handles mouse click or activation on a row."""
        idx = row.get_index()
        if 0 <= idx < len(self._items):
            self._selected_index = idx
            self._accept_selected()
