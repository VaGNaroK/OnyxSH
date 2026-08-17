"""Tests for PathGuard anti-bypass symlink resolution and sensitive path denylists."""

import tempfile
import unittest
from pathlib import Path
from onyxsh.agent.path_guard import PathGuard


class TestPathGuard(unittest.TestCase):
    def test_path_guard_allows_safe_home_paths(self):
        pg = PathGuard(allowed_roots=[str(Path.home())])
        safe_file = Path.home() / "Documents" / "notes.txt"
        self.assertTrue(pg.can_read(safe_file))
        self.assertTrue(pg.can_write(safe_file))

    def test_path_guard_blocks_sensitive_read_paths(self):
        pg = PathGuard()
        ssh_key = Path.home() / ".ssh" / "id_rsa"
        self.assertFalse(pg.can_read(ssh_key))

        gnupg_dir = Path.home() / ".gnupg" / "secring.gpg"
        self.assertFalse(pg.can_read(gnupg_dir))

        aws_cred = Path.home() / ".aws" / "credentials"
        self.assertFalse(pg.can_read(aws_cred))

        env_file = Path.home() / "project" / ".env"
        self.assertFalse(pg.can_read(env_file))

    def test_path_guard_blocks_sensitive_write_paths(self):
        pg = PathGuard()
        bashrc = Path.home() / ".bashrc"
        self.assertFalse(pg.can_write(bashrc))

        zshrc = Path.home() / ".zshrc"
        self.assertFalse(pg.can_write(zshrc))

        profile = Path.home() / ".profile"
        self.assertFalse(pg.can_write(profile))

        shadow = Path("/etc/shadow")
        self.assertFalse(pg.can_write(shadow))

    def test_symlink_bypass_prevention(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp_path = Path(tmpdir)
            allowed_dir = tmp_path / "allowed"
            allowed_dir.mkdir()

            secret_file = tmp_path / "secret.env"
            secret_file.write_text("API_KEY=123456", encoding="utf-8")

            symlink_file = allowed_dir / "link_to_secret"
            try:
                symlink_file.symlink_to(secret_file)
            except OSError:
                self.skipTest("Symlinks not supported in environment")

            pg = PathGuard(
                allowed_roots=[str(allowed_dir)],
                read_denylist=[str(secret_file)],
            )

            # PathGuard must resolve realpath of symlink and block access
            self.assertFalse(pg.can_read(symlink_file))


if __name__ == "__main__":
    unittest.main()
