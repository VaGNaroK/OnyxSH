"""Unit tests for LLM provider VRAM lifecycle management (preload, unload, is_loaded)."""

import unittest
from unittest.mock import MagicMock, patch

from zashterminal.agent.providers.base import LLMProvider
from zashterminal.agent.providers.ollama import OllamaProvider
from zashterminal.terminal.ai_assistant import TerminalAiAssistant


class DummyBaseProvider(LLMProvider):
    def complete(self, messages, tools_schema=None):
        return "ok"


class TestLLMLifecycle(unittest.TestCase):
    def test_base_provider_lifecycle(self):
        provider = DummyBaseProvider({"model": "test-model"})
        self.assertTrue(provider.preload())
        self.assertTrue(provider.unload())
        self.assertTrue(provider.is_loaded())

    def test_ollama_native_base_url(self):
        provider = OllamaProvider({"local_base_url": "http://localhost:11434/v1"})
        self.assertEqual(provider._get_native_base_url(), "http://localhost:11434")

        provider2 = OllamaProvider({"local_base_url": "http://localhost:11434"})
        self.assertEqual(provider2._get_native_base_url(), "http://localhost:11434")

    @patch("requests.post")
    def test_ollama_preload_success(self, mock_post):
        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_post.return_value = mock_response

        provider = OllamaProvider({
            "model": "qwen2.5-coder:3b",
            "local_base_url": "http://localhost:11434/v1",
        })

        result = provider.preload()
        self.assertTrue(result)
        mock_post.assert_called_once()
        args, kwargs = mock_post.call_args
        self.assertEqual(args[0], "http://localhost:11434/api/generate")
        self.assertEqual(kwargs["json"], {"model": "qwen2.5-coder:3b", "keep_alive": -1})

    @patch("requests.post")
    def test_ollama_preload_failure_resilient(self, mock_post):
        mock_post.side_effect = Exception("Connection refused")

        provider = OllamaProvider({
            "model": "qwen2.5-coder:3b",
            "local_base_url": "http://localhost:11434/v1",
        })

        result = provider.preload()
        self.assertFalse(result)

    @patch("requests.post")
    def test_ollama_unload_success(self, mock_post):
        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_post.return_value = mock_response

        provider = OllamaProvider({
            "model": "qwen2.5-coder:3b",
            "local_base_url": "http://localhost:11434/v1",
        })

        result = provider.unload()
        self.assertTrue(result)
        mock_post.assert_called_once()
        args, kwargs = mock_post.call_args
        self.assertEqual(args[0], "http://localhost:11434/api/generate")
        self.assertEqual(kwargs["json"], {"model": "qwen2.5-coder:3b", "keep_alive": 0})

    @patch("requests.get")
    def test_ollama_is_loaded(self, mock_get):
        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.json.return_value = {
            "models": [{"name": "qwen2.5-coder:3b", "model": "qwen2.5-coder:3b", "size_vram": 2159374499}]
        }
        mock_get.return_value = mock_response

        provider = OllamaProvider({
            "model": "qwen2.5-coder:3b",
            "local_base_url": "http://localhost:11434/v1",
        })
        self.assertTrue(provider.is_loaded())

        # Test when not in models
        mock_response.json.return_value = {"models": [{"name": "other-model:7b"}]}
        self.assertFalse(provider.is_loaded())

    @patch("zashterminal.agent.providers.ollama.OllamaProvider.preload")
    def test_ai_assistant_preload_async(self, mock_preload):
        mock_settings = MagicMock()
        mock_settings.get.side_effect = lambda k, default=None: {
            "ai_assistant_enabled": True,
            "ai_assistant_provider": "local",
            "ai_preload_local_model": True,
            "ai_assistant_model": "qwen2.5-coder:3b",
            "ai_local_base_url": "http://localhost:11434/v1",
            "ai_assistant_api_key": "",
        }.get(k, default)

        assistant = TerminalAiAssistant(None, mock_settings, None)
        assistant._preload_model_worker()
        mock_preload.assert_called_once()

    @patch("zashterminal.agent.providers.ollama.OllamaProvider.unload")
    def test_ai_assistant_unload(self, mock_unload):
        mock_settings = MagicMock()
        mock_settings.get.side_effect = lambda k, default=None: {
            "ai_assistant_enabled": True,
            "ai_assistant_provider": "local",
            "ai_unload_on_exit": True,
            "ai_assistant_model": "qwen2.5-coder:3b",
            "ai_local_base_url": "http://localhost:11434/v1",
            "ai_assistant_api_key": "",
        }.get(k, default)

        assistant = TerminalAiAssistant(None, mock_settings, None)
        assistant.unload_model()
        mock_unload.assert_called_once()

    @patch("zashterminal.terminal.ai_assistant.TerminalAiAssistant.unload_model")
    @patch("zashterminal.terminal.ai_assistant.TerminalAiAssistant.preload_model_async")
    def test_handle_setting_changed_lifecycle(self, mock_preload, mock_unload):
        mock_settings = MagicMock()
        mock_settings.get.side_effect = lambda k, default=None: {
            "ai_assistant_enabled": True,
            "ai_assistant_provider": "local",
            "ai_preload_local_model": True,
            "ai_assistant_model": "qwen2.5-coder:3b",
            "ai_local_base_url": "http://localhost:11434/v1",
            "ai_assistant_api_key": "",
        }.get(k, default)

        assistant = TerminalAiAssistant(None, mock_settings, None)

        # 1. Disable AI -> unloads
        assistant.handle_setting_changed("ai_assistant_enabled", True, False)
        mock_unload.assert_called_once()

        # 2. Enable AI -> preloads
        assistant.handle_setting_changed("ai_assistant_enabled", False, True)
        mock_preload.assert_called_once()

        # 3. Change model -> unloads old, preloads new
        mock_unload.reset_mock()
        mock_preload.reset_mock()
        assistant.handle_setting_changed("ai_assistant_model", "qwen2.5-coder:3b", "qwen2.5-coder:7b")
        mock_unload.assert_called_once()
        mock_preload.assert_called_once()


if __name__ == "__main__":
    unittest.main()
