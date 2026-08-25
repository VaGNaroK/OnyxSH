"""Tests for AuditLogger and byte-identical backup rollback."""

import tempfile
import unittest
from pathlib import Path
from onyxsh.agent.audit import AuditLogger
from onyxsh.agent.models import AuditRecord, RiskLevel
from onyxsh.utils.backup import (
    create_file_backup,
    list_file_backups,
    rollback_file_backup,
)


class TestAuditRollback(unittest.TestCase):
    def test_audit_logger_append_and_rotate(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            log_file = Path(tmpdir) / "test_audit.jsonl"
            logger = AuditLogger(log_path=log_file)

            rec = AuditRecord(
                plan_id="plan_1",
                step_id="step_1",
                tool="shell.run",
                argv=["df", "-h"],
                risk=RiskLevel.READ_ONLY,
                user_decision="approved",
                result_status="success",
            )
            logger.append(rec)

            records = logger.get_records()
            self.assertEqual(len(records), 1)
            self.assertEqual(records[0].plan_id, "plan_1")
            self.assertEqual(records[0].argv, ["df", "-h"])

    def test_audit_logger_atomic_rotate_purges_old_records(self):
        """BUG-006: AuditLogger.rotate must atomically purge old entries and preserve recent ones."""
        from datetime import datetime, timedelta, timezone

        with tempfile.TemporaryDirectory() as tmpdir:
            log_file = Path(tmpdir) / "test_rotate_atomic.jsonl"
            logger = AuditLogger(log_path=log_file)

            # 1. Recent record (today)
            rec_recent = AuditRecord(
                plan_id="plan_recent",
                step_id="step_1",
                tool="shell.run",
                argv=["uptime"],
                risk=RiskLevel.READ_ONLY,
                user_decision="approved",
                result_status="success",
                timestamp=datetime.now(timezone.utc).isoformat(),
            )
            logger.append(rec_recent)

            # 2. Old record (60 days ago)
            old_time = datetime.now(timezone.utc) - timedelta(days=60)
            rec_old = AuditRecord(
                plan_id="plan_old",
                step_id="step_2",
                tool="shell.run",
                argv=["ls"],
                risk=RiskLevel.READ_ONLY,
                user_decision="approved",
                result_status="success",
                timestamp=old_time.isoformat(),
            )
            logger.append(rec_old)

            self.assertEqual(len(logger.get_records()), 2)

            # Rotate with 30-day retention
            purged = logger.rotate(retention_days=30)
            self.assertEqual(purged, 1)

            # Only recent record remains and file is valid JSONL
            remaining = logger.get_records()
            self.assertEqual(len(remaining), 1)
            self.assertEqual(remaining[0].plan_id, "plan_recent")

    def test_audit_logger_rotate_missing_file(self):
        """Rotating a non-existent log file should safely return 0 without error."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_file = Path(tmpdir) / "non_existent.jsonl"
            logger = AuditLogger(log_path=log_file)
            self.assertEqual(logger.rotate(retention_days=30), 0)

    def test_file_backup_manifest_and_rollback(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp_path = Path(tmpdir)
            test_file = tmp_path / "config.json"
            test_file.write_text('{"version": 1, "name": "initial"}', encoding="utf-8")

            staged_file = tmp_path / "staged.json"
            staged_file.write_text('{"version": 2, "name": "updated"}', encoding="utf-8")

            # Create backup
            manifest = create_file_backup(
                target_file=test_file,
                staged_file=staged_file,
                plan_id="plan_test_rb",
                step_id="step_rb",
            )
            self.assertTrue(manifest["backup_id"].startswith("backup_"))
            self.assertNotEqual(manifest["sha256_original"], "")

            # Overwrite target with staged content
            test_file.write_text('{"version": 2, "name": "updated"}', encoding="utf-8")
            self.assertIn('"version": 2', test_file.read_text(encoding="utf-8"))

            # Rollback from backup
            success = rollback_file_backup(manifest["backup_id"])
            self.assertTrue(success)

            # Verify restored byte-identical content
            restored_content = test_file.read_text(encoding="utf-8")
            self.assertEqual(restored_content, '{"version": 1, "name": "initial"}')


if __name__ == "__main__":
    unittest.main()
