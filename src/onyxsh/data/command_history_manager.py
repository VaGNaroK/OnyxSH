# onyxsh/data/command_history_manager.py

"""
Structured and enriched command history persistence using SQLite.
Stores executed shell commands with rich metadata: PWD, host/session,
exit code, duration, timestamps, execution counts and favorite (pin) status.
"""

from __future__ import annotations

import os
import re
import sqlite3
import threading
import time
import uuid
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

from ..settings.config import get_config_paths
from ..utils.logger import get_logger


@dataclass
class CommandHistoryItem:
    """Represents a single command entry in the enriched history database."""

    id: str
    command: str
    cwd: str = ""
    host: str = "localhost"
    session_name: str = ""
    exit_code: Optional[int] = None
    duration_ms: int = 0
    timestamp: float = 0.0
    is_pinned: bool = False
    execution_count: int = 1
    last_executed: float = 0.0

    @property
    def formatted_relative_time(self) -> str:
        """Returns a human-readable relative time string (e.g. '5s ago', '2m ago')."""
        diff = max(0.0, time.time() - (self.last_executed or self.timestamp))
        if diff < 60:
            return f"{int(diff)}s"
        elif diff < 3600:
            return f"{int(diff // 60)}m"
        elif diff < 86400:
            return f"{int(diff // 3600)}h"
        elif diff < 604800:
            return f"{int(diff // 86400)}d"
        else:
            return f"{int(diff // 604800)}w"

    @property
    def formatted_duration(self) -> str:
        """Returns human readable duration string (e.g. '1.5s', '250ms')."""
        if not self.duration_ms or self.duration_ms <= 0:
            return ""
        if self.duration_ms < 1000:
            return f"{self.duration_ms}ms"
        seconds = self.duration_ms / 1000.0
        if seconds < 60:
            return f"{seconds:.1f}s"
        mins = int(seconds // 60)
        secs = int(seconds % 60)
        return f"{mins}m {secs}s"

    @property
    def display_cwd(self) -> str:
        """Returns a simplified CWD replacing the home directory with '~'."""
        if not self.cwd:
            return ""
        clean_cwd = self.cwd
        if clean_cwd.startswith("localhost/"):
            clean_cwd = clean_cwd[9:]
        elif clean_cwd.startswith("localhost"):
            clean_cwd = clean_cwd[len("localhost") :]
        home = str(Path.home())
        if clean_cwd == home:
            return "~"
        if clean_cwd.startswith(home + "/"):
            return "~" + clean_cwd[len(home) :]
        return clean_cwd


class CommandHistoryManager:
    """Thread-safe SQLite manager for enriched command history."""

    _instance: Optional[CommandHistoryManager] = None
    _singleton_lock = threading.Lock()

    def __init__(self, db_path: Optional[Path] = None) -> None:
        self.logger = get_logger("onyxsh.data.command_history_manager")
        self._config_paths = get_config_paths()
        if db_path is not None:
            self._db_path = Path(db_path)
        else:
            self._db_path = self._config_paths.CONFIG_DIR / "command_history.db"

        self._lock = threading.RLock()
        self._init_db()

    @classmethod
    def get_instance(
        cls, db_path: Optional[Path] = None
    ) -> CommandHistoryManager:
        """Singleton accessor for CommandHistoryManager."""
        with cls._singleton_lock:
            if cls._instance is None:
                cls._instance = cls(db_path)
            return cls._instance

    def _get_connection(self) -> sqlite3.Connection:
        """Creates a SQLite connection with WAL mode and row factory."""
        self._db_path.parent.mkdir(parents=True, exist_ok=True)
        conn = sqlite3.connect(
            str(self._db_path), timeout=10.0, check_same_thread=False
        )
        conn.row_factory = sqlite3.Row
        conn.execute("PRAGMA journal_mode=WAL;")
        conn.execute("PRAGMA synchronous=NORMAL;")
        return conn

    def _init_db(self) -> None:
        """Creates the command_history table and performance indexes."""
        with self._lock:
            try:
                with self._get_connection() as conn:
                    conn.execute(
                        """
                        CREATE TABLE IF NOT EXISTS command_history (
                            id TEXT PRIMARY KEY,
                            command TEXT NOT NULL,
                            cwd TEXT NOT NULL DEFAULT '',
                            host TEXT NOT NULL DEFAULT 'localhost',
                            session_name TEXT NOT NULL DEFAULT '',
                            exit_code INTEGER,
                            duration_ms INTEGER DEFAULT 0,
                            timestamp REAL NOT NULL,
                            is_pinned INTEGER DEFAULT 0,
                            execution_count INTEGER DEFAULT 1,
                            last_executed REAL NOT NULL
                        );
                        """
                    )
                    conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_cmd_history_timestamp ON command_history(timestamp DESC);"
                    )
                    conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_cmd_history_last_executed ON command_history(last_executed DESC);"
                    )
                    conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_cmd_history_cwd ON command_history(cwd);"
                    )
                    conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_cmd_history_host ON command_history(host);"
                    )
                    conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_cmd_history_pinned ON command_history(is_pinned);"
                    )
                    conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_cmd_history_command ON command_history(command);"
                    )
                    conn.commit()
                self.logger.debug(f"Command history database initialized at {self._db_path}")
            except Exception as e:
                self.logger.error(f"Failed to initialize command history database: {e}")

    def record_command(
        self,
        command: str,
        cwd: str = "",
        host: str = "localhost",
        session_name: str = "",
        exit_code: Optional[int] = 0,
        duration_ms: int = 0,
    ) -> Optional[CommandHistoryItem]:
        """
        Records an executed command. If the exact same command in the same CWD
        exists, updates its execution count, last_executed timestamp, exit code and duration.
        Otherwise, creates a new entry.
        """
        cleaned_cmd = command.strip()
        if not cleaned_cmd:
            return None

        # Ignore password or secret prompt patterns if any
        now = time.time()
        with self._lock:
            try:
                with self._get_connection() as conn:
                    # Check if matching command + cwd + host exists
                    cur = conn.execute(
                        """
                        SELECT id, execution_count, is_pinned FROM command_history
                        WHERE command = ? AND cwd = ? AND host = ?
                        LIMIT 1;
                        """,
                        (cleaned_cmd, cwd, host),
                    )
                    existing = cur.fetchone()

                    if existing:
                        entry_id = existing["id"]
                        new_count = existing["execution_count"] + 1
                        is_pinned = bool(existing["is_pinned"])
                        conn.execute(
                            """
                            UPDATE command_history
                            SET execution_count = ?,
                                last_executed = ?,
                                exit_code = ?,
                                duration_ms = ?,
                                session_name = ?
                            WHERE id = ?;
                            """,
                            (
                                new_count,
                                now,
                                exit_code,
                                duration_ms,
                                session_name,
                                entry_id,
                            ),
                        )
                    else:
                        entry_id = str(uuid.uuid4())
                        new_count = 1
                        is_pinned = False
                        conn.execute(
                            """
                            INSERT INTO command_history (
                                id, command, cwd, host, session_name,
                                exit_code, duration_ms, timestamp,
                                is_pinned, execution_count, last_executed
                            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
                            """,
                            (
                                entry_id,
                                cleaned_cmd,
                                cwd,
                                host,
                                session_name,
                                exit_code,
                                duration_ms,
                                now,
                                0,
                                1,
                                now,
                            ),
                        )
                    conn.commit()

                    return CommandHistoryItem(
                        id=entry_id,
                        command=cleaned_cmd,
                        cwd=cwd,
                        host=host,
                        session_name=session_name,
                        exit_code=exit_code,
                        duration_ms=duration_ms,
                        timestamp=now,
                        is_pinned=is_pinned,
                        execution_count=new_count,
                        last_executed=now,
                    )
            except Exception as e:
                self.logger.error(f"Error recording command history: {e}")
                return None

    def search_history(
        self,
        query: str = "",
        cwd: Optional[str] = None,
        host: Optional[str] = None,
        only_pinned: bool = False,
        limit: int = 100,
        offset: int = 0,
    ) -> List[CommandHistoryItem]:
        """
        Searches command history with optional filters (CWD, host, pinned)
        and relevance scoring for search query matches.
        """
        with self._lock:
            try:
                conditions: List[str] = []
                params: List[Any] = []

                if query.strip():
                    cleaned_q = query.strip()
                    # Substring match on command
                    conditions.append("command LIKE ?")
                    params.append(f"%{cleaned_q}%")

                if cwd:
                    conditions.append("cwd = ?")
                    params.append(cwd)

                if host:
                    conditions.append("host = ?")
                    params.append(host)

                if only_pinned:
                    conditions.append("is_pinned = 1")

                where_clause = (
                    f"WHERE {' AND '.join(conditions)}" if conditions else ""
                )

                sql = f"""
                    SELECT id, command, cwd, host, session_name,
                           exit_code, duration_ms, timestamp,
                           is_pinned, execution_count, last_executed
                    FROM command_history
                    {where_clause}
                    ORDER BY is_pinned DESC, last_executed DESC
                    LIMIT ? OFFSET ?;
                """
                params.extend([limit, offset])

                results: List[CommandHistoryItem] = []
                with self._get_connection() as conn:
                    cur = conn.execute(sql, params)
                    for row in cur.fetchall():
                        results.append(
                            CommandHistoryItem(
                                id=row["id"],
                                command=row["command"],
                                cwd=row["cwd"],
                                host=row["host"],
                                session_name=row["session_name"],
                                exit_code=row["exit_code"],
                                duration_ms=row["duration_ms"],
                                timestamp=row["timestamp"],
                                is_pinned=bool(row["is_pinned"]),
                                execution_count=row["execution_count"],
                                last_executed=row["last_executed"],
                            )
                        )

                # If query is provided, sort by relevance score
                if query.strip():
                    q_lower = query.strip().lower()

                    def _score_item(item: CommandHistoryItem) -> Tuple[int, int, float]:
                        cmd_lower = item.command.lower()
                        # Exact match
                        is_exact = 1 if cmd_lower == q_lower else 0
                        # Starts with match
                        starts_with = 1 if cmd_lower.startswith(q_lower) else 0
                        # Pinned priority + last_executed
                        return (is_exact, starts_with, 1 if item.is_pinned else 0, item.last_executed)

                    results.sort(key=_score_item, reverse=True)

                return results
            except Exception as e:
                self.logger.error(f"Error searching command history: {e}")
                return []

    def toggle_pin(self, entry_id: str) -> bool:
        """Toggles the favorite (is_pinned) status of a command entry."""
        with self._lock:
            try:
                with self._get_connection() as conn:
                    cur = conn.execute(
                        "SELECT is_pinned FROM command_history WHERE id = ?;",
                        (entry_id,),
                    )
                    row = cur.fetchone()
                    if not row:
                        return False
                    new_pinned = 0 if row["is_pinned"] else 1
                    conn.execute(
                        "UPDATE command_history SET is_pinned = ? WHERE id = ?;",
                        (new_pinned, entry_id),
                    )
                    conn.commit()
                    return bool(new_pinned)
            except Exception as e:
                self.logger.error(f"Error toggling pin on entry {entry_id}: {e}")
                return False

    def delete_entry(self, entry_id: str) -> bool:
        """Deletes a specific command history entry by ID."""
        with self._lock:
            try:
                with self._get_connection() as conn:
                    conn.execute(
                        "DELETE FROM command_history WHERE id = ?;", (entry_id,)
                    )
                    conn.commit()
                    return True
            except Exception as e:
                self.logger.error(f"Error deleting entry {entry_id}: {e}")
                return False

    def clear_history(self, scope: str = "all") -> int:
        """
        Clears command history.
        scope:
            'all' - deletes everything except pinned items.
            'everything' - deletes everything including pinned.
            'failed' - deletes items that finished with exit_code != 0.
        """
        with self._lock:
            try:
                with self._get_connection() as conn:
                    if scope == "everything":
                        cur = conn.execute("DELETE FROM command_history;")
                    elif scope == "failed":
                        cur = conn.execute(
                            "DELETE FROM command_history WHERE exit_code != 0 AND is_pinned = 0;"
                        )
                    else:  # all non-pinned
                        cur = conn.execute(
                            "DELETE FROM command_history WHERE is_pinned = 0;"
                        )
                    conn.commit()
                    return cur.rowcount
            except Exception as e:
                self.logger.error(f"Error clearing history (scope={scope}): {e}")
                return 0

    def get_stats(self) -> Dict[str, Any]:
        """Returns statistical metadata about the command history."""
        with self._lock:
            try:
                with self._get_connection() as conn:
                    total_count = conn.execute(
                        "SELECT COUNT(*) FROM command_history;"
                    ).fetchone()[0]
                    pinned_count = conn.execute(
                        "SELECT COUNT(*) FROM command_history WHERE is_pinned = 1;"
                    ).fetchone()[0]
                    top_cmds = conn.execute(
                        """
                        SELECT command, SUM(execution_count) as total_execs
                        FROM command_history
                        GROUP BY command
                        ORDER BY total_execs DESC
                        LIMIT 5;
                        """
                    ).fetchall()
                    return {
                        "total_entries": total_count,
                        "pinned_entries": pinned_count,
                        "top_commands": [
                            {"command": r["command"], "count": r["total_execs"]}
                            for r in top_cmds
                        ],
                    }
            except Exception as e:
                self.logger.error(f"Error getting history stats: {e}")
                return {
                    "total_entries": 0,
                    "pinned_entries": 0,
                    "top_commands": [],
                }


def get_command_history_manager(
    db_path: Optional[Path] = None,
) -> CommandHistoryManager:
    """Convenience accessor for CommandHistoryManager singleton."""
    return CommandHistoryManager.get_instance(db_path)
