"""Tests for shell tools executor (run_argv)."""

import unittest
from onyxsh.agent.shell_tools import run_argv


class TestShellTools(unittest.IsolatedAsyncioTestCase):
    async def test_run_argv_valid_command(self):
        res = await run_argv(["echo", "hello onyxsh"])
        self.assertTrue(res.success)
        self.assertIn("hello onyxsh", res.stdout)
        self.assertEqual(res.returncode, 0)

    async def test_run_argv_rejects_empty_or_non_list(self):
        res = await run_argv([])
        self.assertFalse(res.success)

        res2 = await run_argv("echo hello")  # type: ignore
        self.assertFalse(res2.success)

    async def test_run_argv_blocks_sh_c_bypass(self):
        res = await run_argv(["sh", "-c", "echo bypassed"])
        self.assertFalse(res.success)
        self.assertIn("não são permitidas", res.error)

        res2 = await run_argv(["bash", "-c", "echo bypassed"])
        self.assertFalse(res2.success)
        self.assertIn("não são permitidas", res2.error)

    async def test_run_argv_timeout_enforcement(self):
        res = await run_argv(["sleep", "5"], timeout_seconds=1)
        self.assertFalse(res.success)
        self.assertIn("timed out", res.error.lower())

    async def test_run_argv_redacts_output_secrets(self):
        res = await run_argv(["echo", "SECRET=sk-1234567890abcdef1234567890abcdef"])
        self.assertTrue(res.success)
        self.assertNotIn("sk-1234567890abcdef", res.stdout)
        self.assertIn("[REDACTED", res.stdout)


if __name__ == "__main__":
    unittest.main()
