# tests/test_ai_offline_mode.py
"""Unit tests for AI Strictly Offline Mode (Privacy Guard)."""

import unittest
from unittest.mock import MagicMock

from onyxsh.settings.manager import SettingsManager
from onyxsh.terminal.ai_assistant import TerminalAiAssistant


class TestAiOfflineMode(unittest.TestCase):
    def setUp(self):
        self.settings = SettingsManager()
        # Ensure clean state
        self.settings.set("ai_assistant_enabled", True)
        self.settings.set("ai_assistant_offline_mode", False)
        self.settings.set("ai_assistant_provider", "groq")
        self.settings.set("ai_assistant_model", "llama-3.1-8b-instant")
        self.settings.set("ai_assistant_api_key", "gsk_test_key_12345")
        self.settings.set("ai_local_base_url", "http://localhost:11434/v1")

        self.mock_window = MagicMock()
        self.mock_terminal_manager = MagicMock()
        self.assistant = TerminalAiAssistant(
            self.mock_window, self.settings, self.mock_terminal_manager
        )

    def test_default_offline_mode_state(self):
        """Offline mode should be disabled by default."""
        self.assertFalse(self.assistant.is_offline_mode())

    def test_toggle_offline_mode(self):
        """Setting offline mode should update settings and assistant state."""
        self.assistant.set_offline_mode(True)
        self.assertTrue(self.assistant.is_offline_mode())
        self.assertTrue(self.settings.get("ai_assistant_offline_mode"))

        self.assistant.set_offline_mode(False)
        self.assertFalse(self.assistant.is_offline_mode())
        self.assertFalse(self.settings.get("ai_assistant_offline_mode"))

    def test_load_configuration_in_online_mode(self):
        """Online mode should load configured cloud provider."""
        self.assistant.set_offline_mode(False)
        config = self.assistant._load_configuration()
        self.assertEqual(config["provider"], "groq")
        self.assertEqual(config["model"], "llama-3.1-8b-instant")

    def test_load_configuration_in_offline_mode(self):
        """Offline mode should strictly force provider to 'local' and default local model."""
        self.assistant.set_offline_mode(True)
        config = self.assistant._load_configuration()
        self.assertEqual(config["provider"], "local")
        self.assertEqual(config["model"], self.assistant.DEFAULT_LOCAL_MODEL)

    def test_missing_configuration_in_offline_mode(self):
        """In offline mode, missing cloud API key should not be reported as missing."""
        self.settings.set("ai_assistant_provider", "gemini")
        self.settings.set("ai_assistant_api_key", "")  # No cloud key

        # When online, api_key is missing
        self.assistant.set_offline_mode(False)
        self.assertIn("api_key", self.assistant.missing_configuration())

        # When offline, provider is effectively local, so no cloud api_key needed
        self.assistant.set_offline_mode(True)
        self.assertNotIn("api_key", self.assistant.missing_configuration())

    def test_blocked_cloud_dispatch_in_offline_mode(self):
        """Direct attempts to dispatch to cloud providers in offline mode must be blocked."""
        self.assistant.set_offline_mode(True)

        cloud_config = {
            "provider": "gemini",
            "model": "gemini-2.5-flash",
            "api_key": "dummy",
        }
        messages = [{"role": "user", "content": "hello"}]

        with self.assertRaises(RuntimeError):
            self.assistant._perform_request(cloud_config, messages)

        with self.assertRaises(RuntimeError):
            self.assistant._perform_streaming_request(cloud_config, messages)


if __name__ == "__main__":
    unittest.main()
