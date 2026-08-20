# tests/test_diagnostics.py
"""
Unit tests for the Secure System Diagnostics Generator (SystemDiagnostics).
"""

import json
import unittest
from unittest.mock import MagicMock, patch

from onyxsh.utils.diagnostics import SystemDiagnostics


class TestSystemDiagnostics(unittest.TestCase):
    """Test suite for SystemDiagnostics."""

    def test_sanitize_gemini_api_key(self):
        raw = "Error connecting with AIzaSyD1234567890abcdef1234567890abcde to endpoint"
        sanitized = SystemDiagnostics.sanitize_text(raw)
        self.assertNotIn("AIzaSyD1234567890abcdef1234567890abcde", sanitized)
        self.assertIn("[REDACTED_GEMINI_KEY]", sanitized)

    def test_sanitize_groq_api_key(self):
        raw = "Groq key gsk_1234567890abcdef1234567890abcdef1234567890abcdef12 failed"
        sanitized = SystemDiagnostics.sanitize_text(raw)
        self.assertNotIn("gsk_1234567890abcdef", sanitized)
        self.assertIn("[REDACTED_GROQ_KEY]", sanitized)

    def test_sanitize_openai_openrouter_key(self):
        raw = "Using Authorization header with sk-or-v1-abcdef1234567890abcdef123456"
        sanitized = SystemDiagnostics.sanitize_text(raw)
        self.assertNotIn("sk-or-v1-abcdef1234567890abcdef123456", sanitized)
        self.assertIn("[REDACTED_API_KEY]", sanitized)

    def test_sanitize_github_and_aws_tokens(self):
        raw_gh = "GitHub token ghp_1234567890abcdefghijklmnopqrstuvwxyz12 is valid"
        raw_aws = "AWS key AKIAIOSFODNN7EXAMPLE used"
        self.assertIn("[REDACTED_GITHUB_TOKEN]", SystemDiagnostics.sanitize_text(raw_gh))
        self.assertIn("[REDACTED_AWS_KEY]", SystemDiagnostics.sanitize_text(raw_aws))

    def test_sanitize_ip_addresses(self):
        raw_ip = "Connected to host 192.168.1.50 on port 22 and 10.0.0.1"
        sanitized = SystemDiagnostics.sanitize_text(raw_ip)
        self.assertNotIn("192.168.1.50", sanitized)
        self.assertNotIn("10.0.0.1", sanitized)
        self.assertIn("[REDACTED_IP]", sanitized)

    def test_sanitize_email_addresses(self):
        raw_email = "Contact developer at john.doe@example.com for info"
        sanitized = SystemDiagnostics.sanitize_text(raw_email)
        self.assertNotIn("john.doe@example.com", sanitized)
        self.assertIn("[REDACTED_EMAIL]", sanitized)

    def test_sanitize_user_paths(self):
        raw_path = "Log file created at /home/johndoe/project/test.log"
        sanitized = SystemDiagnostics.sanitize_text(raw_path)
        self.assertNotIn("/home/johndoe/", sanitized)
        self.assertIn("/home/<user>/", sanitized)

    @patch("requests.get")
    def test_check_ollama_status_online(self, mock_get):
        # Mock version response
        mock_resp_version = MagicMock()
        mock_resp_version.status_code = 200
        mock_resp_version.json.return_value = {"version": "0.32.5"}

        # Mock tags response
        mock_resp_tags = MagicMock()
        mock_resp_tags.status_code = 200
        mock_resp_tags.json.return_value = {"models": [{"name": "qwen2.5-coder:7b"}, {"name": "llama3.2"}]}

        mock_get.side_effect = [mock_resp_version, mock_resp_tags]

        res = SystemDiagnostics.check_ollama_status("http://localhost:11434/v1")
        self.assertTrue(res["online"])
        self.assertEqual(res["version"], "0.32.5")
        self.assertIn("qwen2.5-coder:7b", res["models"])

    @patch("requests.get")
    def test_check_ollama_status_offline(self, mock_get):
        mock_get.side_effect = Exception("Connection refused")
        res = SystemDiagnostics.check_ollama_status("http://localhost:11434/v1")
        self.assertFalse(res["online"])
        self.assertIn("Connection refused", res["error"])

    def test_collect_system_data(self):
        mock_settings = MagicMock()
        mock_settings.get.side_effect = lambda k, d=None: {
            "ai_assistant_enabled": True,
            "ai_assistant_offline_mode": False,
            "ai_smart_routing_enabled": True,
            "ai_routing_profile": "auto",
            "ai_fast_provider": "local",
            "ai_fast_model": "qwen2.5-coder:7b",
            "ai_advanced_provider": "gemini",
            "ai_advanced_model": "gemini-2.5-flash",
            "ai_api_key_gemini": "test-key",
            "ai_api_key_groq": "",
            "ai_local_base_url": "http://localhost:11434/v1",
        }.get(k, d)

        with patch.object(SystemDiagnostics, "check_ollama_status", return_value={"online": True, "version": "0.32.5", "models": ["qwen"]}):
            data = SystemDiagnostics.collect_system_data(settings_manager=mock_settings, log_lines=10)

        self.assertIn("app", data)
        self.assertIn("system", data)
        self.assertIn("stack", data)
        self.assertIn("ai", data)
        self.assertTrue(data["ai"]["providers_status"]["gemini"])
        self.assertFalse(data["ai"]["providers_status"]["groq"])

        # Test Markdown generation
        md = SystemDiagnostics.generate_markdown_report(data)
        self.assertIn("# 🔍 OnyxSH System Diagnostic Report", md)
        self.assertIn("Google Gemini (Configurada)", md)
        self.assertIn("Kernel:", md)

        # Test JSON generation
        json_str = SystemDiagnostics.generate_json_report(data)
        parsed = json.loads(json_str)
        self.assertEqual(parsed["app"]["name"], "OnyxSH")

    def test_run_cli_output(self):
        args = MagicMock()
        args.lines = 10
        args.json = True
        args.output = None

        with patch("builtins.print") as mock_print:
            ret = SystemDiagnostics.run_cli(args)
            self.assertEqual(ret, 0)
            mock_print.assert_called()


if __name__ == "__main__":
    unittest.main()
