# onyxsh/ui/dialogs/ai_config_dialog.py

"""AI Assistant configuration dialog."""

import threading
from typing import List, Tuple

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, GLib, GObject, Gtk

from ...settings.manager import SettingsManager
from ...utils.logger import get_logger
from ...utils.translation_utils import _


class AIConfigDialog(Adw.PreferencesWindow):
    """Dialog for configuring AI assistant settings."""

    __gsignals__ = {
        "setting-changed": (GObject.SignalFlags.RUN_FIRST, None, (str, object)),
    }

    # Provider configurations
    PROVIDERS = [
        ("groq", "Groq", "https://api.groq.com/openai/v1"),
        ("gemini", "Gemini", "https://generativelanguage.googleapis.com"),
        ("openrouter", "OpenRouter", "https://openrouter.ai/api/v1"),
        ("local", "Local (Ollama/LM Studio)", "http://localhost:11434/v1"),
    ]

    DEFAULT_MODELS = {
        "groq": "llama-3.1-8b-instant",
        "gemini": "gemini-2.5-flash",
        "openrouter": "openrouter/polaris-alpha",
        "local": "llama3.2",
    }

    CONTEXT_SIZES: List[Tuple[int, str]] = [
        (4096, "4.096 tokens (4K) — Econômico (CPU / GPU < 4GB)"),
        (8192, "8.192 tokens (8K) — Padrão Equilibrado"),
        (16384, "16.384 tokens (16K) — Estendido (GPU 8GB - 12GB)"),
        (32768, "32.768 tokens (32K) — Amplo (GPU 12GB - 16GB / Cloud)"),
        (65536, "65.536 tokens (64K) — Avançado (GPU 24GB+ / Cloud)"),
        (131072, "131.072 tokens (128K) — Máximo (Provedores Cloud)"),
    ]

    def __init__(self, parent_window, settings_manager: SettingsManager):
        super().__init__(
            title=_("Configure AI Assistant"),
            transient_for=parent_window,
            modal=True,
            default_width=750,
            default_height=600,
            search_enabled=False,
        )
        self.add_css_class("onyxsh-dialog")
        self.logger = get_logger("onyxsh.ui.dialogs.ai_config")
        self.settings_manager = settings_manager

        self._setup_ui()
        self.logger.info("AI config dialog initialized")

    def _setup_ui(self) -> None:
        """Set up the dialog UI."""
        page = Adw.PreferencesPage(title=_("AI Assistant"))
        self.add(page)

        # Enable/Disable group - MUST be first element user sees
        enable_group = Adw.PreferencesGroup()
        page.add(enable_group)

        self.enable_switch = Adw.SwitchRow(
            title=_("Enable AI Assistant"),
            subtitle=_("Show the AI Assistant button in the header bar."),
        )
        self.enable_switch.set_active(
            self.settings_manager.get("ai_assistant_enabled", False)
        )
        self.enable_switch.connect("notify::active", self._on_enable_changed)
        enable_group.add(self.enable_switch)

        # Privacy & Offline Mode Group
        privacy_group = Adw.PreferencesGroup(
            title=_("Privacidade e Modo Offline"),
            description=_("Controle de conexão externa e isolamento de dados."),
        )
        page.add(privacy_group)

        self.offline_switch = Adw.SwitchRow(
            title=_("Modo Estritamente Offline (Local-Only)"),
            subtitle=_(
                "Bloqueia qualquer requisição externa (Gemini, Groq, OpenRouter) e restringe a IA a modelos locais no Ollama ou LM Studio."
            ),
        )
        self.offline_switch.set_active(
            self.settings_manager.get("ai_assistant_offline_mode", False)
        )
        self.offline_switch.connect("notify::active", self._on_offline_changed)
        privacy_group.add(self.offline_switch)

        # Provider selection group
        provider_group = Adw.PreferencesGroup()
        page.add(provider_group)

        # Provider combo row
        self.provider_row = Adw.ComboRow(
            title=_("Provider"),
            subtitle=_("Choose between cloud providers or local models."),
        )
        provider_model = Gtk.StringList.new([label for _, label, _ in self.PROVIDERS])
        self.provider_row.set_model(provider_model)

        # Set current provider
        current_provider = self.settings_manager.get("ai_assistant_provider", "groq")
        provider_index = self._get_provider_index(current_provider)
        self.provider_row.set_selected(provider_index)
        self.provider_row.connect("notify::selected", self._on_provider_changed)
        provider_group.add(self.provider_row)

        # Base URL row (for local providers)
        self.base_url_row = Adw.EntryRow(
            title=_("Base URL"),
        )
        self.base_url_row.set_text(
            self.settings_manager.get("ai_local_base_url", "http://localhost:11434/v1")
        )
        self.base_url_row.connect("changed", self._on_base_url_changed)
        provider_group.add(self.base_url_row)

        # Local model options (preload and unload)
        self.preload_switch = Adw.SwitchRow(
            title=_("Pré-carregar Modelo na VRAM ao Iniciar"),
            subtitle=_("Carrega o modelo na GPU em segundo plano ao abrir o terminal para respostas imediatas."),
        )
        self.preload_switch.set_active(
            self.settings_manager.get("ai_preload_local_model", True)
        )
        self.preload_switch.connect("notify::active", self._on_preload_changed)
        provider_group.add(self.preload_switch)

        self.unload_switch = Adw.SwitchRow(
            title=_("Liberar VRAM ao Fechar o Terminal"),
            subtitle=_("Descarrega o modelo da memória GPU imediatamente ao fechar a aplicação."),
        )
        self.unload_switch.set_active(
            self.settings_manager.get("ai_unload_on_exit", True)
        )
        self.unload_switch.connect("notify::active", self._on_unload_changed)
        provider_group.add(self.unload_switch)

        # API Key group
        api_group = Adw.PreferencesGroup()
        page.add(api_group)

        # API Key row
        self.api_key_row = Adw.PasswordEntryRow(
            title=_("API Key"),
        )
        self.api_key_row.set_text(
            self.settings_manager.get("ai_assistant_api_key", "")
        )
        self.api_key_row.connect("changed", self._on_api_key_changed)
        api_group.add(self.api_key_row)

        # Model selection group
        model_group = Adw.PreferencesGroup()
        page.add(model_group)

        # Model entry row
        self.model_row = Adw.EntryRow(
            title=_("Model Identifier"),
        )
        self.model_row.set_text(
            self.settings_manager.get("ai_assistant_model", "")
        )
        self.model_row.connect("changed", self._on_model_changed)
        model_group.add(self.model_row)

        # Browse models button (for OpenRouter - opens searchable dialog)
        self.browse_models_row = Adw.ActionRow(
            title=_("Browse Available Models"),
            subtitle=_("Search and select from available OpenRouter models."),
        )
        self.browse_models_button = Gtk.Button(label=_("Browse Models"))
        self.browse_models_button.set_valign(Gtk.Align.CENTER)
        self.browse_models_button.connect("clicked", self._on_browse_models_clicked)
        self.browse_models_row.add_suffix(self.browse_models_button)
        self.browse_models_row.set_activatable_widget(self.browse_models_button)
        model_group.add(self.browse_models_row)

        # OpenRouter-specific settings group
        self.openrouter_group = Adw.PreferencesGroup(
            title=_("OpenRouter Settings"),
            description=_("Additional settings for OpenRouter API rankings."),
        )
        page.add(self.openrouter_group)

        # Site URL row
        self.site_url_row = Adw.EntryRow(
            title=_("Site URL (optional)"),
        )
        self.site_url_row.set_text(
            self.settings_manager.get("ai_openrouter_site_url", "")
        )
        self.site_url_row.connect("changed", self._on_site_url_changed)
        self.openrouter_group.add(self.site_url_row)

        # Site name row
        self.site_name_row = Adw.EntryRow(
            title=_("Site Name (optional)"),
        )
        self.site_name_row.set_text(
            self.settings_manager.get("ai_openrouter_site_name", "")
        )
        self.site_name_row.connect("changed", self._on_site_name_changed)
        self.openrouter_group.add(self.site_name_row)

        # Smart Model Routing Group
        routing_group = Adw.PreferencesGroup(
            title=_("Roteamento Inteligente de Modelos (Smart Routing)"),
            description=_(
                "Aloca automaticamente entre modelos rápidos e modelos avançados com base na complexidade da tarefa."
            ),
        )
        page.add(routing_group)

        self.smart_routing_switch = Adw.SwitchRow(
            title=_("Ativar Roteamento Inteligente"),
            subtitle=_(
                "Direciona consultas simples para o perfil rápido e planos/scripts complexos para o perfil avançado."
            ),
        )
        self.smart_routing_switch.set_active(
            self.settings_manager.get("ai_smart_routing_enabled", True)
        )
        self.smart_routing_switch.connect(
            "notify::active",
            lambda r, _: self._on_agent_setting_changed("ai_smart_routing_enabled", r.get_active()),
        )
        routing_group.add(self.smart_routing_switch)

        # Profile selection
        self.routing_profile_row = Adw.ComboRow(
            title=_("Modo de Roteamento Padrão"),
            subtitle=_("Escolha se a IA deve alternar automaticamente ou fixar um perfil."),
        )
        profile_options = [
            _("Automático (Classifica complexidade do prompt)"),
            _("Sempre Rápido (Baixa latência / Groq / Local)"),
            _("Sempre Avançado (Raciocínio profundo / Gemini / Claude)"),
        ]
        self.routing_profile_row.set_model(Gtk.StringList.new(profile_options))
        curr_profile = self.settings_manager.get("ai_routing_profile", "auto").lower()
        profile_map = {"auto": 0, "fast": 1, "advanced": 2}
        self.routing_profile_row.set_selected(profile_map.get(curr_profile, 0))
        self.routing_profile_row.connect(
            "notify::selected",
            self._on_routing_profile_changed,
        )
        routing_group.add(self.routing_profile_row)

        # Fast Profile Expander
        fast_expander = Adw.ExpanderRow(
            title=_("⚡ Perfil Rápido (Consultas Pontuais / Sintaxe)"),
            subtitle=_("Modelo e provedor de baixa latência para dúvidas do dia a dia."),
        )
        self.fast_provider_row = Adw.ComboRow(
            title=_("Provedor Rápido"),
        )
        self.fast_provider_row.set_model(Gtk.StringList.new([label for _, label, _ in self.PROVIDERS]))
        curr_fast_p = self.settings_manager.get("ai_fast_provider", "groq")
        self.fast_provider_row.set_selected(self._get_provider_index(curr_fast_p))
        self.fast_provider_row.connect(
            "notify::selected",
            lambda r, _: self._on_fast_provider_changed(r),
        )
        fast_expander.add_row(self.fast_provider_row)

        self.fast_model_row = Adw.EntryRow(
            title=_("Modelo Rápido"),
        )
        self.fast_model_row.set_text(self.settings_manager.get("ai_fast_model", "llama-3.1-8b-instant"))
        self.fast_model_row.connect(
            "changed",
            lambda r: self._on_agent_setting_changed("ai_fast_model", r.get_text().strip()),
        )
        fast_expander.add_row(self.fast_model_row)
        routing_group.add(fast_expander)

        # Advanced Profile Expander
        adv_expander = Adw.ExpanderRow(
            title=_("🧠 Perfil Avançado (Planos Complexos / Scripts / Diagnósticos)"),
            subtitle=_("Modelo e provedor de alto raciocínio para orquestração de múltiplos passos."),
        )
        self.adv_provider_row = Adw.ComboRow(
            title=_("Provedor Avançado"),
        )
        self.adv_provider_row.set_model(Gtk.StringList.new([label for _, label, _ in self.PROVIDERS]))
        curr_adv_p = self.settings_manager.get("ai_advanced_provider", "gemini")
        self.adv_provider_row.set_selected(self._get_provider_index(curr_adv_p))
        self.adv_provider_row.connect(
            "notify::selected",
            lambda r, _: self._on_adv_provider_changed(r),
        )
        adv_expander.add_row(self.adv_provider_row)

        self.adv_model_row = Adw.EntryRow(
            title=_("Modelo Avançado"),
        )
        self.adv_model_row.set_text(self.settings_manager.get("ai_advanced_model", "gemini-2.5-flash"))
        self.adv_model_row.connect(
            "changed",
            lambda r: self._on_agent_setting_changed("ai_advanced_model", r.get_text().strip()),
        )
        adv_expander.add_row(self.adv_model_row)
        routing_group.add(adv_expander)

        # Provider API Keys Expander
        keys_expander = Adw.ExpanderRow(
            title=_("🔑 Chaves de API por Provedor"),
            subtitle=_("Configure as chaves individuais para alternar entre provedores sem reconfigurar."),
        )

        self.key_gemini_row = Adw.PasswordEntryRow(title=_("Google Gemini API Key"))
        self.key_gemini_row.set_text(self.settings_manager.get("ai_api_key_gemini", ""))
        self.key_gemini_row.connect(
            "changed",
            lambda r: self._on_agent_setting_changed("ai_api_key_gemini", r.get_text().strip()),
        )
        self.btn_test_gemini = Gtk.Button(label=_("Testar"))
        self.btn_test_gemini.add_css_class("flat")
        self.btn_test_gemini.set_valign(Gtk.Align.CENTER)
        self.btn_test_gemini.connect("clicked", self._on_test_gemini_key)
        self.key_gemini_row.add_suffix(self.btn_test_gemini)
        keys_expander.add_row(self.key_gemini_row)

        self.key_groq_row = Adw.PasswordEntryRow(title=_("Groq API Key"))
        self.key_groq_row.set_text(self.settings_manager.get("ai_api_key_groq", ""))
        self.key_groq_row.connect(
            "changed",
            lambda r: self._on_agent_setting_changed("ai_api_key_groq", r.get_text().strip()),
        )
        self.btn_test_groq = Gtk.Button(label=_("Testar"))
        self.btn_test_groq.add_css_class("flat")
        self.btn_test_groq.set_valign(Gtk.Align.CENTER)
        self.btn_test_groq.connect("clicked", self._on_test_groq_key)
        self.key_groq_row.add_suffix(self.btn_test_groq)
        keys_expander.add_row(self.key_groq_row)

        self.key_openrouter_row = Adw.PasswordEntryRow(title=_("OpenRouter API Key"))
        self.key_openrouter_row.set_text(self.settings_manager.get("ai_api_key_openrouter", ""))
        self.key_openrouter_row.connect(
            "changed",
            lambda r: self._on_agent_setting_changed("ai_api_key_openrouter", r.get_text().strip()),
        )
        keys_expander.add_row(self.key_openrouter_row)
        routing_group.add(keys_expander)

        # Secure Agent & Context Group
        agent_group = Adw.PreferencesGroup(
            title=_("Modo Agente Seguro e Contexto"),
            description=_("Parâmetros de proteção, níveis de autonomia e contexto do sistema."),
        )
        page.add(agent_group)

        # Max Risk Level Combo
        self.max_risk_row = Adw.ComboRow(
            title=_("Nível Máximo de Autonomia Permitido"),
            subtitle=_("Define o limite máximo de risco para comandos propostos pela IA."),
        )
        risk_options = [
            _("🟢 Nível 0: Somente Leitura (Diagnóstico e Inspeção)"),
            _("🔵 Nível 1: Escrita no Usuário (Criar/Editar Scripts e Arquivos)"),
            _("🟠 Nível 2: Administração Polkit (Serviços e Manutenção)"),
            _("🔴 Nível 3: Operações Críticas (Pacotes e Sistema Global)"),
        ]
        self.max_risk_row.set_model(Gtk.StringList.new(risk_options))
        current_max_risk = min(3, max(0, int(self.settings_manager.get("ai_agent_max_risk_level", 3))))
        self.max_risk_row.set_selected(current_max_risk)
        self.max_risk_row.connect(
            "notify::selected",
            lambda r, _: self._on_agent_setting_changed("ai_agent_max_risk_level", r.get_selected())
        )
        agent_group.add(self.max_risk_row)

        self.auto_run_l0_row = Adw.SwitchRow(
            title=_("Auto-executar Comandos Seguros (Nível 0)"),
            subtitle=_("Executa automaticamente diagnósticos somente-leitura sem exigir confirmação."),
        )
        self.auto_run_l0_row.set_active(
            self.settings_manager.get("ai_agent_auto_run_level0", False)
        )
        self.auto_run_l0_row.connect(
            "notify::active",
            lambda r, _: self._on_agent_setting_changed("ai_agent_auto_run_level0", r.get_active())
        )
        agent_group.add(self.auto_run_l0_row)

        # Post-Verification Switches
        self.post_verify_row = Adw.SwitchRow(
            title=_("Sugerir Verificação Pós-Execução (Sanity Loop)"),
            subtitle=_("Infere e propõe validações automáticas (status de serviços, sintaxe de configs, permissões) após comandos mutantes."),
        )
        self.post_verify_row.set_active(
            self.settings_manager.get("ai_agent_post_verification", True)
        )
        self.post_verify_row.connect(
            "notify::active",
            lambda r, _: self._on_agent_setting_changed("ai_agent_post_verification", r.get_active())
        )
        agent_group.add(self.post_verify_row)

        self.auto_verify_row = Adw.SwitchRow(
            title=_("Executar Verificações Automaticamente"),
            subtitle=_("Dispara os testes de sanidade automaticamente sem exigir clique manual no botão de validar."),
        )
        self.auto_verify_row.set_active(
            self.settings_manager.get("ai_agent_auto_verify", False)
        )
        self.auto_verify_row.connect(
            "notify::active",
            lambda r, _: self._on_agent_setting_changed("ai_agent_auto_verify", r.get_active())
        )
        agent_group.add(self.auto_verify_row)

        # Context Switches
        self.sys_context_row = Adw.SwitchRow(
            title=_("Incluir Contexto do Sistema e Hardware"),
            subtitle=_("Informa à IA a distribuição Linux, arquitetura e dados do sistema para comandos exatos."),
        )
        self.sys_context_row.set_active(
            self.settings_manager.get("ai_agent_include_system_context", True)
        )
        self.sys_context_row.connect(
            "notify::active",
            lambda r, _: self._on_agent_setting_changed("ai_agent_include_system_context", r.get_active())
        )
        agent_group.add(self.sys_context_row)

        self.pwd_context_row = Adw.SwitchRow(
            title=_("Incluir Diretório Atual (PWD) no Contexto"),
            subtitle=_("Permite que a IA use caminhos relativos ao diretório em que você está navegando."),
        )
        self.pwd_context_row.set_active(
            self.settings_manager.get("ai_agent_include_pwd_context", True)
        )
        self.pwd_context_row.connect(
            "notify::active",
            lambda r, _: self._on_agent_setting_changed("ai_agent_include_pwd_context", r.get_active())
        )
        agent_group.add(self.pwd_context_row)

        # Context Size Combo Row with GPU detection
        from ...utils.platform import detect_gpu_info
        gpu_info = detect_gpu_info()
        gpu_desc = gpu_info.get("description", "Recomendado: 8K")

        self.context_size_row = Adw.ComboRow(
            title=_("Tamanho da Janela de Contexto (Tokens)"),
            subtitle=_("Hardware: {desc}").format(desc=gpu_desc),
        )
        context_labels = [label for _, label in self.CONTEXT_SIZES]
        self.context_size_row.set_model(Gtk.StringList.new(context_labels))

        current_context_size = int(self.settings_manager.get("ai_context_size", 8192))
        context_idx = self._get_context_size_index(current_context_size)
        self.context_size_row.set_selected(context_idx)
        self.context_size_row.connect("notify::selected", self._on_context_size_changed)
        agent_group.add(self.context_size_row)

        self.scope_row = Adw.ActionRow(
            title=_("Escopo de Diretórios e Políticas"),
            subtitle=_("Configurar pastas autorizadas e proteções de sistema."),
        )
        scope_btn = Gtk.Button(label=_("Configurar Escopo"))
        scope_btn.set_valign(Gtk.Align.CENTER)
        scope_btn.connect("clicked", self._on_scope_clicked)
        self.scope_row.add_suffix(scope_btn)
        self.scope_row.set_activatable_widget(scope_btn)
        agent_group.add(self.scope_row)

        # Visual Levels Guide Expander
        levels_expander = Adw.ExpanderRow(
            title=_("ℹ️ Guia dos Níveis de Risco do Contexto"),
            subtitle=_("Clique para entender o que cada nível de permissão autoriza ou restringe."),
        )
        
        row_l0 = Adw.ActionRow(
            title=_("🟢 Nível 0: Somente Leitura (Diagnóstico)"),
            subtitle=_("Comandos puramente informativos (lshw, df, free, ps, ls, cat). Risco zero."),
        )
        levels_expander.add_row(row_l0)

        row_l1 = Adw.ActionRow(
            title=_("🔵 Nível 1: Escrita no Espaço do Usuário"),
            subtitle=_("Criação de scripts, edição de arquivos e git na pasta home. Requer 1 clique de aprovação."),
        )
        levels_expander.add_row(row_l1)

        row_l2 = Adw.ActionRow(
            title=_("🟠 Nível 2: Administração Polkit"),
            subtitle=_("Serviços e configurações do sistema (systemctl, journalctl). Requer autenticação segura."),
        )
        levels_expander.add_row(row_l2)

        row_l3 = Adw.ActionRow(
            title=_("🔴 Nível 3: Operações Críticas Globais"),
            subtitle=_("Instalação/remoção de pacotes e arquivos em /etc (apt, dpkg). Exige confirmação explícita."),
        )
        levels_expander.add_row(row_l3)

        row_l4 = Adw.ActionRow(
            title=_("⛔ Nível 4: Ações Destrutivas (Bloqueado)"),
            subtitle=_("Comandos perigosos (rm -rf /, mkfs, dd). Permanentemente bloqueados pelo motor de segurança."),
        )
        levels_expander.add_row(row_l4)

        agent_group.add(levels_expander)

        # Update UI based on current provider
        self._update_ui_for_provider(current_provider)

    def _get_context_size_index(self, size: int) -> int:
        """Get the index of a context size in the CONTEXT_SIZES list."""
        for i, (val, _) in enumerate(self.CONTEXT_SIZES):
            if val == size:
                return i
        return 1  # Default 8192

    def _on_context_size_changed(self, combo_row, _param) -> None:
        """Handle context window size selection change."""
        idx = combo_row.get_selected()
        if 0 <= idx < len(self.CONTEXT_SIZES):
            val = self.CONTEXT_SIZES[idx][0]
            self.settings_manager.set("ai_context_size", val)
            self.emit("setting-changed", "ai_context_size", val)

    def _get_provider_index(self, provider_id: str) -> int:
        """Get the index of a provider in the PROVIDERS list."""
        for i, (pid, _name, _desc) in enumerate(self.PROVIDERS):
            if pid == provider_id:
                return i
        return 0

    def _get_selected_provider_id(self) -> str:
        """Get the currently selected provider ID."""
        index = self.provider_row.get_selected()
        if 0 <= index < len(self.PROVIDERS):
            return self.PROVIDERS[index][0]
        return "groq"

    def _update_ui_for_provider(self, provider_id: str) -> None:
        """Update UI elements based on the selected provider and offline mode."""
        is_offline = self.settings_manager.get("ai_assistant_offline_mode", False)
        is_local = provider_id == "local"
        is_openrouter = provider_id == "openrouter"

        # If offline mode is active, provider is strictly local and combo is restricted
        if is_offline:
            self.provider_row.set_subtitle(
                _("🔒 Restrito ao provedor Local pelo Modo Estritamente Offline.")
            )
            self.provider_row.set_sensitive(False)
            self.api_key_row.set_sensitive(False)
        else:
            self.provider_row.set_subtitle(
                _("Choose between cloud providers or local models.")
            )
            self.provider_row.set_sensitive(True)
            self.api_key_row.set_sensitive(not is_local or False)

        # Show/hide base URL and VRAM switches for local provider
        self.base_url_row.set_visible(is_local)
        self.preload_switch.set_visible(is_local)
        self.unload_switch.set_visible(is_local)

        # Show/hide browse models button (only for OpenRouter)
        self.browse_models_row.set_visible(is_openrouter and not is_offline)

        # Show/hide OpenRouter-specific settings
        self.openrouter_group.set_visible(is_openrouter and not is_offline)

        # Update model placeholder
        default_model = self.DEFAULT_MODELS.get(provider_id, "")
        self.model_row.set_text(
            self.settings_manager.get("ai_assistant_model", "") or default_model
        )

        # Update subtitles based on provider
        if provider_id == "groq":
            self.model_row.set_title(_("Model Identifier"))
            self.api_key_row.set_title(_("Groq API Key"))
        elif provider_id == "gemini":
            self.model_row.set_title(_("Model Identifier"))
            self.api_key_row.set_title(_("Google AI Studio API Key"))
        elif provider_id == "openrouter":
            self.model_row.set_title(_("Model Identifier"))
            self.api_key_row.set_title(_("OpenRouter API Key"))
        elif provider_id == "local":
            self.model_row.set_title(_("Model Name"))
            self.api_key_row.set_title(_("API Key (if required)"))

    def _on_offline_changed(self, switch_row: Adw.SwitchRow, _param) -> None:
        """Handle offline mode toggle."""
        val = switch_row.get_active()
        self.settings_manager.set("ai_assistant_offline_mode", val)
        self.emit("setting-changed", "ai_assistant_offline_mode", val)
        if val:
            # Switch provider combo to local if current is cloud
            current_provider = self.settings_manager.get("ai_assistant_provider", "groq")
            if current_provider != "local":
                local_idx = self._get_provider_index("local")
                self.provider_row.set_selected(local_idx)
        self._update_ui_for_provider(self._get_selected_provider_id())

    def _on_provider_changed(self, combo_row, _param) -> None:
        """Handle provider selection change."""
        provider_id = self._get_selected_provider_id()
        self.settings_manager.set("ai_assistant_provider", provider_id)
        self._update_ui_for_provider(provider_id)
        self.emit("setting-changed", "ai_assistant_provider", provider_id)

    def _on_preload_changed(self, switch_row, _param) -> None:
        """Handle preload local model toggle."""
        val = switch_row.get_active()
        self.settings_manager.set("ai_preload_local_model", val)
        self.emit("setting-changed", "ai_preload_local_model", val)

    def _on_unload_changed(self, switch_row, _param) -> None:
        """Handle unload on exit toggle."""
        val = switch_row.get_active()
        self.settings_manager.set("ai_unload_on_exit", val)
        self.emit("setting-changed", "ai_unload_on_exit", val)

    def _on_base_url_changed(self, entry_row) -> None:
        """Handle base URL change."""
        url = entry_row.get_text().strip()
        self.settings_manager.set("ai_local_base_url", url)
        self.emit("setting-changed", "ai_local_base_url", url)

    def _on_api_key_changed(self, entry_row) -> None:
        """Handle API key change."""
        key = entry_row.get_text().strip()
        self.settings_manager.set("ai_assistant_api_key", key)
        self.emit("setting-changed", "ai_assistant_api_key", key)

    def _on_model_changed(self, entry_row) -> None:
        """Handle model change."""
        model = entry_row.get_text().strip()
        self.settings_manager.set("ai_assistant_model", model)
        self.emit("setting-changed", "ai_assistant_model", model)

    def _on_agent_setting_changed(self, key: str, value) -> None:
        """Handle Agent mode setting changes."""
        self.settings_manager.set(key, value)
        self.emit("setting-changed", key, value)
        if "api_key" in key:
            val_preview = f"{value[:4]}... (len={len(value)})" if value else "EMPTY"
            self.logger.info(f"[AIConfigDialog] Setting updated: {key} = {val_preview}")
        else:
            self.logger.info(f"[AIConfigDialog] Setting updated: {key} = {value}")

    def _on_routing_profile_changed(self, combo_row, _param) -> None:
        """Handle routing profile change."""
        idx = combo_row.get_selected()
        profiles = ["auto", "fast", "advanced"]
        if 0 <= idx < len(profiles):
            val = profiles[idx]
            self.settings_manager.set("ai_routing_profile", val)
            self.emit("setting-changed", "ai_routing_profile", val)
            self.logger.info(f"[AIConfigDialog] Routing profile changed to: {val}")

    def _on_fast_provider_changed(self, combo_row) -> None:
        """Handle fast provider selection change."""
        idx = combo_row.get_selected()
        if 0 <= idx < len(self.PROVIDERS):
            provider_id = self.PROVIDERS[idx][0]
            self.settings_manager.set("ai_fast_provider", provider_id)
            self.emit("setting-changed", "ai_fast_provider", provider_id)
            self.logger.info(f"[AIConfigDialog] Fast provider changed to: {provider_id}")

    def _on_adv_provider_changed(self, combo_row) -> None:
        """Handle advanced provider selection change."""
        idx = combo_row.get_selected()
        if 0 <= idx < len(self.PROVIDERS):
            provider_id = self.PROVIDERS[idx][0]
            self.settings_manager.set("ai_advanced_provider", provider_id)
            self.emit("setting-changed", "ai_advanced_provider", provider_id)
            self.logger.info(f"[AIConfigDialog] Advanced provider changed to: {provider_id}")

    def _on_scope_clicked(self, _button) -> None:
        """Open the agent scope configuration dialog."""
        from .agent_scope_dialog import AgentScopeDialog
        dialog = AgentScopeDialog(self, self.settings_manager)
        dialog.present()

    def _on_site_url_changed(self, entry_row) -> None:
        """Handle site URL change."""
        url = entry_row.get_text().strip()
        self.settings_manager.set("ai_openrouter_site_url", url)
        self.emit("setting-changed", "ai_openrouter_site_url", url)

    def _on_site_name_changed(self, entry_row) -> None:
        """Handle site name change."""
        name = entry_row.get_text().strip()
        self.settings_manager.set("ai_openrouter_site_name", name)
        self.emit("setting-changed", "ai_openrouter_site_name", name)

    def _on_enable_changed(self, switch_row, _param) -> None:
        """Handle enable/disable change."""
        enabled = switch_row.get_active()
        self.settings_manager.set("ai_assistant_enabled", enabled)
        self.emit("setting-changed", "ai_assistant_enabled", enabled)

    def _on_browse_models_clicked(self, button) -> None:
        """Open a searchable dialog to browse and select OpenRouter models."""
        api_key = self.settings_manager.get("ai_assistant_api_key", "").strip()
        if not api_key:
            self._show_toast(_("Please enter an API key first."))
            return

        # Open the model browser dialog
        dialog = OpenRouterModelBrowserDialog(self, api_key)
        dialog.connect("model-selected", self._on_model_browser_selected)
        dialog.present()

    def _on_model_browser_selected(self, dialog, model_id: str) -> None:
        """Handle model selection from the browser dialog."""
        self.model_row.set_text(model_id)
        dialog.close()

    def _show_toast(self, message: str) -> None:
        """Show a toast notification."""
        toast = Adw.Toast(title=message)
        self.add_toast(toast)

    def _on_test_gemini_key(self, button: Gtk.Button) -> None:
        key = self.key_gemini_row.get_text().strip()
        if not key:
            self._show_toast(_("Digite a chave do Gemini antes de testar."))
            return

        button.set_sensitive(False)
        button.set_label(_("Testando..."))

        def test_worker():
            from ...agent.providers.gemini import GeminiProvider
            models = GeminiProvider.discover_available_models(key, force_refresh=True)
            success = len(models) > 0

            def update_ui():
                button.set_sensitive(True)
                button.set_label(_("Testar"))
                if success:
                    model_list_str = ", ".join(models[:3])
                    self._show_toast(_("✅ Gemini conectado! Modelos: {models}").format(models=model_list_str))
                else:
                    self._show_toast(_("❌ Falha na autenticação do Google Gemini (Chave inválida)."))

            GLib.idle_add(update_ui)

        threading.Thread(target=test_worker, daemon=True).start()

    def _on_test_groq_key(self, button: Gtk.Button) -> None:
        key = self.key_groq_row.get_text().strip()
        if not key:
            self._show_toast(_("Digite a chave da Groq antes de testar."))
            return

        button.set_sensitive(False)
        button.set_label(_("Testando..."))

        def test_worker():
            from ...agent.providers.groq import GroqProvider
            prov = GroqProvider({"provider": "groq", "api_key": key})
            try:
                prov.complete([{"role": "user", "content": "Ping"}])
                success = True
            except Exception:
                success = False

            def update_ui():
                button.set_sensitive(True)
                button.set_label(_("Testar"))
                if success:
                    self._show_toast(_("✅ Conexão com a Groq estabelecida com sucesso!"))
                else:
                    self._show_toast(_("❌ Falha na autenticação com a Groq."))

            GLib.idle_add(update_ui)

        threading.Thread(target=test_worker, daemon=True).start()


