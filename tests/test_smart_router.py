# tests/test_smart_router.py
"""Unit tests for the Smart Model Routing engine (Item 3.3)."""

import unittest
from unittest.mock import MagicMock

from onyxsh.agent.router import (
    RouteDecision,
    RoutingProfile,
    SmartRouter,
    TaskComplexity,
    TaskComplexityClassifier,
)


class TestSmartRouter(unittest.TestCase):
    """Test suite for SmartRouter and TaskComplexityClassifier."""

    def setUp(self):
        self.mock_settings = MagicMock()
        self._settings_store = {
            "ai_smart_routing_enabled": True,
            "ai_routing_profile": "auto",
            "ai_fast_provider": "groq",
            "ai_fast_model": "llama-3.1-8b-instant",
            "ai_advanced_provider": "gemini",
            "ai_advanced_model": "gemini-2.5-flash",
            "ai_assistant_provider": "gemini",
            "ai_assistant_model": "gemini-2.5-flash",
            "ai_assistant_api_key": "legacy-key-123",
            "ai_api_key_gemini": "gemini-key-456",
            "ai_api_key_groq": "groq-key-789",
            "ai_api_key_openrouter": "",
            "ai_local_base_url": "http://localhost:11434/v1",
            "ai_assistant_offline_mode": False,
        }
        self.mock_settings.get.side_effect = lambda k, default=None: self._settings_store.get(k, default)
        self.router = SmartRouter(self.mock_settings)

    def test_classify_simple_queries(self):
        """Test classification of simple queries and syntax questions."""
        self.assertEqual(TaskComplexityClassifier.classify("como listar arquivos no linux?"), TaskComplexity.SIMPLE)
        self.assertEqual(TaskComplexityClassifier.classify("qual o comando para ver a memória ram?"), TaskComplexity.SIMPLE)
        self.assertEqual(TaskComplexityClassifier.classify("how to check open ports?"), TaskComplexity.SIMPLE)
        self.assertEqual(TaskComplexityClassifier.classify("tar -h"), TaskComplexity.SIMPLE)
        self.assertEqual(TaskComplexityClassifier.classify("ls -la"), TaskComplexity.SIMPLE)

    def test_classify_complex_scripting_queries(self):
        """Test classification of complex scripts and multi-step tasks."""
        self.assertEqual(TaskComplexityClassifier.classify("crie um script bash de backup incremental"), TaskComplexity.COMPLEX)
        self.assertEqual(TaskComplexityClassifier.classify("escreva um código python para monitorar cpu"), TaskComplexity.COMPLEX)
        self.assertEqual(TaskComplexityClassifier.classify("passo a passo para instalar e configurar docker e postgresql"), TaskComplexity.COMPLEX)
        self.assertEqual(TaskComplexityClassifier.classify("build a deployment pipeline for web server"), TaskComplexity.COMPLEX)

    def test_classify_security_diagnostics_queries(self):
        """Test classification of security and self-healing diagnostic tasks."""
        self.assertEqual(TaskComplexityClassifier.classify("auditoria de segurança das regras de firewall ufw"), TaskComplexity.SECURITY)
        self.assertEqual(TaskComplexityClassifier.classify("analisar vulnerabilidade cve no pacote openssl"), TaskComplexity.SECURITY)
        self.assertEqual(TaskComplexityClassifier.classify("diagnosticar erro no serviço systemd nginx"), TaskComplexity.SECURITY)
        self.assertEqual(TaskComplexityClassifier.classify("self-healing do cluster"), TaskComplexity.SECURITY)

    def test_offline_mode_highest_precedence(self):
        """Test that offline mode strictly forces local provider and blocks cloud."""
        decision = self.router.resolve_route(
            prompt="Crie um script complexo de IA",
            is_offline_mode=True,
        )
        self.assertEqual(decision.provider, "local")
        self.assertEqual(decision.api_key, "")
        self.assertIn("Offline", decision.reason)

    def test_auto_route_simple_to_fast_provider(self):
        """Test that AUTO routing routes simple queries to fast provider (Groq)."""
        decision = self.router.resolve_route(prompt="como ver meu ip?")
        self.assertEqual(decision.provider, "groq")
        self.assertEqual(decision.model, "llama-3.1-8b-instant")
        self.assertEqual(decision.api_key, "groq-key-789")
        self.assertEqual(decision.complexity_inferred, TaskComplexity.SIMPLE)

    def test_auto_route_complex_to_advanced_provider(self):
        """Test that AUTO routing routes complex tasks to advanced provider (Gemini)."""
        decision = self.router.resolve_route(prompt="crie um script de automação com rotação de logs")
        self.assertEqual(decision.provider, "gemini")
        self.assertEqual(decision.model, "gemini-2.5-flash")
        self.assertEqual(decision.api_key, "gemini-key-456")
        self.assertEqual(decision.complexity_inferred, TaskComplexity.COMPLEX)

    def test_forced_fast_profile(self):
        """Test that forcing FAST profile overrides complex prompt classification."""
        decision = self.router.resolve_route(
            prompt="crie um script complexo",
            forced_profile=RoutingProfile.FAST,
        )
        self.assertEqual(decision.provider, "groq")
        self.assertEqual(decision.profile_used, RoutingProfile.FAST)

    def test_forced_advanced_profile(self):
        """Test that forcing ADVANCED profile overrides simple prompt classification."""
        decision = self.router.resolve_route(
            prompt="como listar arquivos?",
            forced_profile=RoutingProfile.ADVANCED,
        )
        self.assertEqual(decision.provider, "gemini")
        self.assertEqual(decision.profile_used, RoutingProfile.ADVANCED)

    def test_api_key_fallback_to_legacy(self):
        """Test fallback to legacy API key when provider-specific key is missing."""
        self._settings_store["ai_api_key_groq"] = ""
        decision = self.router.resolve_route(prompt="como listar arquivos?")
        self.assertEqual(decision.api_key, "legacy-key-123")

    def test_disabled_smart_routing_uses_legacy_config(self):
        """Test that disabling smart routing uses legacy provider/model settings."""
        self._settings_store["ai_smart_routing_enabled"] = False
        self._settings_store["ai_assistant_provider"] = "groq"
        self._settings_store["ai_assistant_model"] = "llama-3.1-8b-instant"

        decision = self.router.resolve_route(prompt="crie um plano completo")
        self.assertEqual(decision.provider, "groq")
        self.assertIn("desativado", decision.reason.lower())

    def test_route_decision_fallback_attributes(self):
        """Test that RouteDecision contains fallback metadata attributes."""
        decision = self.router.resolve_route(prompt="como listar arquivos?")
        self.assertFalse(decision.is_fallback)
        self.assertEqual(decision.fallback_reason, "")

        decision.is_fallback = True
        decision.fallback_reason = "Google Gemini timeout"
        self.assertTrue(decision.is_fallback)
        self.assertEqual(decision.fallback_reason, "Google Gemini timeout")

    def test_gemini_dynamic_model_discovery_candidates(self):
        """Test that GeminiProvider discovers models and filters candidates properly."""
        from onyxsh.agent.providers.gemini import GeminiProvider
        candidates = GeminiProvider.discover_available_models("")
        self.assertIsInstance(candidates, list)
        self.assertIn("gemini-2.5-flash", candidates)


if __name__ == "__main__":
    unittest.main()
