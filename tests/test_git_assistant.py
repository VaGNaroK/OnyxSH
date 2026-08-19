# tests/test_git_assistant.py
"""Unit tests for Git Assistant, Git Utilities, and Secret Leak Auditor."""

import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

from onyxsh.terminal.git_assistant import GitCommitAssistant
from onyxsh.utils.git_utils import (
    audit_diff_for_secrets,
    clean_file_uri_to_path,
    get_git_diff,
    get_git_status,
    get_repo_root,
    is_git_repository,
)


class TestGitAssistant(unittest.TestCase):
    def setUp(self):
        self.mock_ai = MagicMock()
        self.assistant = GitCommitAssistant(self.mock_ai)

    def test_clean_file_uri_to_path(self):
        """Semantic tracking URI with query strings and localhost prefixes should be cleaned."""
        uri_with_query = "file://localhost/home/vagnarok/CryoMint-RAM?__zt_sem__=D_0_bHM=__A"
        clean = clean_file_uri_to_path(uri_with_query)
        self.assertEqual(clean, "/home/vagnarok/CryoMint-RAM")

        uri_with_spaces = "file:///home/vagnarok/my%20project/repo?param=1"
        clean_spaces = clean_file_uri_to_path(uri_with_spaces)
        self.assertEqual(clean_spaces, "/home/vagnarok/my project/repo")

    def test_is_git_repository_current_dir(self):
        """Current working repo root should be identified as a Git repository."""
        repo_root = Path(__file__).resolve().parent.parent
        self.assertTrue(is_git_repository(repo_root))
        detected_root = get_repo_root(repo_root)
        self.assertIsNotNone(detected_root)
        self.assertEqual(detected_root.resolve(), repo_root.resolve())

    def test_audit_diff_no_secrets(self):
        """Clean diff with normal code should return 0 findings."""
        clean_diff = """
diff --git a/src/main.py b/src/main.py
index 1234567..89abcdef 100644
--- a/src/main.py
+++ b/src/main.py
@@ -10,3 +10,4 @@ def calculate(a, b):
+    result = a + b
+    return result
"""
        findings = audit_diff_for_secrets(clean_diff)
        self.assertEqual(len(findings), 0)

    def test_audit_diff_detects_api_key(self):
        """Diff containing leaked API key should be flagged."""
        dirty_diff = """
diff --git a/config.py b/config.py
--- a/config.py
+++ b/config.py
@@ -1,2 +1,3 @@
+GROQ_API_KEY = "gsk_1234567890abcdef1234567890abcdef"
"""
        findings = audit_diff_for_secrets(dirty_diff)
        self.assertEqual(len(findings), 1)
        self.assertEqual(findings[0]["file"], "config.py")
        self.assertIn("API_KEY", findings[0]["type"])

    def test_audit_diff_detects_aws_key(self):
        """Diff containing leaked AWS Access Key should be flagged."""
        dirty_diff = """
diff --git a/deploy.sh b/deploy.sh
--- a/deploy.sh
+++ b/deploy.sh
@@ -1,2 +1,3 @@
+export AWS_ACCESS_KEY_ID="AKIAIOSFODNN7EXAMPLE"
"""
        findings = audit_diff_for_secrets(dirty_diff)
        self.assertEqual(len(findings), 1)
        self.assertEqual(findings[0]["file"], "deploy.sh")
        self.assertIn("AWS_KEY", findings[0]["type"])

    def test_audit_diff_detects_private_key(self):
        """Diff containing private key block header should be flagged."""
        dirty_diff = """
diff --git a/id_rsa b/id_rsa
--- a/id_rsa
+++ b/id_rsa
@@ -0,0 +1,3 @@
+-----BEGIN RSA PRIVATE KEY-----
+MIIEowIBAAKCAQEA0...
+-----END RSA PRIVATE KEY-----
"""
        findings = audit_diff_for_secrets(dirty_diff)
        self.assertTrue(len(findings) >= 1)
        self.assertEqual(findings[0]["file"], "id_rsa")
        self.assertIn("PRIVATE_KEY", findings[0]["type"])

    def test_clean_commit_output_strips_markdown_and_noise(self):
        """Output should strip code blocks and noise."""
        raw_markdown = """```
feat(terminal): add git assistant integration

- Implement Conventional Commits generation
- Add pre-commit secret leak detection
```"""
        cleaned = self.assistant._clean_commit_output(raw_markdown)
        self.assertTrue(cleaned.startswith("feat(terminal):"))
        self.assertNotIn("```", cleaned)

    def test_clean_commit_output_json_unwrapping(self):
        """JSON output from structured models should be properly unwrapped."""
        json_output = '{"reply": "fix(ui): resolve GTK markup warning\\n\\n- Escape ampersand character"}'
        cleaned = self.assistant._clean_commit_output(json_output)
        self.assertEqual(
            cleaned,
            "fix(ui): resolve GTK markup warning\n\n- Escape ampersand character",
        )

    def test_build_prompt_styles(self):
        """Prompts for conventional, short, and detailed styles should contain appropriate rules."""
        conv_prompt = self.assistant._build_prompt(
            "main", "staged", "unstaged", "diff", "conventional", "Portuguese"
        )
        self.assertIn("Conventional Commits", conv_prompt)
        self.assertIn("Portuguese", conv_prompt)

        short_prompt = self.assistant._build_prompt(
            "main", "staged", "unstaged", "diff", "short", "English"
        )
        self.assertIn("single concise line", short_prompt)

        detailed_prompt = self.assistant._build_prompt(
            "main", "staged", "unstaged", "diff", "detailed", "English"
        )
        self.assertIn("Detailed bullet points", detailed_prompt)


if __name__ == "__main__":
    unittest.main()
