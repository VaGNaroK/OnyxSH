"""Command Palette Dialog - Fast spotlight-style keyboard-driven action runner."""

from __future__ import annotations

import unicodedata
from typing import TYPE_CHECKING, Any, Callable, Dict, List, Optional

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, GLib, Gtk, Pango

from ...helpers import accelerator_to_label
from ...sessions.storage import SessionStorageManager
from ...utils.icons import icon_image
from ...utils.logger import get_logger
from ...utils.translation_utils import _
from .base_dialog import BaseDialog

if TYPE_CHECKING:
    from ...window import CommTerminalWindow


def _normalize(text: str) -> str:
    """Normalize string for accent-insensitive search."""
    if not text:
        return ""
    nfkd = unicodedata.normalize("NFKD", text)
    return "".join(c for c in nfkd if not unicodedata.combining(c)).lower()


class CommandPaletteItem:
    """Represents a single executable action inside the Command Palette."""

    def __init__(
        self,
        item_id: str,
        title: str,
        category: str,
        icon_name: str,
        action_name: Optional[str] = None,
        callback: Optional[Callable[[], None]] = None,
        shortcut: Optional[str] = None,
        keywords: Optional[List[str]] = None,
    ) -> None:
        self.item_id = item_id
        self.title = title
        self.category = category
        self.icon_name = icon_name
        self.action_name = action_name
        self.callback = callback
        self.shortcut = shortcut
        self.keywords = keywords or []
        self._search_index = _normalize(
            f"{title} {category} {' '.join(self.keywords)} {item_id}"
        )

    def matches(self, query: str) -> bool:
        """Check if item matches search query."""
        if not query:
            return True
        norm_query = _normalize(query).strip()
        tokens = norm_query.split()
        return all(token in self._search_index for token in tokens)

    def score(self, query: str) -> int:
        """Calculate match relevance score for sorting."""
        if not query:
            return 0
        norm_query = _normalize(query).strip()
        norm_title = _normalize(self.title)
        norm_category = _normalize(self.category)

        # Exact title match
        if norm_title == norm_query:
            return 100
        # Title starts with query
        if norm_title.startswith(norm_query):
            return 80
        # Word in title starts with query
        if any(w.startswith(norm_query) for w in norm_title.split()):
            return 60
        # Query in title
        if norm_query in norm_title:
            return 40
        # Query in category
        if norm_query in norm_category:
            return 30
        # Query in keywords / index
        if norm_query in self._search_index:
            return 20
        return 0


