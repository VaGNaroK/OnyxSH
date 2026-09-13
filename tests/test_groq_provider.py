# tests/test_groq_provider.py
"""Unit tests for GroqProvider."""

import unittest
from unittest.mock import MagicMock, patch

from onyxsh.agent.providers.groq import GroqProvider


class TestGroqProvider(unittest.TestCase):
    """Test suite for GroqProvider."""

    def setUp(self):
        GroqProvider._discovered_models_cache.clear()

    def test_default_model(self):
        """Verify default model is openai/gpt-oss-120b."""
        provider = GroqProvider({"provider": "groq", "api_key": "dummy_key"})
        self.assertEqual(provider.model, "openai/gpt-oss-120b")

    def test_custom_model(self):
        """Verify custom model override."""
        provider = GroqProvider({"provider": "groq", "api_key": "dummy_key", "model": "qwen/qwen3.8-27b"})
        self.assertEqual(provider.model, "qwen/qwen3.8-27b")

    def test_missing_api_key_raises(self):
        """Calling complete without api_key raises ValueError."""
        provider = GroqProvider({"provider": "groq", "api_key": ""})
        with self.assertRaises(ValueError):
            provider.complete([{"role": "user", "content": "ping"}])

    def test_discover_available_models_fallback(self):
        """When empty API key is passed, default candidates are returned."""
        models = GroqProvider.discover_available_models("")
        self.assertIn("openai/gpt-oss-120b", models)
        self.assertIn("qwen/qwen3.8-27b", models)

    @patch("requests.get")
    def test_discover_available_models_from_api(self, mock_get):
        """Mock successful API response filtering whisper/guard models."""
        mock_resp = MagicMock()
        mock_resp.status_code = 200
        mock_resp.json.return_value = {
            "data": [
                {"id": "openai/gpt-oss-120b"},
                {"id": "qwen/qwen3.8-27b"},
                {"id": "whisper-large-v3"},
                {"id": "meta-llama/llama-prompt-guard-2-22m"},
            ]
        }
        mock_get.return_value = mock_resp

        models = GroqProvider.discover_available_models("test_key", force_refresh=True)
        self.assertEqual(models, ["openai/gpt-oss-120b", "qwen/qwen3.8-27b"])

    @patch("requests.post")
    def test_complete_without_json_does_not_set_response_format(self, mock_post):
        """Groq rejects response_format json_object if messages lack 'json'."""
        mock_resp = MagicMock()
        mock_resp.status_code = 200
        mock_resp.json.return_value = {
            "choices": [{"message": {"role": "assistant", "content": "PONG"}}]
        }
        mock_post.return_value = mock_resp

        provider = GroqProvider({"provider": "groq", "api_key": "test_key"})
        result = provider.complete([{"role": "user", "content": "Ping"}])

        self.assertEqual(result, "PONG")
        payload = mock_post.call_args[1]["json"]
        self.assertNotIn("response_format", payload)

    @patch("requests.post")
    def test_complete_with_json_sets_response_format(self, mock_post):
        """Groq accepts response_format json_object when messages contain 'json'."""
        mock_resp = MagicMock()
        mock_resp.status_code = 200
        mock_resp.json.return_value = {
            "choices": [{"message": {"role": "assistant", "content": "{\"status\": \"ok\"}"}}]
        }
        mock_post.return_value = mock_resp

        provider = GroqProvider({"provider": "groq", "api_key": "test_key"})
        result = provider.complete([{"role": "user", "content": "Respond in JSON format"}])

        self.assertEqual(result, "{\"status\": \"ok\"}")
        payload = mock_post.call_args[1]["json"]
        self.assertEqual(payload.get("response_format"), {"type": "json_object"})

    @patch("requests.post")
    def test_complete_stream(self, mock_post):
        """Test streaming chunk aggregation and callbacks."""
        mock_resp = MagicMock()
        mock_resp.status_code = 200
        mock_resp.iter_lines.return_value = [
            b'data: {"choices": [{"delta": {"content": "Hello"}}]}',
            b'data: {"choices": [{"delta": {"content": " world"}}]}',
            b'data: [DONE]',
        ]
        mock_post.return_value = mock_resp

        chunks = []
        finished_flags = []

        def callback(chunk, is_done):
            if chunk:
                chunks.append(chunk)
            finished_flags.append(is_done)

        provider = GroqProvider({"provider": "groq", "api_key": "test_key"})
        full_text = provider.complete_stream(
            [{"role": "user", "content": "Hi"}],
            callback=callback
        )

        self.assertEqual(full_text, "Hello world")
        self.assertEqual(chunks, ["Hello", " world"])
        self.assertTrue(finished_flags[-1])


if __name__ == "__main__":
    unittest.main()
