"""Tests for secret redactor (redact_secrets)."""

import unittest
from onyxsh.agent.redactor import redact_secrets


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

    def test_redact_gitlab_personal_access_token(self):
        """BUG-002: GitLab PAT tokens (glpat-...) must be detected."""
        token_sample = "glpat-" + "mockTokenForTesting12345"
        text = f"export GITLAB_TOKEN={token_sample}"
        redacted, count = redact_secrets(text)
        self.assertGreaterEqual(count, 1)
        self.assertNotIn("glpat-", redacted)
        self.assertIn("[REDACTED_GITLAB_TOKEN]", redacted)

    def test_redact_slack_bot_token(self):
        """BUG-002: Slack bot tokens (xoxb-...) must be detected."""
        token_sample = "xoxb-" + "mock" + "-dummy" + "-tokenForTesting"
        text = f"SLACK_BOT_TOKEN={token_sample}"
        redacted, count = redact_secrets(text)
        self.assertGreaterEqual(count, 1)
        self.assertNotIn("xoxb-", redacted)
        self.assertIn("[REDACTED_SLACK_TOKEN]", redacted)

    def test_redact_slack_user_token(self):
        """BUG-002: Slack user tokens (xoxp-...) must be detected."""
        token_sample = "xoxp-" + "mock" + "-dummy" + "-tokenForTesting"
        text = f"token: {token_sample}"
        redacted, count = redact_secrets(text)
        self.assertGreaterEqual(count, 1)
        self.assertNotIn("xoxp-", redacted)

    def test_redact_vercel_token(self):
        """BUG-002: Vercel deploy tokens must be detected."""
        token_sample = "vercel_" + "mockDeployTokenForTesting123"
        text = f'VERCEL_TOKEN="{token_sample}"'
        redacted, count = redact_secrets(text)
        self.assertGreaterEqual(count, 1)
        self.assertNotIn(token_sample, redacted)
        self.assertIn("[REDACTED_VERCEL_TOKEN]", redacted)

    def test_redact_vault_token(self):
        """BUG-002: HashiCorp Vault tokens (hvs.*) must be detected."""
        token_sample = "hvs." + "mockVaultTokenForTesting12345"
        text = f"X-Vault-Token: {token_sample}"
        redacted, count = redact_secrets(text)
        self.assertGreaterEqual(count, 1)
        self.assertNotIn("hvs.", redacted)
        self.assertIn("[REDACTED_VAULT_TOKEN]", redacted)

    def test_redact_multiple_secrets_in_same_text(self):
        """Ensures multiple distinct secrets in a single text are all redacted."""
        t1 = "glpat-" + "mockTokenMulti12345678"
        t2 = "xoxb-" + "mock" + "-dummy" + "-multiTest"
        text = (
            "AWS_KEY=AKIAIOSFODNN7EXAMPLE\n"
            f"GITLAB_TOKEN={t1}\n"
            f"SLACK={t2}\n"
        )
        redacted, count = redact_secrets(text)
        self.assertGreaterEqual(count, 3)
        self.assertNotIn("AKIAIOSFODNN7EXAMPLE", redacted)
        self.assertNotIn("glpat-", redacted)
        self.assertNotIn("xoxb-", redacted)

    def test_safe_text_untouched(self):
        """Normal text without secrets should pass through unchanged."""
        text = "This is a normal log message with no secrets."
        redacted, count = redact_secrets(text)
        self.assertEqual(count, 0)
        self.assertEqual(redacted, text)


if __name__ == "__main__":
    unittest.main()

