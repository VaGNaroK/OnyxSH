# onyxsh/terminal/copy_mode.py
"""
Modo de Cópia por Teclado (Copy-Mode acessível estilo Tmux) para OnyxSH.

Permite que usuários naveguem pelo buffer de texto do terminal usando o teclado,
selecionem trechos com teclas de movimentação e copiem diretamente para a área
de transferência do sistema, sem necessidade de mouse.

Atalhos principais no Copy-Mode:
    - Setas / h, j, k, l: Mover cursor virtual
    - Home / 0 / End / $: Início e fim da linha
    - Page Up / Page Down: Rolar página para cima / baixo
    - v / Espaço: Alternar modo de seleção visual (marcar início)
    - Shift + Setas: Selecionar enquanto move
    - Enter / y / c: Copiar seleção (ou linha atual) para o clipboard e sair
    - Esc / q: Cancelar seleção ou sair do modo
"""

from typing import Optional, Tuple

import gi

try:
    gi.require_version("Gtk", "4.0")
    gi.require_version("Vte", "3.91")
    gi.require_version("Adw", "1")
except (ValueError, AttributeError):
    pass

from gi.repository import Adw, Gdk, GLib, Gtk, Vte

from ..utils.translation_utils import _
from ..utils.logger import get_logger

logger = get_logger("onyxsh.terminal.copy_mode")


