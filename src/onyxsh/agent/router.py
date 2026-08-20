# onyxsh/agent/router.py
"""
Smart Model Routing engine for OnyxSH AI Assistant.

Directs requests to appropriate AI models (fast/local vs advanced/cloud)
based on task complexity, selected profile, and offline/privacy policies.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
import re
from typing import Any, Dict, Optional, Tuple

from ..utils.logger import get_logger
from ..utils.translation_utils import _


class TaskComplexity(str, Enum):
    """Estimated complexity of an AI prompt / user request."""
    SIMPLE = "simple"         # Quick syntax, flag explanations, single CLI command
    COMPLEX = "complex"       # Multi-step plans, long scripts, automations, migration
    SECURITY = "security"     # Security audits, vulnerability checks, self-healing diagnostics


class RoutingProfile(str, Enum):
    """Active routing profile requested by user or system."""
    AUTO = "auto"             # Automatically classify complexity and route
    FAST = "fast"             # Force fast / low-latency model (Groq / Ollama local)
    ADVANCED = "advanced"     # Force advanced reasoning model (Gemini / Claude / OpenRouter)
    SECURITY = "security"     # Force security / deep analysis model


@dataclass
class RouteDecision:
    """Result of a smart routing evaluation."""
    provider: str
    model: str
    api_key: str
    profile_used: RoutingProfile
    complexity_inferred: TaskComplexity
    reason: str
    base_url: Optional[str] = None
    openrouter_site_url: str = ""
    openrouter_site_name: str = ""
    is_fallback: bool = False
    fallback_reason: str = ""


class TaskComplexityClassifier:
    """Heuristic classifier for user prompt complexity."""

    # Keywords and regex patterns indicative of complex multi-step/scripting tasks
    _COMPLEX_PATTERNS = [
        r"\b(crie|criar|escreva|escrever|gere|gerar|desenvolva)\s+.*\b(script|código|automação|pipeline|playbook|dockerfile)\b",
        r"\b(pipeline|playbook|dockerfile|workflow|automação|automacao)\b",
        r"\b(passo\s+a\s+passo|etapas|plano|planejar|roteiro|tutorial)\b",
        r"\b(instalar?\s+e\s+configurar?|subir\s+serviço|deploy|implantar?|migrar?|orquestrar?)\b",
        r"\b(configurar?|configurações?|configuracao|administrar?|gerenciar?)\s+.*\b(ip|rede|interface|servidor|dns|dhcp|ssh|vpn|proxy|nginx|apache|banco|postgres|mysql|storage|disco|partição|particao|usuario|usuarios|grupos|permissoes)\b",
        r"\b(backup\s+e\s+restaur|rotinas?\s+de\s+backup|alta\s+disponibilidade|cluster)\b",
        r"\b(otimizar?\s+desempenho|analisar?\s+gargalo|troubleshoot|diagnosticar?\s+profund)\b",
        r"\b(create|write|generate|build)\s+.*\b(script|code|automation|pipeline|playbook|dockerfile)\b",
        r"\b(configure|configuration|setup|manage|administer)\s+.*\b(ip|network|interface|server|dns|dhcp|ssh|vpn|proxy|nginx|apache|database|postgres|mysql|storage|disk|partition|users|permissions)\b",
        r"\b(step\s+by\s+step|multi-step|plan|planning|workflow|architecture)\b",
        r"\b(install\s+and\s+configure|setup\s+service|deploy|deployment|migration|orchestrate)\b",
        r"\b(backup\s+and\s+restore|high\s+availability|cluster)\b",
    ]

    # Keywords and regex patterns indicative of security audits or diagnostics
    _SECURITY_PATTERNS = [
        r"\b(segurança|vulnerabilidade|cve|auditoria|auditar|hardening|firewall|iptables|ufw)\b",
        r"\b(permissoes?\s+indevidas|rootkit|malware|analise\s+de\s+logs?\s+de\s+erro)\b",
        r"\b(self-healing|auto-fix|corrigir?\s+falha|reparar?\s+serviço|diagnosticar?\s+erro)\b",
        r"\b(security|vulnerability|cve|audit|hardening|firewall|exploit|penetration)\b",
        r"\b(self-healing|auto-fix|repair\s+service|diagnose\s+failure)\b",
    ]

    # Quick syntax / lookup patterns (definitely simple)
    _SIMPLE_PATTERNS = [
        r"^(como\s+(ver|listar?|checar?|mostrar?|exibir?|consultar?)|qual\s+(o\s+)?comando|qual\s+a\s+flag|o\s+que\s+significa)\b",
        r"^(how\s+to\s+(check|list|show|view|find)|what\s+is\s+the\s+command|what\s+does\s+flag)\b",
        r"^(\w+\s+--help|\w+\s+-h|man\s+\w+|sintaxe\s+de\s+\w+)$",
    ]

    @classmethod
    def classify(cls, prompt: str) -> TaskComplexity:
        """Classify a prompt into TaskComplexity."""
        if not prompt or not prompt.strip():
            return TaskComplexity.SIMPLE

        text = prompt.strip().lower()

        # Check security / diagnostic patterns first
        for pat in cls._SECURITY_PATTERNS:
            if re.search(pat, text, re.IGNORECASE):
                return TaskComplexity.SECURITY

        # Check complex scripting and multi-step patterns
        for pat in cls._COMPLEX_PATTERNS:
            if re.search(pat, text, re.IGNORECASE):
                return TaskComplexity.COMPLEX

        # Check explicit simple patterns
        for pat in cls._SIMPLE_PATTERNS:
            if re.search(pat, text, re.IGNORECASE):
                return TaskComplexity.SIMPLE

        # Length / line count heuristics
        lines = [l for l in text.splitlines() if l.strip()]
        if len(lines) >= 3 or len(text) > 300:
            return TaskComplexity.COMPLEX

        # Default to simple for short concise prompts
        return TaskComplexity.SIMPLE


class SmartRouter:
    """
    Intelligent router that resolves the ideal AI provider, model, and API key
    based on settings, active routing profile, task complexity, and privacy mode.
    """

    DEFAULT_FAST_PROVIDER = "groq"
    DEFAULT_FAST_MODEL = "llama-3.1-8b-instant"

    DEFAULT_ADVANCED_PROVIDER = "gemini"
    DEFAULT_ADVANCED_MODEL = "gemini-2.5-flash"

    DEFAULT_LOCAL_MODEL = "llama3.2"

    def __init__(self, settings_manager: Any) -> None:
        self.settings_manager = settings_manager
        self.logger = get_logger("onyxsh.agent.router")

    def _get_api_key_for_provider(self, provider: str) -> str:
        """Retrieve the configured API key for a specific provider with fallback."""
        key_name = f"ai_api_key_{provider.lower()}"
        key = self.settings_manager.get(key_name, "").strip()
        if key:
            return key
        # Fallback to legacy single api key if provider matches default or empty
        return self.settings_manager.get("ai_assistant_api_key", "").strip()

    def resolve_route(
        self,
        prompt: str = "",
        forced_profile: Optional[RoutingProfile] = None,
        is_offline_mode: Optional[bool] = None,
    ) -> RouteDecision:
        """
        Calculates the routing decision for a given prompt and current settings.

        Args:
            prompt: The user request text.
            forced_profile: Optional manual profile override (e.g. from UI selector).
            is_offline_mode: If None, checks settings_manager.
        """
        if is_offline_mode is None:
            is_offline_mode = bool(
                self.settings_manager.get("ai_assistant_offline_mode", False)
            )

        # 1. Strictly Offline Privacy Policy (Highest Precedence)
        if is_offline_mode:
            local_base_url = self.settings_manager.get(
                "ai_local_base_url", "http://localhost:11434/v1"
            ).strip()
            raw_model = self.settings_manager.get("ai_assistant_model", "").strip()
            if not raw_model or raw_model in {
                self.DEFAULT_FAST_MODEL,
                self.DEFAULT_ADVANCED_MODEL,
                "openrouter/polaris-alpha",
            }:
                local_model = self.DEFAULT_LOCAL_MODEL
            else:
                local_model = raw_model

            decision = RouteDecision(
                provider="local",
                model=local_model,
                api_key="",
                profile_used=RoutingProfile.FAST,
                complexity_inferred=TaskComplexity.SIMPLE,
                reason=_("Modo Estritamente Offline ativo (execução restrita a modelo local Ollama/LM Studio)."),
                base_url=local_base_url,
            )
            self._log_decision(decision, prompt, is_offline_mode)
            return decision

        # 2. Check if Smart Routing is globally enabled
        smart_routing_enabled = bool(
            self.settings_manager.get("ai_smart_routing_enabled", True)
        )

        # 3. Determine active profile
        if forced_profile:
            active_profile = forced_profile
        else:
            profile_str = self.settings_manager.get(
                "ai_routing_profile", "auto"
            ).lower()
            try:
                active_profile = RoutingProfile(profile_str)
            except ValueError:
                active_profile = RoutingProfile.AUTO

        # 4. If Smart Routing is disabled and no manual forced profile, use legacy configuration
        if not smart_routing_enabled and active_profile == RoutingProfile.AUTO:
            legacy_provider = self.settings_manager.get(
                "ai_assistant_provider", "gemini"
            ).strip()
            legacy_model = self.settings_manager.get(
                "ai_assistant_model", ""
            ).strip()
            legacy_key = self.settings_manager.get(
                "ai_assistant_api_key", ""
            ).strip()
            decision = RouteDecision(
                provider=legacy_provider,
                model=legacy_model or self.DEFAULT_ADVANCED_MODEL,
                api_key=legacy_key,
                profile_used=RoutingProfile.AUTO,
                complexity_inferred=TaskComplexity.SIMPLE,
                reason=_("Roteamento Inteligente desativado: utilizando provedor padrão das configurações."),
                base_url=self.settings_manager.get("ai_local_base_url", "http://localhost:11434/v1"),
                openrouter_site_url=self.settings_manager.get("ai_openrouter_site_url", ""),
                openrouter_site_name=self.settings_manager.get("ai_openrouter_site_name", ""),
            )
            self._log_decision(decision, prompt, is_offline_mode)
            return decision

        # 5. Classify task complexity
        complexity = TaskComplexityClassifier.classify(prompt)

        # 6. Route resolution based on profile
        if active_profile == RoutingProfile.FAST or (
            active_profile == RoutingProfile.AUTO and complexity == TaskComplexity.SIMPLE
        ):
            fast_provider = self.settings_manager.get(
                "ai_fast_provider", self.DEFAULT_FAST_PROVIDER
            ).strip()
            fast_model = self.settings_manager.get(
                "ai_fast_model", self.DEFAULT_FAST_MODEL
            ).strip()
            api_key = self._get_api_key_for_provider(fast_provider)
            reason = (
                _("Roteado para perfil Rápido (consulta simples / baixa latência).")
                if active_profile == RoutingProfile.AUTO
                else _("Perfil Rápido selecionado manualmente.")
            )
            decision = RouteDecision(
                provider=fast_provider,
                model=fast_model or self.DEFAULT_FAST_MODEL,
                api_key=api_key,
                profile_used=RoutingProfile.FAST,
                complexity_inferred=complexity,
                reason=reason,
                base_url=self.settings_manager.get("ai_local_base_url", "http://localhost:11434/v1"),
                openrouter_site_url=self.settings_manager.get("ai_openrouter_site_url", ""),
                openrouter_site_name=self.settings_manager.get("ai_openrouter_site_name", ""),
            )
            self._log_decision(decision, prompt, is_offline_mode)
            return decision

        # Advanced or Security profile
        adv_provider = self.settings_manager.get(
            "ai_advanced_provider", self.DEFAULT_ADVANCED_PROVIDER
        ).strip()
        adv_model = self.settings_manager.get(
            "ai_advanced_model", self.DEFAULT_ADVANCED_MODEL
        ).strip()
        api_key = self._get_api_key_for_provider(adv_provider)

        if complexity == TaskComplexity.SECURITY:
            reason = _("Roteado para modelo avançado com foco em segurança/diagnóstico.")
        elif active_profile == RoutingProfile.AUTO:
            reason = _("Roteado para modelo avançado (tarefa complexa / planejamento multi-passo).")
        else:
            reason = _("Perfil Avançado selecionado manualmente.")

        decision = RouteDecision(
            provider=adv_provider,
            model=adv_model or self.DEFAULT_ADVANCED_MODEL,
            api_key=api_key,
            profile_used=RoutingProfile.ADVANCED,
            complexity_inferred=complexity,
            reason=reason,
            base_url=self.settings_manager.get("ai_local_base_url", "http://localhost:11434/v1"),
            openrouter_site_url=self.settings_manager.get("ai_openrouter_site_url", ""),
            openrouter_site_name=self.settings_manager.get("ai_openrouter_site_name", ""),
        )
        self._log_decision(decision, prompt, is_offline_mode)
        return decision

    def _log_decision(self, decision: RouteDecision, prompt: str, offline: bool) -> None:
        key_preview = f"{decision.api_key[:4]}... (len={len(decision.api_key)})" if decision.api_key else "NONE"
        self.logger.info(
            f"[SmartRouter] Route calculated: prompt='{prompt[:50]}...' | "
            f"provider='{decision.provider}', model='{decision.model}', key={key_preview} | "
            f"profile={decision.profile_used.value}, complexity={decision.complexity_inferred.value}, offline={offline} | "
            f"reason='{decision.reason}'"
        )
