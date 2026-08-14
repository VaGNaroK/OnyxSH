"""Dialog for configuring the security scope, allowed roots, and policies of the AI Agent."""

from __future__ import annotations

from pathlib import Path
from typing import Optional

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, GLib, Gtk

from ...settings.manager import SettingsManager, get_settings_manager
from ...utils.logger import get_logger
from ...utils.translation_utils import _
from .base_dialog import BaseDialog


class AgentScopeDialog(BaseDialog):
    """Dialog allowing users to inspect and adjust allowed directories, denylists, and trust rules."""

    def __init__(self, parent_window, settings_manager: Optional[SettingsManager] = None) -> None:
        super().__init__(
            parent_window=parent_window,
            dialog_title=_("Configuração de Escopo e Segurança do Agente"),
            auto_setup_toolbar=True,
            default_width=680,
            default_height=520,
        )
        self.logger = get_logger("zashterminal.ui.dialogs.agent_scope")
        self.settings_manager = settings_manager or get_settings_manager()
        self._build_content()

    def _build_content(self) -> None:
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=16)
        main_box.set_margin_top(16)
        main_box.set_margin_bottom(16)
        main_box.set_margin_start(16)
        main_box.set_margin_end(16)

        # 1. Allowed Roots Group
        roots_group = Adw.PreferencesGroup()
        roots_group.set_title(_("Diretórios Permitidos para Ações do Agente"))
        roots_group.set_description(
            _("O agente tem permissão de leitura/escrita restrita apenas aos diretórios listados abaixo.")
        )

        allowed_roots = self.settings_manager.get("ai_agent_allowed_roots", [])
        for path_str in allowed_roots:
            row = Adw.ActionRow()
            row.set_title(path_str)
            roots_group.add(row)

        main_box.append(roots_group)

        # 2. Protection Policies Group
        policy_group = Adw.PreferencesGroup()
        policy_group.set_title(_("Proteções Ativas (Default-Deny)"))

        row_ssh = Adw.ActionRow()
        row_ssh.set_title(_("Bloqueio de Chaves e Credenciais"))
        row_ssh.set_subtitle(_("~/.ssh, ~/.gnupg, ~/.aws, ~/.docker, arquivos .env e históricos"))
        badge_ssh = Gtk.Label(label="🛡️ Ativo")
        badge_ssh.add_css_class("accent")
        row_ssh.add_suffix(badge_ssh)
        policy_group.add(row_ssh)

        row_dotfiles = Adw.ActionRow()
        row_dotfiles.set_title(_("Proteção de Inicialização de Shell"))
        row_dotfiles.set_subtitle(_("~/.bashrc, ~/.zshrc, ~/.profile e diretórios de sistema"))
        badge_dot = Gtk.Label(label="🛡️ Ativo")
        badge_dot.add_css_class("accent")
        row_dotfiles.add_suffix(badge_dot)
        policy_group.add(row_dotfiles)

        row_redactor = Adw.ActionRow()
        row_redactor.set_title(_("Ofuscação Automática de Segredos"))
        row_redactor.set_subtitle(_("Filtra chaves de API, senhas e tokens antes de qualquer envio ao LLM"))
        badge_red = Gtk.Label(label="🛡️ Ativo")
        badge_red.add_css_class("accent")
        row_redactor.add_suffix(badge_red)
        policy_group.add(row_redactor)

        main_box.append(policy_group)

        if self._scrolled_window:
            self._scrolled_window.set_child(main_box)
        else:
            self.set_content(main_box)
