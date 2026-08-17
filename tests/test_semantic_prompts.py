# tests/test_semantic_prompts.py

import time
import unittest
from unittest.mock import MagicMock

import gi

gi.require_version("Vte", "3.91")
from gi.repository import Vte

from onyxsh.terminal.semantic_tracker import (
    SemanticCommand,
    SemanticTerminalState,
    SemanticTracker,
    get_semantic_tracker,
)


class TestSemanticPrompts(unittest.TestCase):
    """Test suite for OSC 133 Semantic Prompts and Command Tracking."""

    def setUp(self):
        self.tracker = SemanticTracker()
        self.mock_terminal = MagicMock()
        self.mock_terminal.get_cursor_position.return_value = (0, 10)
        self.mock_terminal.get_text_range.return_value = (
            "total 8\n-rw-r--r-- 1 user user 100 Aug 17 00:00 file.txt\n",
            [],
        )
        self.mock_terminal.get_text_range_format.return_value = (
            "total 8\n-rw-r--r-- 1 user user 100 Aug 17 00:00 file.txt\n",
            50,
        )

    def test_semantic_command_duration_formatting(self):
        """Test human-readable formatting of command durations."""
        cmd = SemanticCommand(command_id="cmd_1")
        self.assertEqual(cmd.formatted_duration, "")

        cmd.duration = 0.250
        self.assertEqual(cmd.formatted_duration, "250ms")

        cmd.duration = 1.4
        self.assertEqual(cmd.formatted_duration, "1.4s")

        cmd.duration = 65.0
        self.assertEqual(cmd.formatted_duration, "1m 5s")

    def test_semantic_terminal_state_lifecycle(self):
        """Test full prompt -> command -> execute -> finish lifecycle."""
        state = SemanticTerminalState(self.mock_terminal)

        # OSC 133;A: Prompt starts at row 5
        state.start_prompt(5)
        self.assertEqual(state.current_prompt_row, 5)
        self.assertIn(5, state.prompt_rows)

        # OSC 133;B: Command input starts at row 5
        state.start_command(5)
        self.assertEqual(state.current_command_row, 5)

        # OSC 133;C: Execution starts at row 6
        state.execute_command(6, command_text="ls -la", cwd="/home/user")
        self.assertIsNotNone(state.current_command)
        self.assertEqual(state.current_command.command_text, "ls -la")
        self.assertEqual(state.current_command.output_start_row, 6)

        # OSC 133;D: Finished at row 8 with exit code 0
        finished = state.finish_command(8, exit_code=0)
        self.assertIsNotNone(finished)
        self.assertTrue(finished.is_finished)
        self.assertTrue(finished.is_success)
        self.assertEqual(finished.exit_code, 0)
        self.assertEqual(finished.output_end_row, 8)
        self.assertGreaterEqual(finished.duration, 0.0)
        self.assertEqual(len(state.commands), 1)

    def test_tracker_handle_osc133_flow(self):
        """Test tracker handling OSC 133 actions via handle_osc133."""
        # 1. Prompt Start (A)
        self.mock_terminal.get_cursor_position.return_value = (0, 10)
        self.tracker.handle_osc133(self.mock_terminal, "A")

        # 2. Command Start (B)
        self.mock_terminal.get_cursor_position.return_value = (15, 10)
        self.tracker.handle_osc133(self.mock_terminal, "B")

        # 3. Output Start (C)
        self.mock_terminal.get_cursor_position.return_value = (0, 11)
        self.tracker.handle_osc133(self.mock_terminal, "C")

        # 4. Finish (D;0)
        self.mock_terminal.get_cursor_position.return_value = (0, 15)
        cmd = self.tracker.handle_osc133(self.mock_terminal, "D", "0")

        self.assertIsNotNone(cmd)
        self.assertEqual(cmd.exit_code, 0)
        self.assertEqual(cmd.prompt_row, 10)
        self.assertEqual(cmd.output_start_row, 11)
        self.assertEqual(cmd.output_end_row, 15)

    def test_tracker_error_exit_code(self):
        """Test tracker capturing error exit codes (e.g. 127)."""
        self.mock_terminal.get_cursor_position.return_value = (0, 20)
        self.tracker.handle_osc133(self.mock_terminal, "A")
        self.tracker.handle_osc133(self.mock_terminal, "C")
        cmd = self.tracker.handle_osc133(self.mock_terminal, "D", "127")

        self.assertIsNotNone(cmd)
        self.assertFalse(cmd.is_success)
        self.assertEqual(cmd.exit_code, 127)

    def test_prompt_navigation_lookups(self):
        """Test previous and next prompt row navigation."""
        state = self.tracker.get_or_create_state(self.mock_terminal)
        state.start_prompt(10)
        state.start_prompt(25)
        state.start_prompt(50)

        # Before row 30, previous should be 25
        prev_row = self.tracker.get_previous_prompt_row(self.mock_terminal, 30)
        self.assertEqual(prev_row, 25)

        # Before row 20, previous should be 10
        prev_row = self.tracker.get_previous_prompt_row(self.mock_terminal, 20)
        self.assertEqual(prev_row, 10)

        # After row 15, next should be 25
        next_row = self.tracker.get_next_prompt_row(self.mock_terminal, 15)
        self.assertEqual(next_row, 25)

        # After row 30, next should be 50
        next_row = self.tracker.get_next_prompt_row(self.mock_terminal, 30)
        self.assertEqual(next_row, 50)

    def test_extract_command_output(self):
        """Test extraction of command output from VTE."""
        self.mock_terminal.get_cursor_position.return_value = (0, 5)
        self.tracker.handle_osc133(self.mock_terminal, "A")
        self.tracker.handle_osc133(self.mock_terminal, "C")
        cmd = self.tracker.handle_osc133(self.mock_terminal, "D", "0")

        output = self.tracker.extract_command_output(self.mock_terminal, cmd)
        self.assertIn("file.txt", output)
        self.assertEqual(cmd.output_cache, output)


if __name__ == "__main__":
    unittest.main()
