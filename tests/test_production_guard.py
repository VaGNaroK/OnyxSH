# tests/test_production_guard.py

import unittest

from onyxsh.sessions.models import SessionFolder, SessionItem
from onyxsh.terminal.production_guard import ProductionGuard, get_production_guard
from onyxsh.utils.translation_utils import _


class TestProductionGuard(unittest.TestCase):
    def setUp(self):
        self.guard = ProductionGuard()

    def test_session_item_is_production(self):
        # Default is False
        session = SessionItem(name="Test Session", session_type="ssh", host="prod-srv-01.company.com")
        self.assertFalse(session.is_production)

        # Explicitly set to True
        session.is_production = True
        self.assertTrue(session.is_production)

        # Serialization to dict
        data = session.to_dict()
        self.assertTrue(data.get("is_production"))

        # Deserialization from dict
        loaded = SessionItem.from_dict(data)
        self.assertTrue(loaded.is_production)

    def test_session_folder_is_production(self):
        folder = SessionFolder(name="Production Cluster", path="/Production Cluster", is_production=True)
        self.assertTrue(folder.is_production)

        data = folder.to_dict()
        self.assertTrue(data.get("is_production"))

        loaded = SessionFolder.from_dict(data)
        self.assertTrue(loaded.is_production)

    def test_destructive_rm_commands(self):
        dangerous = [
            "rm -rf /var/log/*",
            "rm -fr /etc/nginx",
            "sudo rm -rf /",
            "rm -r /opt/app",
            "sudo rm --recursive --force /data",
            "shred -u secret.txt",
            "wipefs -a /dev/sdb",
        ]
        for cmd in dangerous:
            violation = self.guard.evaluate_command(cmd)
            self.assertIsNotNone(violation, f"Command should be detected as dangerous: {cmd}")
            self.assertIn(violation.category, [_("File System"), _("Storage & Partitions")])

    def test_destructive_disk_commands(self):
        dangerous = [
            "mkfs.ext4 /dev/nvme0n1p1",
            "sudo mkfs.xfs /dev/sda1",
            "dd if=/dev/zero of=/dev/sda bs=1M",
            "sudo fdisk /dev/sdb",
            "parted /dev/sdc",
        ]
        for cmd in dangerous:
            violation = self.guard.evaluate_command(cmd)
            self.assertIsNotNone(violation, f"Command should be detected as dangerous: {cmd}")
            self.assertEqual(violation.category, _("Storage & Partitions"))

    def test_system_power_commands(self):
        dangerous = [
            "shutdown -h now",
            "sudo shutdown -r +5",
            "reboot",
            "sudo poweroff",
            "init 0",
            "sudo init 6",
        ]
        for cmd in dangerous:
            violation = self.guard.evaluate_command(cmd)
            self.assertIsNotNone(violation, f"Command should be detected as dangerous: {cmd}")
            self.assertEqual(violation.category, _("System Power"))

    def test_service_commands(self):
        dangerous = [
            "systemctl stop nginx",
            "sudo systemctl disable postgresql",
            "service apache2 stop",
            "service mysql restart",
        ]
        for cmd in dangerous:
            violation = self.guard.evaluate_command(cmd)
            self.assertIsNotNone(violation, f"Command should be detected as dangerous: {cmd}")
            self.assertEqual(violation.category, _("Services & Daemons"))

    def test_git_destructive_commands(self):
        dangerous = [
            "git reset --hard HEAD~1",
            "git clean -fd",
            "git push origin main --force",
            "git push -f origin main",
        ]
        for cmd in dangerous:
            violation = self.guard.evaluate_command(cmd)
            self.assertIsNotNone(violation, f"Command should be detected as dangerous: {cmd}")
            self.assertEqual(violation.category, _("Version Control"))

    def test_database_destructive_commands(self):
        dangerous = [
            "DROP DATABASE production_db;",
            "TRUNCATE TABLE users;",
            "drop schema public cascade;",
        ]
        for cmd in dangerous:
            violation = self.guard.evaluate_command(cmd)
            self.assertIsNotNone(violation, f"Command should be detected as dangerous: {cmd}")
            self.assertEqual(violation.category, _("Database"))

    def test_safe_commands_allowed(self):
        safe = [
            "ls -la /var/log",
            "cd /home/user",
            "cat /etc/hosts",
            "git status",
            "git diff",
            "git log -n 5",
            "systemctl status nginx",
            "journalctl -u docker -n 50",
            "ps aux | grep python",
            "top",
            "htop",
            "df -h",
            "free -m",
            "ping 8.8.8.8",
            "curl https://example.com",
        ]
        for cmd in safe:
            violation = self.guard.evaluate_command(cmd)
            self.assertIsNone(violation, f"Safe command should NOT be blocked: {cmd}")

    def test_composite_and_multiline_destructive_commands(self):
        composite_dangerous = [
            "cd /tmp && rm -rf /tmp/pasta",
            "ls -la ; sudo rm -rf /tmp/test",
            "mkdir /tmp/x || rm -fr /tmp/x",
            "echo 'hello' | rm -rf /tmp/demo",
            "cd /tmp\nls -la pasta\nrm -rf /tmp/pasta",
            "sudo -s\nrm -rf /var/cache/*",
        ]
        for cmd in composite_dangerous:
            violation = self.guard.evaluate_command(cmd)
            self.assertIsNotNone(violation, f"Composite command should be detected as dangerous: {cmd}")
            self.assertEqual(violation.category, _("File System"))


if __name__ == "__main__":
    unittest.main()