class CommandPaletteDialog(BaseDialog):
    """Spotlight-style command palette for quick action search and execution."""

    def __init__(self, parent_window: CommTerminalWindow) -> None:
        super().__init__(
            parent_window=parent_window,
            dialog_title=_("Paleta de Comandos"),
            auto_setup_toolbar=False,
            default_width=580,
            default_height=440,
        )
        self.logger = get_logger("zashterminal.ui.dialogs.command_palette")
        self.add_css_class("command-palette-dialog")

        self._items: List[CommandPaletteItem] = []
        self._filtered_items: List[CommandPaletteItem] = []
        self._list_box: Gtk.ListBox = Gtk.ListBox()
        self._search_entry: Gtk.SearchEntry = Gtk.SearchEntry()

        self._build_catalog()
        self._setup_ui()
        self._setup_key_controller()

    def _build_catalog(self) -> None:
        """Build the master catalog of all actions and saved sessions."""
        settings = (
            self.parent_window.settings_manager
            if hasattr(self.parent_window, "settings_manager")
            else None
        )

        def get_accel_label(action_name: str) -> Optional[str]:
            if not settings:
                return None
            shortcut = settings.get_shortcut(action_name)
            if shortcut:
                return accelerator_to_label(shortcut)
            return None

        # 1. Tab and Window Actions
        self._items.append(
            CommandPaletteItem(
                "new-local-tab",
                _("Nova Aba"),
                _("Abas e Janelas"),
                "tab-new-symbolic",
                action_name="new-local-tab",
                shortcut=get_accel_label("new-local-tab"),
                keywords=["aba", "tab", "novo", "terminal", "nova"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "close-tab",
                _("Fechar Aba Atual"),
                _("Abas e Janelas"),
                "window-close-symbolic",
                action_name="close-tab",
                shortcut=get_accel_label("close-tab"),
                keywords=["fechar", "sair", "aba", "close", "tab"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "next-tab",
                _("Próxima Aba"),
                _("Abas e Janelas"),
                "go-next-symbolic",
                action_name="next-tab",
                shortcut=get_accel_label("next-tab"),
                keywords=["avançar", "proxima", "tab", "navegar"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "previous-tab",
                _("Aba Anterior"),
                _("Abas e Janelas"),
                "go-previous-symbolic",
                action_name="previous-tab",
                shortcut=get_accel_label("previous-tab"),
                keywords=["voltar", "anterior", "tab", "navegar"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "new-window",
                _("Nova Janela"),
                _("Abas e Janelas"),
                "window-new-symbolic",
                action_name="new-window",
                shortcut=get_accel_label("new-window"),
                keywords=["janela", "window", "novo"],
            )
        )

        # 2. Splits and Panes
        self._items.append(
            CommandPaletteItem(
                "split-horizontal",
                _("Dividir Painel Horizontalmente"),
                _("Divisão de Telas"),
                "view-grid-symbolic",
                action_name="split-horizontal",
                shortcut=get_accel_label("split-horizontal"),
                keywords=["split", "horizontal", "dividir", "painel"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "split-vertical",
                _("Dividir Painel Verticalmente"),
                _("Divisão de Telas"),
                "view-grid-symbolic",
                action_name="split-vertical",
                shortcut=get_accel_label("split-vertical"),
                keywords=["split", "vertical", "dividir", "painel"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "close-pane",
                _("Fechar Painel Dividido"),
                _("Divisão de Telas"),
                "window-close-symbolic",
                action_name="close-pane",
                shortcut=get_accel_label("close-pane"),
                keywords=["fechar", "painel", "split", "close"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "focus-pane-up",
                _("Focar Painel Acima"),
                _("Divisão de Telas"),
                "go-up-symbolic",
                action_name="focus-pane-up",
                shortcut=get_accel_label("focus-pane-up"),
                keywords=["foco", "cima", "acima", "painel"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "focus-pane-down",
                _("Focar Painel Abaixo"),
                _("Divisão de Telas"),
                "go-down-symbolic",
                action_name="focus-pane-down",
                shortcut=get_accel_label("focus-pane-down"),
                keywords=["foco", "baixo", "abaixo", "painel"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "focus-pane-left",
                _("Focar Painel à Esquerda"),
                _("Divisão de Telas"),
                "go-previous-symbolic",
                action_name="focus-pane-left",
                shortcut=get_accel_label("focus-pane-left"),
                keywords=["foco", "esquerda", "painel"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "focus-pane-right",
                _("Focar Painel à Direita"),
                _("Divisão de Telas"),
                "go-next-symbolic",
                action_name="focus-pane-right",
                shortcut=get_accel_label("focus-pane-right"),
                keywords=["foco", "direita", "painel"],
            )
        )

        # 3. Terminal & Editing Actions
        self._items.append(
            CommandPaletteItem(
                "copy",
                _("Copiar Seleção"),
                _("Terminal"),
                "edit-copy-symbolic",
                action_name="copy",
                shortcut=get_accel_label("copy"),
                keywords=["copiar", "copy", "clipboard"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "paste",
                _("Colar Área de Transferência"),
                _("Terminal"),
                "edit-paste-symbolic",
                action_name="paste",
                shortcut=get_accel_label("paste"),
                keywords=["colar", "paste", "clipboard"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "select-all",
                _("Selecionar Tudo"),
                _("Terminal"),
                "edit-select-all-symbolic",
                action_name="select-all",
                shortcut=get_accel_label("select-all"),
                keywords=["selecionar", "tudo", "select"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "clear-session",
                _("Limpar Tela do Terminal"),
                _("Terminal"),
                "edit-clear-symbolic",
                action_name="clear-session",
                shortcut=get_accel_label("clear-session"),
                keywords=["limpar", "clear", "tela", "reset"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "jump-previous-prompt",
                _("Pular para o Prompt Anterior"),
                _("Navegação Semântica"),
                "go-up-symbolic",
                action_name="jump-previous-prompt",
                shortcut="Alt + ↑",
                keywords=["prompt", "anterior", "subir", "comando", "osc133", "jump"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "jump-next-prompt",
                _("Pular para o Próximo Prompt"),
                _("Navegação Semântica"),
                "go-down-symbolic",
                action_name="jump-next-prompt",
                shortcut="Alt + ↓",
                keywords=["prompt", "proximo", "descer", "comando", "osc133", "jump"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "copy-last-output",
                _("Copiar Saída do Último Comando"),
                _("Navegação Semântica"),
                "edit-copy-symbolic",
                action_name="copy-last-output",
                keywords=["copiar", "saida", "output", "ultimo", "comando"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "analyze-last-error-ai",
                _("Analisar Último Erro com Assistente de IA"),
                _("Assistente de IA"),
                "sparkles-symbolic",
                action_name="analyze-last-error-ai",
                keywords=["ia", "erro", "analisar", "corrigir", "ai", "diagnostico", "error"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "show-command-history",
                _("Histórico Enriquecido de Comandos"),
                _("Terminal"),
                "document-open-recent-symbolic",
                action_name="show-command-history",
                shortcut="Ctrl + R",
                keywords=[
                    "historico",
                    "history",
                    "comandos",
                    "busca",
                    "fuzzy",
                    "sqlite",
                    "pwd",
                    "recent",
                    "favoritos",
                ],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "toggle-search",
                _("Buscar no Terminal"),
                _("Terminal"),
                "edit-find-symbolic",
                action_name="toggle-search",
                shortcut=get_accel_label("toggle-search"),
                keywords=["busca", "find", "procurar", "pesquisar"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "toggle-broadcast",
                _("Modo Transmissão (Comando p/ Todas as Abas)"),
                _("Terminal"),
                "network-transmit-symbolic",
                action_name="toggle-broadcast",
                shortcut=get_accel_label("toggle-broadcast"),
                keywords=["broadcast", "todas", "abas", "transmissao"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "zoom-in",
                _("Aumentar Zoom / Tamanho da Fonte"),
                _("Terminal"),
                "zoom-in-symbolic",
                action_name="zoom-in",
                shortcut=get_accel_label("zoom-in"),
                keywords=["zoom", "aumentar", "fonte", "texto"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "zoom-out",
                _("Diminuir Zoom / Tamanho da Fonte"),
                _("Terminal"),
                "zoom-out-symbolic",
                action_name="zoom-out",
                shortcut=get_accel_label("zoom-out"),
                keywords=["zoom", "diminuir", "fonte", "texto"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "zoom-reset",
                _("Resetar Zoom"),
                _("Terminal"),
                "zoom-original-symbolic",
                action_name="zoom-reset",
                shortcut=get_accel_label("zoom-reset"),
                keywords=["zoom", "resetar", "padrao"],
            )
        )

        # 4. AI & Agent Actions
        self._items.append(
            CommandPaletteItem(
                "ai-assistant",
                _("Assistente de IA (Abrir / Fechar)"),
                _("Assistente de IA"),
                "chat-message-new-symbolic",
                action_name="ai-assistant",
                shortcut=get_accel_label("ai-assistant"),
                keywords=["ia", "ai", "chat", "assistente", "llm", "abrir", "fechar"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "configure-ai",
                _("Configurar Provedores e Modelos de IA"),
                _("Assistente de IA"),
                "preferences-system-symbolic",
                action_name="configure-ai",
                keywords=["ia", "ai", "gemini", "groq", "openrouter", "ollama", "configurar"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "open-agent-scope",
                _("Políticas de Segurança e Escopo do Agente"),
                _("Assistente de IA"),
                "security-high-symbolic",
                callback=self._open_agent_scope,
                keywords=["segurança", "politicas", "escopo", "agente", "roots", "bloqueio"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "open-audit-log",
                _("Registro de Auditoria de Ações do Agente"),
                _("Assistente de IA"),
                "document-properties-symbolic",
                callback=self._open_audit_log,
                keywords=["auditoria", "log", "rollback", "historico", "agente"],
            )
        )

        # 5. Sessions & Management
        self._items.append(
            CommandPaletteItem(
                "toggle-sidebar",
                _("Painel Lateral de Sessões SSH"),
                _("Sessões & SSH"),
                "view-dual-symbolic",
                action_name="toggle-sidebar",
                shortcut=get_accel_label("toggle-sidebar"),
                keywords=["sessoes", "ssh", "sidebar", "painel", "sessao"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "toggle-file-manager",
                _("Gerenciador de Arquivos SFTP"),
                _("Sessões & SSH"),
                "folder-remote-symbolic",
                action_name="toggle-file-manager",
                shortcut=get_accel_label("toggle-file-manager"),
                keywords=["sftp", "arquivos", "file", "manager", "remoto"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "show-command-manager",
                _("Gerenciador de Comandos e Snippets"),
                _("Ferramentas"),
                "format-text-code-symbolic",
                action_name="show-command-manager",
                shortcut=get_accel_label("show-command-manager"),
                keywords=["snippets", "comandos", "atalhos", "scripts"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "toggle-tftp-server",
                _("Servidor TFTP Embutido"),
                _("Ferramentas"),
                "network-server-symbolic",
                action_name="toggle-tftp-server",
                keywords=["tftp", "servidor", "rede", "switch"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "save-layout",
                _("Salvar Layout de Sessões Atual"),
                _("Sessões & SSH"),
                "document-save-symbolic",
                action_name="save-layout",
                keywords=["salvar", "layout", "sessao"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "import-securecrt-sessions",
                _("Importar Sessões do SecureCRT"),
                _("Sessões & SSH"),
                "document-open-symbolic",
                action_name="import-securecrt-sessions",
                keywords=["importar", "securecrt", "sessoes"],
            )
        )

        # 6. Settings & Preferences
        self._items.append(
            CommandPaletteItem(
                "preferences",
                _("Preferências do Zashterminal"),
                _("Configurações"),
                "preferences-system-symbolic",
                action_name="preferences",
                shortcut=get_accel_label("preferences"),
                keywords=["preferencias", "configuracoes", "settings", "tema"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "shortcuts",
                _("Atalhos de Teclado"),
                _("Configurações"),
                "input-keyboard-symbolic",
                action_name="shortcuts",
                keywords=["atalhos", "teclado", "shortcuts", "keys"],
            )
        )
        self._items.append(
            CommandPaletteItem(
                "highlight-settings",
                _("Regras de Realce Sintático e Temas"),
                _("Configurações"),
                "applications-graphics-symbolic",
                action_name="highlight-settings",
                keywords=["realce", "sintaxe", "highlight", "cores"],
            )
        )

        # 7. Dynamic Saved SSH Sessions
        try:
            storage = SessionStorageManager()
            saved_sessions, _folders = storage.load_sessions()
            for session in saved_sessions:
                session_name = getattr(session, "name", "Session")
                host = getattr(session, "host", "")
                user = getattr(session, "user", "")
                icon = (
                    "network-server-symbolic"
                    if getattr(session, "protocol", "ssh") == "ssh"
                    else "utilities-terminal-symbolic"
                )
                self._items.append(
                    CommandPaletteItem(
                        f"session:{session_name}",
                        f"{_('Conectar:')} {session_name}",
                        _("Sessões SSH"),
                        icon,
                        callback=lambda s=session: self._connect_session(s),
                        keywords=[
                            "ssh",
                            "conectar",
                            "connect",
                            "sessao",
                            "sessoes",
                            session_name,
                            host,
                            user,
                        ],
                    )
                )
        except Exception as e:
            self.logger.debug(f"Could not load sessions into palette: {e}")

    def _connect_session(self, session: Any) -> None:
        """Connect to a saved session."""
        if hasattr(self.parent_window, "_on_session_activated"):
            self.parent_window._on_session_activated(session)

    def _setup_ui(self) -> None:
        """Construct the Command Palette UI."""
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        main_box.set_margin_top(12)
        main_box.set_margin_bottom(12)
        main_box.set_margin_start(12)
        main_box.set_margin_end(12)

        # Search Entry at top
        self._search_entry.set_placeholder_text(
            _("Digite um comando ou busque uma ação...")
        )
        self._search_entry.add_css_class("command-palette-search")
        self._search_entry.connect("search-changed", self._on_search_changed)
        self._search_entry.connect("activate", self._on_entry_activate)
        main_box.append(self._search_entry)

        # Scrolled Results List
        scrolled = Gtk.ScrolledWindow()
        scrolled.set_vexpand(True)
        scrolled.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        scrolled.set_min_content_height(320)

        self._list_box.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self._list_box.set_activate_on_single_click(True)
        self._list_box.add_css_class("rich-list")
        self._list_box.add_css_class("boxed-list")
        self._list_box.connect("row-activated", self._on_row_activated)
        scrolled.set_child(self._list_box)

        main_box.append(scrolled)
        self.set_content(main_box)

        # Initial population
        self._filter_items("")

        # Stop search or Escape closes the palette
        self._search_entry.connect("stop-search", lambda _e: self.close())

        # Grab focus on search entry when shown
        self.connect("show", lambda w: self._search_entry.grab_focus())

    def _setup_key_controller(self) -> None:
        """Attach keyboard controller for Up/Down/Enter/Escape navigation."""
        key_controller = Gtk.EventControllerKey()
        key_controller.connect("key-pressed", self._on_key_pressed)
        self.add_controller(key_controller)

        # Also attach to search entry specifically for immediate Escape handling
        search_key_ctrl = Gtk.EventControllerKey()
        search_key_ctrl.connect("key-pressed", self._on_key_pressed)
        self._search_entry.add_controller(search_key_ctrl)

    def _on_key_pressed(
        self,
        controller: Gtk.EventControllerKey,
        keyval: int,
        keycode: int,
        state: Gdk.ModifierType,
    ) -> bool:
        """Handle Enter, Arrow keys, and Escape."""
        if keyval == Gdk.KEY_Escape:
            self.close()
            return True

        if keyval in (Gdk.KEY_Return, Gdk.KEY_KP_Enter):
            selected_row = self._list_box.get_selected_row()
            if not selected_row:
                selected_row = self._list_box.get_row_at_index(0)
            if selected_row:
                self._on_row_activated(self._list_box, selected_row)
                return True
            return False

        if keyval in (Gdk.KEY_Down, Gdk.KEY_Up):
            selected_row = self._list_box.get_selected_row()
            current_index = selected_row.get_index() if selected_row else -1

            num_rows = len(self._filtered_items)
            if num_rows == 0:
                return False

            if keyval == Gdk.KEY_Down:
                next_index = min(current_index + 1, num_rows - 1)
            else:
                next_index = max(current_index - 1, 0)

            target_row = self._list_box.get_row_at_index(next_index)
            if target_row:
                self._list_box.select_row(target_row)
                target_row.grab_focus()
                self._search_entry.grab_focus()
            return True

        return False

    def _on_search_changed(self, entry: Gtk.SearchEntry) -> None:
        """Filter command items on text change."""
        query = entry.get_text()
        self._filter_items(query)

    def _filter_items(self, query: str) -> None:
        """Filter and sort items based on search query."""
        # Clear existing rows
        while (child := self._list_box.get_first_child()) is not None:
            self._list_box.remove(child)

        matching = [item for item in self._items if item.matches(query)]
        if query.strip():
            matching.sort(key=lambda item: item.score(query), reverse=True)

        self._filtered_items = matching

        for item in self._filtered_items:
            row = self._create_row(item)
            self._list_box.append(row)

        # Auto select first row
        first_row = self._list_box.get_row_at_index(0)
        if first_row:
            self._list_box.select_row(first_row)

    def _create_row(self, item: CommandPaletteItem) -> Gtk.ListBoxRow:
        """Build row widget for a command item."""
        row = Gtk.ListBoxRow()
        row._palette_item = item  # Attach reference

        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        box.set_margin_top(8)
        box.set_margin_bottom(8)
        box.set_margin_start(10)
        box.set_margin_end(10)

        # Icon
        icon = icon_image(item.icon_name) or Gtk.Image.new_from_icon_name(item.icon_name)
        icon.set_pixel_size(20)
        icon.set_valign(Gtk.Align.CENTER)
        box.append(icon)

        # Text labels (Title + Category)
        text_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        text_box.set_hexpand(True)
        text_box.set_valign(Gtk.Align.CENTER)

        title_label = Gtk.Label(label=item.title)
        title_label.set_halign(Gtk.Align.START)
        title_label.set_ellipsize(Pango.EllipsizeMode.END)
        title_label.add_css_class("heading")
        text_box.append(title_label)

        cat_label = Gtk.Label(label=item.category)
        cat_label.set_halign(Gtk.Align.START)
        cat_label.add_css_class("caption")
        cat_label.add_css_class("dim-label")
        text_box.append(cat_label)

        box.append(text_box)

        # Shortcut badge (if any)
        if item.shortcut:
            badge = Gtk.Label(label=item.shortcut)
            badge.add_css_class("dim-label")
            badge.add_css_class("caption")
            badge.add_css_class("command-palette-badge")
            badge.set_valign(Gtk.Align.CENTER)
            box.append(badge)

        row.set_child(box)
        return row

    def _on_entry_activate(self, _entry: Gtk.SearchEntry) -> None:
        """Handle Enter key on search entry."""
        selected_row = self._list_box.get_selected_row()
        if not selected_row:
            selected_row = self._list_box.get_row_at_index(0)
        if selected_row:
            self._on_row_activated(self._list_box, selected_row)

    def _on_row_activated(self, _list_box: Gtk.ListBox, row: Gtk.ListBoxRow) -> None:
        """Execute selected item action and close dialog."""
        item: Optional[CommandPaletteItem] = getattr(row, "_palette_item", None)
        self.close()

        if not item:
            return

        GLib.idle_add(self._execute_item, item)

    def _execute_item(self, item: CommandPaletteItem) -> bool:
        """Execute action callback or activate window action."""
        try:
            self.logger.info(f"Executing palette action: {item.item_id}")
            if item.callback:
                item.callback()
                return False

            if not item.action_name:
                return False

            handler = getattr(self.parent_window, "action_handler", None)

            # 1. Action-specific handlers
            if item.action_name == "ai-assistant":
                if hasattr(self.parent_window, "_on_ai_assistant_requested"):
                    self.parent_window._on_ai_assistant_requested()
                    return False
            elif item.action_name == "next-tab":
                if hasattr(self.parent_window, "tab_manager"):
                    self.parent_window.tab_manager.select_next_tab()
                    return False
            elif item.action_name == "previous-tab":
                if hasattr(self.parent_window, "tab_manager"):
                    self.parent_window.tab_manager.select_previous_tab()
                    return False
            elif item.action_name == "toggle-sidebar":
                if handler and hasattr(handler, "toggle_sidebar_action"):
                    handler.toggle_sidebar_action()
                    return False

            # 2. Direct method on WindowActions handler
            if handler:
                method_name = item.action_name.replace("-", "_")
                if hasattr(handler, method_name):
                    method = getattr(handler, method_name)
                    method()
                    return False

            # 3. Lookup and activate Gio.Action on window
            act = self.parent_window.lookup_action(item.action_name)
            if act:
                act.activate(None)
                return False

            # 4. Lookup on application
            app = self.parent_window.get_application()
            if app:
                app_act = app.lookup_action(item.action_name)
                if app_act:
                    app_act.activate(None)
                    return False

            # 5. Fallback widget activation
            self.parent_window.activate_action(f"win.{item.action_name}", None)
        except Exception as e:
            self.logger.error(f"Error executing palette action '{item.item_id}': {e}", exc_info=True)
        return False

    def _open_agent_scope(self) -> None:
        """Helper to open agent scope dialog."""
        from .agent_scope_dialog import AgentScopeDialog

        settings = (
            self.parent_window.settings_manager
            if hasattr(self.parent_window, "settings_manager")
            else None
        )
        dialog = AgentScopeDialog(self.parent_window, settings)
        dialog.present()

    def _open_audit_log(self) -> None:
        """Helper to open audit log dialog."""
        from .audit_log_dialog import AuditLogDialog

        dialog = AuditLogDialog(self.parent_window)
        dialog.present()