class KeyboardCopyModeController:
    """
    Controlador do modo de cópia e navegação por teclado para uma instância de Vte.Terminal.
    """

    def __init__(self, terminal: Vte.Terminal, parent_window=None):
        self.terminal = terminal
        self.parent_window = parent_window

        self.is_active = False
        self.cursor_row = 0
        self.cursor_col = 0
        self.anchor_row: Optional[int] = None
        self.anchor_col: Optional[int] = None
        self.is_selecting = False

        self._popover: Optional[Gtk.Popover] = None
        self._status_label: Optional[Gtk.Label] = None
        self._key_controller: Optional[Gtk.EventControllerKey] = None

        self._setup_key_controller()

        # Limpeza defensiva se o terminal for destruído
        if hasattr(self.terminal, "connect"):
            self.terminal.connect("destroy", self._on_terminal_destroyed)

    def _on_terminal_destroyed(self, _widget):
        self.deactivate()

    def _setup_key_controller(self):
        """Instala o EventControllerKey na fase CAPTURE para interceptar teclas antes do VTE."""
        self._key_controller = Gtk.EventControllerKey()
        self._key_controller.set_propagation_phase(Gtk.PropagationPhase.CAPTURE)
        self._key_controller.connect("key-pressed", self._on_key_pressed)
        if isinstance(self.terminal, Gtk.Widget):
            self.terminal.add_controller(self._key_controller)

    def activate(self) -> None:
        """Ativa o Copy-Mode no terminal."""
        if self.is_active:
            return

        self.is_active = True
        try:
            col, row = self.terminal.get_cursor_position()
            self.cursor_col = int(col)
            self.cursor_row = int(row)
        except Exception:
            self.cursor_col = 0
            self.cursor_row = 0

        self.is_selecting = False
        self.anchor_row = None
        self.anchor_col = None

        # Adicionar classe CSS no terminal
        if hasattr(self.terminal, "add_css_class"):
            self.terminal.add_css_class("terminal-copy-mode-active")

        # Atualizar pane se houver
        pane = self._get_pane()
        if pane and hasattr(pane, "add_css_class"):
            pane.add_css_class("copy-mode-active")
            toggle_btn = getattr(pane, "copy_mode_btn", None)
            if toggle_btn and not toggle_btn.get_active():
                toggle_btn.set_active(True)

        self._show_overlay_ui()
        self._scroll_cursor_into_view()
        self._update_ui()
        logger.debug(f"Keyboard Copy-Mode ativado em ({self.cursor_row}, {self.cursor_col})")

    def deactivate(self) -> None:
        """Desativa o Copy-Mode e restaura o terminal para operação normal."""
        if not self.is_active:
            return

        self.is_active = False
        self.is_selecting = False
        self.anchor_row = None
        self.anchor_col = None

        # Remover classe CSS
        if hasattr(self.terminal, "remove_css_class"):
            self.terminal.remove_css_class("terminal-copy-mode-active")

        pane = self._get_pane()
        if pane and hasattr(pane, "remove_css_class"):
            pane.remove_css_class("copy-mode-active")
            toggle_btn = getattr(pane, "copy_mode_btn", None)
            if toggle_btn and toggle_btn.get_active():
                toggle_btn.set_active(False)

        if self._popover:
            try:
                self._popover.popdown()
            except Exception:
                pass

        logger.debug("Keyboard Copy-Mode desativado.")

    def toggle(self) -> bool:
        """Alterna o estado do Copy-Mode."""
        if self.is_active:
            self.deactivate()
            return False
        else:
            self.activate()
            return True

    def _get_pane(self):
        """Localiza o contêiner de split pane do terminal se existir."""
        pane = getattr(self.terminal, "onyxsh_pane", None)
        if not pane and self.parent_window and hasattr(self.parent_window, "tab_manager"):
            page = self.parent_window.tab_manager.get_page_for_terminal(self.terminal)
            if page:
                pane = self.parent_window.tab_manager._find_pane_for_terminal(
                    page, self.terminal
                )
        return pane

    def _show_overlay_ui(self):
        """Cria e exibe a pílula visual flutuante com status e instruções."""
        if not self._popover:
            self._popover = Gtk.Popover()
            if isinstance(self.terminal, Gtk.Widget):
                try:
                    self._popover.set_parent(self.terminal)
                except Exception:
                    pass
            self._popover.set_autohide(False)
            self._popover.set_has_arrow(False)
            self._popover.set_position(Gtk.PositionType.TOP)
            self._popover.add_css_class("copy-mode-popover")

            box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
            box.add_css_class("copy-mode-pill")

            icon = Gtk.Image.new_from_icon_name("input-keyboard-symbolic")
            icon.set_pixel_size(14)
            box.append(icon)

            self._status_label = Gtk.Label(label="")
            self._status_label.add_css_class("caption")
            box.append(self._status_label)

            copy_btn = Gtk.Button(label=_("Copiar"))
            copy_btn.add_css_class("flat")
            copy_btn.add_css_class("suggested-action")
            copy_btn.connect("clicked", lambda _: self._copy_and_exit())
            box.append(copy_btn)

            close_btn = Gtk.Button()
            close_btn.set_icon_name("window-close-symbolic")
            close_btn.add_css_class("flat")
            close_btn.connect("clicked", lambda _: self.deactivate())
            box.append(close_btn)

            self._popover.set_child(box)

        try:
            width = 640
            if hasattr(self.terminal, "get_width"):
                width = max(200, self.terminal.get_width())
            rect = Gdk.Rectangle()
            rect.x = int(width / 2)
            rect.y = 8
            rect.width = 1
            rect.height = 1
            self._popover.set_pointing_to(rect)
            if isinstance(self.terminal, Gtk.Widget):
                self._popover.popup()
        except Exception:
            pass

    def _update_ui(self):
        """Atualiza o texto de orientação e status."""
        if not self._status_label:
            return

        if self.is_selecting and self.anchor_row is not None and self.anchor_col is not None:
            text = self.get_selected_text()
            char_count = len(text)
            self._status_label.set_text(
                _("Selecionado: {count} chars | Enter: Copiar • Esc: Cancelar").format(
                    count=char_count
                )
            )
        else:
            self._status_label.set_text(
                _(
                    "Linha {row}, Col {col} | Setas: mover • v: selecionar • Enter: copiar linha • Esc: sair"
                ).format(row=self.cursor_row, col=self.cursor_col)
            )

    def _scroll_cursor_into_view(self):
        """Ajusta o scroll vertical para garantir que a linha do cursor virtual esteja visível."""
        try:
            parent = self.terminal.get_parent()
            while parent and not isinstance(parent, Gtk.ScrolledWindow):
                parent = parent.get_parent()

            if parent and isinstance(parent, Gtk.ScrolledWindow):
                adj = parent.get_vadjustment()
                if adj:
                    val = adj.get_value()
                    page_size = adj.get_page_size()
                    if self.cursor_row < val:
                        adj.set_value(float(self.cursor_row))
                    elif self.cursor_row >= val + page_size:
                        adj.set_value(float(self.cursor_row - page_size + 1))
        except Exception as e:
            logger.debug(f"Erro ao rolar terminal para cursor virtual: {e}")

    def get_selection_bounds(self) -> Optional[Tuple[int, int, int, int]]:
        """Retorna (start_row, start_col, end_row, end_col) normalizados ou None."""
        if not self.is_selecting or self.anchor_row is None or self.anchor_col is None:
            return None

        p1 = (self.anchor_row, self.anchor_col)
        p2 = (self.cursor_row, self.cursor_col)

        if p1 <= p2:
            return (p1[0], p1[1], p2[0], p2[1])
        else:
            return (p2[0], p2[1], p1[0], p1[1])

    def get_selected_text(self) -> str:
        """Extrai o texto selecionado (ou a linha corrente se não houver seleção)."""
        bounds = self.get_selection_bounds()
        if bounds:
            start_row, start_col, end_row, end_col = bounds
            try:
                text, _ = self.terminal.get_text_range_format(
                    Vte.Format.TEXT, start_row, start_col, end_row, end_col
                )
                return text or ""
            except Exception as e:
                logger.debug(f"Falha ao extrair texto do intervalo: {e}")
                return ""
        else:
            # Sem seleção: linha atual inteira
            try:
                max_cols = max(80, self.terminal.get_column_count())
                text, _ = self.terminal.get_text_range_format(
                    Vte.Format.TEXT, self.cursor_row, 0, self.cursor_row, max_cols - 1
                )
                return (text or "").rstrip()
            except Exception as e:
                logger.debug(f"Falha ao extrair linha atual: {e}")
                return ""

    def _copy_and_exit(self) -> None:
        """Copia o texto atual para a área de transferência e finaliza o modo."""
        text = self.get_selected_text()
        if text:
            try:
                clipboard = (
                    self.terminal.get_clipboard()
                    if hasattr(self.terminal, "get_clipboard")
                    else self.terminal.get_display().get_clipboard()
                )
                if clipboard:
                    clipboard.set(text)

                # Feedback visual via toast na janela principal
                if self.parent_window and hasattr(self.parent_window, "toast_overlay"):
                    preview = text.strip()[:30]
                    if len(text.strip()) > 30:
                        preview += "..."
                    self.parent_window.toast_overlay.add_toast(
                        Adw.Toast.new(
                            _("Copiado para o clipboard: \"{preview}\"").format(
                                preview=preview
                            )
                        )
                    )
            except Exception as e:
                logger.error(f"Erro ao gravar no clipboard: {e}")

        self.deactivate()

    def _on_key_pressed(
        self, controller, keyval: int, keycode: int, state: Gdk.ModifierType
    ) -> bool:
        """Trata o pressionamento de teclas no modo de navegação/cópia por teclado."""
        if not self.is_active:
            return False

        shift_pressed = bool(state & Gdk.ModifierType.SHIFT_MASK)
        ctrl_pressed = bool(state & Gdk.ModifierType.CONTROL_MASK)

        # 1. Sair ou Cancelar Seleção: Escape ou 'q'
        if keyval in (Gdk.KEY_Escape, Gdk.KEY_q, Gdk.KEY_Q):
            if self.is_selecting:
                self.is_selecting = False
                self.anchor_row = None
                self.anchor_col = None
                self._update_ui()
            else:
                self.deactivate()
            return True

        # 2. Confirmar Cópia: Enter, 'y', 'c' (sem Ctrl)
        if keyval in (Gdk.KEY_Return, Gdk.KEY_KP_Enter, Gdk.KEY_y, Gdk.KEY_Y):
            self._copy_and_exit()
            return True

        if keyval in (Gdk.KEY_c, Gdk.KEY_C) and not ctrl_pressed:
            self._copy_and_exit()
            return True

        # 3. Iniciar / Alternar Seleção: 'v' ou Barra de Espaço
        if keyval in (Gdk.KEY_v, Gdk.KEY_V, Gdk.KEY_space) and not ctrl_pressed:
            if not self.is_selecting:
                self.is_selecting = True
                self.anchor_row = self.cursor_row
                self.anchor_col = self.cursor_col
            else:
                self.is_selecting = False
                self.anchor_row = None
                self.anchor_col = None
            self._update_ui()
            return True

        # Se segurar Shift ao mover setas, ativa automaticamente a seleção se ainda não iniciada
        if shift_pressed and not self.is_selecting:
            self.is_selecting = True
            self.anchor_row = self.cursor_row
            self.anchor_col = self.cursor_col

        max_cols = max(1, self.terminal.get_column_count() if hasattr(self.terminal, "get_column_count") else 80)
        row_pos = 0
        if hasattr(self.terminal, "get_cursor_position"):
            try:
                row_pos = int(self.terminal.get_cursor_position()[1])
            except Exception:
                pass
        max_rows = max(
            self.terminal.get_row_count() if hasattr(self.terminal, "get_row_count") else 24,
            row_pos,
            self.cursor_row,
        )

        # 4. Movimentação do Cursor
        if keyval in (Gdk.KEY_Left, Gdk.KEY_h, Gdk.KEY_H):
            self.cursor_col = max(0, self.cursor_col - 1)
        elif keyval in (Gdk.KEY_Right, Gdk.KEY_l, Gdk.KEY_L):
            self.cursor_col = min(max_cols - 1, self.cursor_col + 1)
        elif keyval in (Gdk.KEY_Up, Gdk.KEY_k, Gdk.KEY_K):
            self.cursor_row = max(0, self.cursor_row - 1)
        elif keyval in (Gdk.KEY_Down, Gdk.KEY_j, Gdk.KEY_J):
            self.cursor_row = min(max_rows, self.cursor_row + 1)
        elif keyval in (Gdk.KEY_Home, Gdk.KEY_0):
            self.cursor_col = 0
        elif keyval in (Gdk.KEY_End, Gdk.KEY_dollar):
            self.cursor_col = max(0, max_cols - 1)
        elif keyval == Gdk.KEY_Page_Up:
            self.cursor_row = max(0, self.cursor_row - 15)
        elif keyval == Gdk.KEY_Page_Down:
            self.cursor_row = min(max_rows, self.cursor_row + 15)
        else:
            # Consome outras teclas enquanto em Copy-Mode para não vazar para o PTY
            return True

        self._scroll_cursor_into_view()
        self._update_ui()
        return True
