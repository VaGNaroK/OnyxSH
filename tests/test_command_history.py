# tests/test_command_history.py

import os
import tempfile
import time
import unittest
from pathlib import Path

from onyxsh.data.command_history_manager import (
    CommandHistoryItem,
    CommandHistoryManager,
)


class TestCommandHistoryManager(unittest.TestCase):
    """Test suite for CommandHistoryManager SQLite operations."""

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.db_path = Path(self.temp_dir.name) / "test_history.db"
        self.mgr = CommandHistoryManager(self.db_path)

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_record_and_search_command(self):
        """Test recording new commands and searching them."""
        item1 = self.mgr.record_command(
            "git status", cwd="/home/user/project", host="localhost", exit_code=0, duration_ms=120
        )
        self.assertIsNotNone(item1)
        self.assertEqual(item1.command, "git status")
        self.assertEqual(item1.execution_count, 1)

        # Record another command
        self.mgr.record_command(
            "docker compose up -d", cwd="/home/user/docker", host="server1", exit_code=0, duration_ms=2500
        )

        # Search all
        results = self.mgr.search_history()
        self.assertEqual(len(results), 2)

        # Search with query
        git_results = self.mgr.search_history(query="git")
        self.assertEqual(len(git_results), 1)
        self.assertEqual(git_results[0].command, "git status")

    def test_record_existing_command_increments_count(self):
        """Test that recording the exact same command in the same CWD increments execution count."""
        self.mgr.record_command("ls -la", cwd="/home/user", host="localhost", exit_code=0)
        time.sleep(0.01)
        updated = self.mgr.record_command("ls -la", cwd="/home/user", host="localhost", exit_code=0)

        self.assertIsNotNone(updated)
        self.assertEqual(updated.execution_count, 2)

        results = self.mgr.search_history(query="ls")
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0].execution_count, 2)

    def test_filters_cwd_and_host(self):
        """Test filtering by CWD and host."""
        self.mgr.record_command("npm test", cwd="/app/frontend", host="localhost")
        self.mgr.record_command("cargo build", cwd="/app/backend", host="localhost")
        self.mgr.record_command("htop", cwd="/app/frontend", host="remote-vps")

        # Filter by CWD
        frontend_cmds = self.mgr.search_history(cwd="/app/frontend")
        self.assertEqual(len(frontend_cmds), 2)

        # Filter by Host
        vps_cmds = self.mgr.search_history(host="remote-vps")
        self.assertEqual(len(vps_cmds), 1)
        self.assertEqual(vps_cmds[0].command, "htop")

    def test_toggle_pin_and_only_pinned_filter(self):
        """Test pinning favorite commands."""
        item = self.mgr.record_command("ssh deploy@production", cwd="/home/user")
        self.assertFalse(item.is_pinned)

        # Toggle pin
        is_pinned = self.mgr.toggle_pin(item.id)
        self.assertTrue(is_pinned)

        pinned_results = self.mgr.search_history(only_pinned=True)
        self.assertEqual(len(pinned_results), 1)
        self.assertEqual(pinned_results[0].command, "ssh deploy@production")

        # Toggle back
        is_pinned_again = self.mgr.toggle_pin(item.id)
        self.assertFalse(is_pinned_again)
        self.assertEqual(len(self.mgr.search_history(only_pinned=True)), 0)

    def test_delete_entry_and_clear_history(self):
        """Test deleting individual entries and clearing history."""
        cmd1 = self.mgr.record_command("command 1")
        cmd2 = self.mgr.record_command("command 2")
        cmd3 = self.mgr.record_command("failed command", exit_code=1)

        # Delete single entry
        self.mgr.delete_entry(cmd1.id)
        results = self.mgr.search_history()
        self.assertEqual(len(results), 2)
        self.assertNotIn("command 1", [r.command for r in results])

        # Clear failed
        self.mgr.clear_history(scope="failed")
        results = self.mgr.search_history()
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0].command, "command 2")

    def test_history_stats(self):
        """Test statistical aggregation of command history."""
        self.mgr.record_command("make build", cwd="/dir1")
        self.mgr.record_command("make build", cwd="/dir1")
        self.mgr.record_command("make test", cwd="/dir1")

        stats = self.mgr.get_stats()
        self.assertEqual(stats["total_entries"], 2)
        self.assertEqual(stats["top_commands"][0]["command"], "make build")
        self.assertEqual(stats["top_commands"][0]["count"], 2)


if __name__ == "__main__":
    unittest.main()
