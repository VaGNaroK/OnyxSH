"""Tests for secret redactor (redact_secrets)."""

import unittest
from zashterminal.agent.redactor import redact_secrets


class TestRedactor(unittest.TestCase):
    def test_redact_private_key_block(self):
        text = (
            "-----BEGIN RSA PRIVATE KEY-----\n"
            "MIIEowIBAAKCAQEA0Yk4...\n"
            "-----END RSA PRIVATE KEY-----\n"
        )
        redacted, count = redact_secrets(text)
        self.assertGreaterEqual(count, 1)
        self.assertNotIn("-----BEGIN RSA PRIVATE KEY-----", redacted)
        self.assertIn("[REDACTED_PRIVATE_KEY]", redacted)

    def test_redact_aws_access_key(self):
        text = "Deploy using AKIAIOSFODNN7EXAMPLE and secret."
        redacted, count = redact_secrets(text)
        self.assertGreaterEqual(count, 1)
        self.assertNotIn("AKIAIOSFODNN7EXAMPLE", redacted)
        self.assertIn("[REDACTED_AWS_KEY]", redacted)

    def test_redact_bearer_token(self):
        text = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.e30.t-IDcBQIrHGror"
        redacted, count = redact_secrets(text)
        self.assertGreaterEqual(count, 1)
        self.assertNotIn("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9", redacted)
        self.assertIn("[REDACTED_BEARER_TOKEN]", redacted)

    def test_redact_api_key_patterns(self):
        text = "openai_api_key = 'sk-1234567890abcdef1234567890abcdef'\n"
        redacted, count = redact_secrets(text)
        self.assertGreaterEqual(count, 1)
        self.assertNotIn("sk-1234567890abcdef1234567890abcdef", redacted)
        self.assertIn("[REDACTED_API_KEY]", redacted)

    def test_redact_database_url(self):
        text = "DATABASE_URL=postgres://user:SuperSecretPassword123@localhost:5432/mydb"
        redacted, count = redact_secrets(text)
        self.assertGreaterEqual(count, 1)
        self.assertNotIn("SuperSecretPassword123", redacted)
        self.assertIn("[REDACTED]", redacted)


if __name__ == "__main__":
    unittest.main()