class OpenRouterModelBrowserDialog(Adw.Window):
    """Searchable dialog for browsing and selecting OpenRouter models."""

    __gsignals__ = {
        "model-selected": (GObject.SignalFlags.RUN_FIRST, None, (str,)),
    }

    def __init__(self, parent_window, api_key: str):
        super().__init__(
            title=_("Select OpenRouter Model"),
            transient_for=parent_window,
            modal=True,
            default_width=800,
            default_height=600,
        )
        self.add_css_class("onyxsh-dialog")
        self.api_key = api_key
        self.logger = get_logger("onyxsh.ui.dialogs.model_browser")
        self._all_models: List[Tuple[str, str]] = []
        self._filtered_models: List[Tuple[str, str]] = []
        self._fetching = False

        self._setup_ui()
        self._fetch_models()

    def _setup_ui(self) -> None:
        """Set up the dialog UI."""
        toolbar_view = Adw.ToolbarView()
        self.set_content(toolbar_view)

        # Header bar
        header = Adw.HeaderBar()
        header.set_show_end_title_buttons(True)
        header.set_show_start_title_buttons(False)

        cancel_button = Gtk.Button(label=_("Cancel"))
        cancel_button.connect("clicked", lambda _: self.close())
        header.pack_start(cancel_button)

        toolbar_view.add_top_bar(header)

        # Main content
        main_box = Gtk.Box(
            orientation=Gtk.Orientation.VERTICAL,
            spacing=12,
            margin_top=12,
            margin_bottom=12,
            margin_start=12,
            margin_end=12,
        )
        toolbar_view.set_content(main_box)

        # Search entry
        self.search_entry = Gtk.SearchEntry(
            placeholder_text=_("Search models by name or ID...")
        )
        self.search_entry.connect("search-changed", self._on_search_changed)
        main_box.append(self.search_entry)

        # Status label / spinner
        self.status_box = Gtk.Box(
            orientation=Gtk.Orientation.HORIZONTAL,
            spacing=8,
            halign=Gtk.Align.CENTER,
            margin_top=8,
            margin_bottom=8,
        )
        self.spinner = Gtk.Spinner()
        self.status_label = Gtk.Label(label=_("Loading models..."))
        self.status_box.append(self.spinner)
        self.status_box.append(self.status_label)
        main_box.append(self.status_box)

        # Results count label
        self.count_label = Gtk.Label(
            label="",
            css_classes=["dim-label"],
            halign=Gtk.Align.START,
        )
        main_box.append(self.count_label)

        # Scrolled window with model list
        scrolled = Gtk.ScrolledWindow(vexpand=True)
        scrolled.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        scrolled.add_css_class("card")
        main_box.append(scrolled)

        # Model list
        self.list_box = Gtk.ListBox()
        self.list_box.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self.list_box.add_css_class("boxed-list")
        self.list_box.connect("row-activated", self._on_row_activated)
        scrolled.set_child(self.list_box)

    def _fetch_models(self) -> None:
        """Fetch models from OpenRouter API."""
        if self._fetching:
            return

        self._fetching = True
        self.spinner.start()
        self.status_box.set_visible(True)
        self.count_label.set_visible(False)

        thread = threading.Thread(
            target=self._fetch_models_thread,
            daemon=True,
        )
        thread.start()

    def _fetch_models_thread(self) -> None:
        """Fetch models in a background thread."""
        try:
            import requests

            headers = {
                "Authorization": f"Bearer {self.api_key}",
                "Content-Type": "application/json",
            }

            response = requests.get(
                "https://openrouter.ai/api/v1/models",
                headers=headers,
                timeout=30,
            )

            if response.status_code >= 400:
                GLib.idle_add(self._on_fetch_error, f"HTTP {response.status_code}")
                return

            data = response.json()
            models = data.get("data", [])

            # Extract model id and name
            model_list = []
            for model in models:
                model_id = model.get("id", "")
                model_name = model.get("name", model_id)
                if model_id:
                    model_list.append((model_id, model_name))

            # Sort by name
            model_list.sort(key=lambda x: x[1].lower())

            GLib.idle_add(self._on_fetch_success, model_list)

        except Exception as e:
            GLib.idle_add(self._on_fetch_error, str(e))

    def _on_fetch_success(self, models: List[Tuple[str, str]]) -> None:
        """Handle successful model fetch."""
        self._fetching = False
        self.spinner.stop()
        self.status_box.set_visible(False)

        self._all_models = models
        self._filtered_models = models
        self._update_model_list()

        # Focus search entry
        self.search_entry.grab_focus()

    def _on_fetch_error(self, error: str) -> None:
        """Handle fetch error."""
        self._fetching = False
        self.spinner.stop()
        self.status_label.set_text(
            _("Failed to load models: {error}").format(error=error)
        )
        self.status_label.add_css_class("error")

    def _on_search_changed(self, search_entry) -> None:
        """Filter models based on search text."""
        search_text = search_entry.get_text().lower().strip()

        if not search_text:
            self._filtered_models = self._all_models
        else:
            self._filtered_models = [
                (mid, name)
                for mid, name in self._all_models
                if search_text in mid.lower() or search_text in name.lower()
            ]

        self._update_model_list()

    def _update_model_list(self) -> None:
        """Update the model list display."""
        # Clear existing items
        while True:
            row = self.list_box.get_row_at_index(0)
            if row is None:
                break
            self.list_box.remove(row)

        # Update count label
        total = len(self._all_models)
        shown = len(self._filtered_models)
        if total == shown:
            self.count_label.set_text(_("{count} models available").format(count=total))
        else:
            self.count_label.set_text(
                _("Showing {shown} of {total} models").format(shown=shown, total=total)
            )
        self.count_label.set_visible(True)

        # Add filtered models (limit to first 100 for performance)
        for model_id, model_name in self._filtered_models[:100]:
            row = self._create_model_row(model_id, model_name)
            self.list_box.append(row)

        if len(self._filtered_models) > 100:
            hint_label = Gtk.Label(
                label=_("Showing first 100 results. Refine your search to see more."),
                css_classes=["dim-label"],
                margin_top=8,
                margin_bottom=8,
            )
            hint_row = Gtk.ListBoxRow(child=hint_label, selectable=False)
            self.list_box.append(hint_row)

    def _create_model_row(self, model_id: str, model_name: str) -> Gtk.ListBoxRow:
        """Create a row for a model."""
        row = Gtk.ListBoxRow()
        row.model_id = model_id  # Store ID for later retrieval

        box = Gtk.Box(
            orientation=Gtk.Orientation.VERTICAL,
            spacing=4,
            margin_top=8,
            margin_bottom=8,
            margin_start=12,
            margin_end=12,
        )

        # Model name (prominent)
        name_label = Gtk.Label(
            label=model_name,
            xalign=0,
            css_classes=["heading"],
            wrap=True,
            wrap_mode=2,  # WORD_CHAR
        )
        box.append(name_label)

        # Model ID (smaller, dim)
        id_label = Gtk.Label(
            label=model_id,
            xalign=0,
            css_classes=["dim-label", "caption"],
            selectable=True,
        )
        box.append(id_label)

        row.set_child(box)
        return row

    def _on_row_activated(self, list_box, row) -> None:
        """Handle row activation (selection)."""
        if hasattr(row, "model_id"):
            self.emit("model-selected", row.model_id)
