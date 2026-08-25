"""Audit logging and retention system for OnyxSH Agent Mode."""

from __future__ import annotations

import json
import os
import threading
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Optional

from .models import AuditRecord


AUDIT_BASE_DIR = Path.home() / ".local" / "share" / "onyxsh" / "audit"
DEFAULT_AUDIT_LOG = AUDIT_BASE_DIR / "audit.jsonl"


class AuditLogger:
    """Thread-safe append-only audit logger for tracking agent actions and decisions."""

    _instance: Optional[AuditLogger] = None
    _lock = threading.Lock()

    def __init__(self, log_path: Optional[Path] = None) -> None:
        self.log_path = log_path or DEFAULT_AUDIT_LOG
        self.log_path.parent.mkdir(parents=True, exist_ok=True)
        self._write_lock = threading.Lock()

    @classmethod
    def get_default(cls) -> AuditLogger:
        """Singleton accessor for the default audit logger."""
        if cls._instance is None:
            with cls._lock:
                if cls._instance is None:
                    cls._instance = cls()
        return cls._instance

    def append(self, record: AuditRecord) -> None:
        """
        Append an AuditRecord in JSONL format.
        """
        payload = json.dumps(record.to_dict(), ensure_ascii=False)
        with self._write_lock:
            with open(self.log_path, "a", encoding="utf-8") as f:
                f.write(payload + "\n")

    def get_records(self, limit: int = 100) -> list[AuditRecord]:
        """
        Read recorded audits from newest to oldest.
        """
        if not self.log_path.exists():
            return []

        records: list[AuditRecord] = []
        with self._write_lock:
            try:
                with open(self.log_path, "r", encoding="utf-8") as f:
                    for line in f:
                        line_str = line.strip()
                        if line_str:
                            try:
                                data = json.loads(line_str)
                                records.append(AuditRecord.from_dict(data))
                            except Exception:
                                continue
            except Exception:
                return []

        return list(reversed(records))[:limit]

    def rotate(self, retention_days: int = 30) -> int:
        """
        Rotate and purge audit entries older than retention threshold.

        Returns:
            Number of purged records.
        """
        if not self.log_path.exists():
            return 0

        cutoff = datetime.now(timezone.utc) - timedelta(days=retention_days)
        kept_records: list[dict] = []
        purged_count = 0

        with self._write_lock:
            try:
                with open(self.log_path, "r", encoding="utf-8") as f:
                    for line in f:
                        line_str = line.strip()
                        if line_str:
                            try:
                                data = json.loads(line_str)
                                ts_str = data.get("timestamp", "")
                                record_time = datetime.fromisoformat(ts_str)
                                if record_time >= cutoff:
                                    kept_records.append(data)
                                else:
                                    purged_count += 1
                            except Exception:
                                kept_records.append(data)

                # Rewrite file with kept records atomically using temporary file
                tmp_path = self.log_path.with_name(f"{self.log_path.name}.tmp.{os.getpid()}")
                with open(tmp_path, "w", encoding="utf-8") as f:
                    for item in kept_records:
                        f.write(json.dumps(item, ensure_ascii=False) + "\n")
                    f.flush()
                    os.fsync(f.fileno())

                # Atomic replace on same filesystem
                tmp_path.replace(self.log_path)
            except Exception:
                try:
                    if "tmp_path" in locals() and tmp_path.exists():
                        tmp_path.unlink()
                except Exception:
                    pass
                return 0

        return purged_count

