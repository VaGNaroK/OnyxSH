"""Tests for FSTools (staging, unified diff, apply, and trash)."""

import json
import tempfile
import unittest
from pathlib import Path
from zashterminal.agent.fs_tools import FSTools
from zashterminal.agent.path_guard import PathGuard


class TestFSTools(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.tmpdir = tempfile.TemporaryDirectory()
        self.tmp_path = Path(self.tmpdir.name)
        self.pg = PathGuard(allowed_roots=[self.tmpdir.name])
        self.tools = FSTools(path_guard=self.pg)

    def tearDown(self):
        self.tmpdir.cleanup()

    async def test_fs_read_and_search(self):
        test_file = self.tmp_path / "hello.txt"
        test_file.write_text("Line 1\nZashterminal Agent\nLine 3", encoding="utf-8")

        res = await self.tools.read_file(str(test_file))
        self.assertTrue(res.success)
        self.assertIn("Zashterminal Agent", res.stdout)

        search_res = await self.tools.search_text(path=str(self.tmp_path), pattern="Agent")
        self.assertTrue(search_res.success)
        matches = json.loads(search_res.stdout)
        self.assertGreaterEqual(len(matches), 1)

    async def test_fs_propose_edit_and_apply(self):
        target_file = self.tmp_path / "config.ini"
        target_file.write_text("timeout = 30\nport = 8080\n", encoding="utf-8")

        prop_res = await self.tools.propose_edit(
            path=str(target_file),
            new_content="timeout = 60\nport = 8080\n",
            plan_id="plan_test_1",
        )
        self.assertTrue(prop_res.success)
        self.assertIn("-timeout = 30", prop_res.stdout)
        self.assertIn("+timeout = 60", prop_res.stdout)

        # Locate staged file
        staging_dir = Path.home() / ".cache" / "zashterminal" / "ai-staging" / "plan_test_1"
        staged_files = list(staging_dir.glob("*.staged"))
        self.assertGreaterEqual(len(staged_files), 1)
        staged_path = str(staged_files[0])

        apply_res = await self.tools.apply_staged(
            target_path=str(target_file),
            staged_path=staged_path,
            backup=True,
        )
        self.assertTrue(apply_res.success)
        self.assertEqual(target_file.read_text(encoding="utf-8"), "timeout = 60\nport = 8080\n")

    async def test_fs_move_to_trash(self):
        to_trash = self.tmp_path / "obsolete.log"
        to_trash.write_text("old logs", encoding="utf-8")

        res = await self.tools.move_to_trash(str(to_trash))
        self.assertTrue(res.success)
        self.assertFalse(to_trash.exists())


if __name__ == "__main__":
    unittest.main()
