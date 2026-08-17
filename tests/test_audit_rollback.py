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
