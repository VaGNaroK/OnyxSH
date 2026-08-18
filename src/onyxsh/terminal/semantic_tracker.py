# onyxsh/terminal/semantic_tracker.py

import threading
import time
from dataclasses import dataclass
from typing import Any, Callable, Dict, List, Optional
from weakref import WeakKeyDictionary

import gi

gi.require_version("Vte", "3.91")
from gi.repository import GLib, Vte

from ..utils.logger import get_logger


@dataclass
class SemanticCommand:
    """Represents a single command execution tracked via semantic prompt sequences."""

    command_id: str
    command_text: str = ""
    prompt_row: int = 0
    command_row: int = 0
    output_start_row: int = 0
    output_end_row: Optional[int] = None
    start_time: float = 0.0
    end_time: Optional[float] = None
    duration: Optional[float] = None
    exit_code: Optional[int] = None
    cwd: str = ""
    output_cache: Optional[str] = None

    @property
    def is_finished(self) -> bool:
        return self.exit_code is not None

    @property
    def is_success(self) -> bool:
        return self.exit_code == 0

    @property
    def formatted_duration(self) -> str:
        if self.duration is None:
            return ""
        if self.duration < 1.0:
            return f"{int(self.duration * 1000)}ms"
        if self.duration < 60.0:
            return f"{self.duration:.1f}s"
        mins = int(self.duration // 60)
        secs = self.duration % 60
        return f"{mins}m {secs:.0f}s"


class SemanticTerminalState:
    """Tracks semantic commands, cursor history and prompt positions for a single terminal."""

    def __init__(self, terminal: Vte.Terminal) -> None:
        self.terminal = terminal
        self.commands: List[SemanticCommand] = []
        self.current_command: Optional[SemanticCommand] = None
        self.current_prompt_row: int = 0
        self.current_command_row: int = 0
        self.prompt_rows: List[int] = []
        self._cmd_counter: int = 0

    def start_prompt(self, row: int) -> None:
        """OSC 133;A: Prompt start."""
        self.current_prompt_row = row
        if not self.prompt_rows or self.prompt_rows[-1] != row:
            self.prompt_rows.append(row)
            if len(self.prompt_rows) > 200:
                self.prompt_rows.pop(0)

    def start_command(self, row: int) -> None:
        """OSC 133;B: Command input start."""
        self.current_command_row = row

    def execute_command(self, row: int, command_text: str = "", cwd: str = "") -> None:
        """OSC 133;C: Execution / Output start."""
        self._cmd_counter += 1
        cmd = SemanticCommand(
            command_id=f"cmd_{self._cmd_counter}_{int(time.time() * 1000)}",
            command_text=command_text.strip(),
            prompt_row=self.current_prompt_row,
            command_row=self.current_command_row or self.current_prompt_row,
            output_start_row=row,
            start_time=time.time(),
            cwd=cwd,
        )
        self.current_command = cmd

    def finish_command(self, row: int, exit_code: int = 0) -> Optional[SemanticCommand]:
        """OSC 133;D: Command finished with exit code."""
        if not self.current_command:
            # If a command just finished less than 0.5s ago, ignore duplicate finish signal
            if self.commands and (time.time() - (self.commands[-1].end_time or 0.0)) < 0.5:
                return None

            # Create synthetic command if C was skipped
            self._cmd_counter += 1
            self.current_command = SemanticCommand(
                command_id=f"cmd_{self._cmd_counter}_{int(time.time() * 1000)}",
                prompt_row=self.current_prompt_row,
                command_row=self.current_command_row or self.current_prompt_row,
                output_start_row=self.current_prompt_row,
                start_time=time.time(),
            )

        cmd = self.current_command
        cmd.output_end_row = row
        cmd.end_time = time.time()
        cmd.duration = max(0.0, cmd.end_time - cmd.start_time)
        cmd.exit_code = exit_code
        self.commands.append(cmd)
        if len(self.commands) > 100:
            self.commands.pop(0)
        self.current_command = None
        return cmd


class SemanticTracker:
    """Central singleton tracker for OSC 133 semantic shell events across terminals."""

    def __init__(self) -> None:
        self.logger = get_logger("onyxsh.terminal.semantic_tracker")
        self._terminals: WeakKeyDictionary[Vte.Terminal, SemanticTerminalState] = (
            WeakKeyDictionary()
        )
        self._lock = threading.RLock()
        self.on_command_finished_callbacks: List[
            Callable[[Vte.Terminal, SemanticCommand], None]
        ] = []

    def get_or_create_state(self, terminal: Vte.Terminal) -> SemanticTerminalState:
        with self._lock:
            if terminal not in self._terminals:
                self._terminals[terminal] = SemanticTerminalState(terminal)
            return self._terminals[terminal]

    def untrack_terminal(self, terminal: Vte.Terminal) -> None:
        with self._lock:
            if terminal in self._terminals:
                del self._terminals[terminal]

    def register_command_finished_callback(
        self, callback: Callable[[Vte.Terminal, SemanticCommand], None]
    ) -> None:
        if callback not in self.on_command_finished_callbacks:
            self.on_command_finished_callbacks.append(callback)

    def handle_osc133(
        self, terminal: Vte.Terminal, action: str, param: str = ""
    ) -> Optional[SemanticCommand]:
        """Processes an OSC 133 semantic sequence received from a terminal."""
        try:
            state = self.get_or_create_state(terminal)
            col, row = terminal.get_cursor_position()

            self.logger.debug(
                f"handle_osc133 called: action={action}, param='{param}'"
            )
            if action == "A":
                # Prompt start
                state.start_prompt(row)
                return None

            elif action == "B":
                # Command start
                state.start_command(row)
                return None

            elif action == "C":
                # Command execution / output start
                cwd = ""
                if hasattr(terminal, "get_current_directory_uri"):
                    try:
                        uri = terminal.get_current_directory_uri()
                        if isinstance(uri, str) and uri.startswith("file://"):
                            from urllib.parse import unquote, urlparse

                            parsed = urlparse(uri)
                            cwd = unquote(parsed.path)
                    except Exception:
                        cwd = ""
                state.execute_command(row, command_text="", cwd=cwd)
                self.logger.debug(
                    f"Command execution STARTED (C): start_time={state.current_command.start_time if state.current_command else None}"
                )
                return state.current_command

            elif action == "D":
                # Command finished with exit code
                exit_code = 0
                if param:
                    try:
                        exit_code = int(param.split(";")[0])
                    except (ValueError, IndexError):
                        exit_code = 0

                finished_cmd = state.finish_command(row, exit_code)
                if finished_cmd:
                    self.logger.debug(
                        f"Command FINISHED (D): text='{finished_cmd.command_text}', duration={finished_cmd.duration:.2f}s, exit_code={finished_cmd.exit_code}"
                    )
                    self._notify_finished(terminal, finished_cmd)
                return finished_cmd

        except Exception as e:
            self.logger.error(f"Error processing OSC 133 action {action}: {e}")
        return None

    def _notify_finished(self, terminal: Vte.Terminal, cmd: SemanticCommand) -> None:
        for cb in self.on_command_finished_callbacks:
            try:
                GLib.idle_add(cb, terminal, cmd)
            except Exception as e:
                self.logger.error(f"Error invoking semantic callback: {e}")

    def get_last_command(self, terminal: Vte.Terminal) -> Optional[SemanticCommand]:
        with self._lock:
            state = self._terminals.get(terminal)
            if state and state.commands:
                return state.commands[-1]
            return None

    def get_last_command_text(self, terminal: Vte.Terminal) -> str:
        """Extracts the command text string of the last command."""
        cmd = self.get_last_command(terminal)
        if not cmd:
            return ""
        if cmd.command_text:
            return cmd.command_text
        return self.extract_command_text(terminal, cmd)

    def extract_command_text(
        self, terminal: Vte.Terminal, cmd: SemanticCommand
    ) -> str:
        """Extracts command line text from terminal buffer or command object."""
        if cmd.command_text:
            return cmd.command_text
        try:
            prompt_row = cmd.prompt_row
            start_row = (
                cmd.output_start_row
                if cmd.output_start_row is not None
                else prompt_row
            )
            col_count = (
                terminal.get_column_count()
                if hasattr(terminal, "get_column_count")
                else 200
            )
            if hasattr(terminal, "get_text_range_format"):
                res = terminal.get_text_range_format(
                    Vte.Format.TEXT, prompt_row, 0, start_row + 1, col_count
                )
                text = res[0] if isinstance(res, tuple) else (res or "")
            else:
                text = ""

            lines = [l for l in text.splitlines() if l.strip()]
            if lines:
                last_line = lines[-1]
                for prompt_char in ("$ ", "# ", "❯ ", "➜ ", "> "):
                    if prompt_char in last_line:
                        last_line = last_line.split(prompt_char, 1)[-1]
                cmd.command_text = last_line.strip()
                return cmd.command_text
        except Exception as e:
            self.logger.debug(f"Failed to extract command text from buffer: {e}")
        return ""

    def get_last_output_text(self, terminal: Vte.Terminal) -> Optional[str]:
        """Extracts the exact text output generated by the last executed command."""
        cmd = self.get_last_command(terminal)
        if not cmd:
            return None
        return self.extract_command_output(terminal, cmd)

    def extract_command_output(
        self, terminal: Vte.Terminal, cmd: SemanticCommand
    ) -> str:
        """Uses VTE text range extraction to return output between command start and finish."""
        if cmd.output_cache is not None:
            return cmd.output_cache

        try:
            start_row = cmd.output_start_row
            end_row = cmd.output_end_row if cmd.output_end_row is not None else start_row
            if end_row < start_row:
                end_row = start_row

            col_count = (
                terminal.get_column_count()
                if hasattr(terminal, "get_column_count")
                else 200
            )

            # Extract full text from start_row to end_row using modern VTE Format API
            if hasattr(terminal, "get_text_range_format"):
                text_range = terminal.get_text_range_format(
                    Vte.Format.TEXT, start_row, 0, end_row + 1, col_count
                )
            else:
                text_range = terminal.get_text_range(
                    start_row, 0, end_row + 1, col_count, None, None
                )

            if isinstance(text_range, tuple):
                output_text = text_range[0] or ""
            elif isinstance(text_range, str):
                output_text = text_range
            else:
                output_text = ""

            cmd.output_cache = output_text.strip()
            return cmd.output_cache
        except Exception as e:
            self.logger.debug(f"Failed to extract text range from terminal: {e}")
            return ""

    def get_previous_prompt_row(
        self, terminal: Vte.Terminal, current_row: int
    ) -> Optional[int]:
        """Finds the nearest prompt row before the current row."""
        with self._lock:
            state = self._terminals.get(terminal)
            if not state or not state.prompt_rows:
                return None

            for row in reversed(state.prompt_rows):
                if row < current_row:
                    return row
            return state.prompt_rows[0] if state.prompt_rows else None

    def get_next_prompt_row(
        self, terminal: Vte.Terminal, current_row: int
    ) -> Optional[int]:
        """Finds the nearest prompt row after the current row."""
        with self._lock:
            state = self._terminals.get(terminal)
            if not state or not state.prompt_rows:
                return None

            for row in state.prompt_rows:
                if row > current_row:
                    return row
            return state.prompt_rows[-1] if state.prompt_rows else None


# Global semantic tracker singleton
_global_semantic_tracker: Optional[SemanticTracker] = None
_tracker_init_lock = threading.Lock()


def get_semantic_tracker() -> SemanticTracker:
    global _global_semantic_tracker
    with _tracker_init_lock:
        if _global_semantic_tracker is None:
            _global_semantic_tracker = SemanticTracker()
        return _global_semantic_tracker
