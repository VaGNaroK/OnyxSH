# tests/test_filemanager_operations.py
"""Unit tests for onyxsh.filemanager.operations.FileOperations."""

import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

from onyxsh.filemanager.operations import FileOperations, OperationCancelledError
from onyxsh.sessions.models import SessionItem


class TestFileOperations(unittest.TestCase):
    """Tests covering FileOperations methods, parsing, caching, and error handling."""

    def setUp(self):
        self.local_session = SessionItem(
            name="Local Session",
            session_type="local",
            user="localuser",
            host="localhost",
            port=22,
        )
        self.ssh_session = SessionItem(
            name="Remote SSH",
            session_type="ssh",
            user="remoteuser",
            host="remote.server.org",
            port=2222,
        )
        self.ops_local = FileOperations(self.local_session)
        self.ops_ssh = FileOperations(self.ssh_session)

    def tearDown(self):
        self.ops_local.shutdown()
        self.ops_ssh.shutdown()

    def test_get_session_key(self):
        key_local = self.ops_local._get_session_key(self.local_session)
        self.assertEqual(key_local, "localuser@localhost:22")

        key_ssh = self.ops_ssh._get_session_key(self.ssh_session)
        self.assertEqual(key_ssh, "remoteuser@remote.server.org:2222")

    def test_normalize_remote_path(self):
        with patch.object(self.ops_ssh, "_get_remote_home_directory", return_value="/home/remoteuser"):
            # $HOME tokens
            self.assertEqual(self.ops_ssh._normalize_remote_path("$HOME", self.ssh_session), "/home/remoteuser")
            self.assertEqual(self.ops_ssh._normalize_remote_path("${HOME}", self.ssh_session), "/home/remoteuser")
            self.assertEqual(self.ops_ssh._normalize_remote_path("~", self.ssh_session), "/home/remoteuser")

            # Subpaths with $HOME or ~
            self.assertEqual(self.ops_ssh._normalize_remote_path("$HOME/logs/app.log", self.ssh_session), "/home/remoteuser/logs/app.log")
            self.assertEqual(self.ops_ssh._normalize_remote_path("${HOME}/scripts/test.sh", self.ssh_session), "/home/remoteuser/scripts/test.sh")
            self.assertEqual(self.ops_ssh._normalize_remote_path("~/data/file.csv", self.ssh_session), "/home/remoteuser/data/file.csv")

            # Absolute or relative paths untouched
            self.assertEqual(self.ops_ssh._normalize_remote_path("/var/log/syslog", self.ssh_session), "/var/log/syslog")
            self.assertEqual(self.ops_ssh._normalize_remote_path("relative/path.txt", self.ssh_session), "relative/path.txt")
            self.assertEqual(self.ops_ssh._normalize_remote_path("", self.ssh_session), "")

        # When remote home cannot be determined
        with patch.object(self.ops_ssh, "_get_remote_home_directory", return_value=None):
            self.assertEqual(self.ops_ssh._normalize_remote_path("$HOME/logs", self.ssh_session), "./logs")
            self.assertEqual(self.ops_ssh._normalize_remote_path("~", self.ssh_session), ".")

    def test_command_available_and_cache(self):
        # Mock execute_command_on_session to return True for rsync, False for fakecmd
        def mock_exec(cmd, **kwargs):
            if "rsync" in cmd:
                return True, "/usr/bin/rsync"
            return False, ""

        with patch.object(self.ops_local, "execute_command_on_session", side_effect=mock_exec) as mock_cmd:
            # 1. First call checks command and populates cache
            self.assertTrue(self.ops_local.check_command_available("rsync"))
            self.assertEqual(mock_cmd.call_count, 1)

            # 2. Second call uses cache (call count remains 1)
            self.assertTrue(self.ops_local.check_command_available("rsync", use_cache=True))
            self.assertEqual(mock_cmd.call_count, 1)

            # 3. Bypass cache triggers execution
            self.assertTrue(self.ops_local.check_command_available("rsync", use_cache=False))
            self.assertEqual(mock_cmd.call_count, 2)

            # 4. Unavailable command
            self.assertFalse(self.ops_local.check_command_available("fakecmd"))

    def test_parse_transfer_error(self):
        # Permission denied in English
        err_en = "rsync: failed to set permissions on '/root/test': Permission denied (13)"
        parsed_en = self.ops_local._parse_transfer_error(err_en)
        self.assertTrue("Permission Denied" in parsed_en or "Permissão Negada" in parsed_en)

        # Permission denied in Portuguese
        err_pt = "cp: não é possível criar arquivo '/etc/test': Permissão negada"
        parsed_pt = self.ops_local._parse_transfer_error(err_pt)
        self.assertTrue("Permission Denied" in parsed_pt or "Permissão Negada" in parsed_pt)

        # Operation not permitted
        err_op = "rm: cannot remove '/sys/kernel': Operation not permitted"
        parsed_op = self.ops_local._parse_transfer_error(err_op)
        self.assertTrue("Permission Denied" in parsed_op or "Permissão Negada" in parsed_op)

        # Empty output fallback
        parsed_empty = self.ops_local._parse_transfer_error("   ")
        self.assertTrue("unknown transfer error" in parsed_empty.lower() or "desconhecido" in parsed_empty.lower() or "erro" in parsed_empty.lower())

        # Other error messages should be preserved
        custom_err = "No space left on device"
        self.assertEqual(self.ops_local._parse_transfer_error(custom_err), custom_err)

    def test_get_directory_size_local(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            test_file = os.path.join(tmpdir, "test.bin")
            with open(test_file, "wb") as f:
                f.write(b"x" * 1024)

            size = self.ops_local.get_directory_size(tmpdir, is_remote=False)
            self.assertGreaterEqual(size, 1024)

    def test_get_directory_size_remote(self):
        with patch.object(self.ops_ssh, "execute_command_on_session", return_value=(True, "4096000\t/var/www")):
            size = self.ops_ssh.get_directory_size("/var/www", is_remote=True)
            self.assertEqual(size, 4096000)

        # Failure case
        with patch.object(self.ops_ssh, "execute_command_on_session", return_value=(False, "du: cannot read")):
            size = self.ops_ssh.get_directory_size("/var/www", is_remote=True)
            self.assertEqual(size, 0)

    def test_get_free_space_local(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            free_space = self.ops_local.get_free_space(tmpdir, is_remote=False)
            self.assertGreater(free_space, 0)

    def test_get_free_space_remote(self):
        output = "Avail\n53687091200\n"
        with patch.object(self.ops_ssh, "execute_command_on_session", return_value=(True, output)):
            avail = self.ops_ssh.get_free_space("/home/remoteuser", is_remote=True)
            self.assertEqual(avail, 53687091200)

        with patch.object(self.ops_ssh, "execute_command_on_session", return_value=(False, "df: error")):
            avail = self.ops_ssh.get_free_space("/home/remoteuser", is_remote=True)
            self.assertEqual(avail, -1)

    def test_get_remote_file_timestamp(self):
        with patch.object(self.ops_ssh, "execute_command_on_session", return_value=(True, "1756140000\n")):
            ts = self.ops_ssh.get_remote_file_timestamp("/var/log/syslog")
            self.assertEqual(ts, 1756140000)

        with patch.object(self.ops_ssh, "execute_command_on_session", return_value=(False, "stat: cannot stat")):
            ts = self.ops_ssh.get_remote_file_timestamp("/var/log/syslog")
            self.assertIsNone(ts)

    def test_download_file_sync_validation(self):
        # Calling on a local session should fail early
        success, msg = self.ops_local.download_file_sync("/path", "/tmp/local")
        self.assertFalse(success)
        self.assertTrue("Not a remote SSH session" in msg or "Não é uma sessão SSH remota" in msg or "SSH" in msg)

    def test_shutdown_terminates_active_processes(self):
        mock_proc = MagicMock()
        mock_proc.pid = 99999
        self.ops_local._active_processes["transfer-1"] = mock_proc

        with patch("os.getpgid", return_value=99999), patch("os.killpg") as mock_killpg:
            self.ops_local.shutdown()
            mock_killpg.assert_called_once()
            self.assertEqual(len(self.ops_local._active_processes), 0)


if __name__ == "__main__":
    unittest.main()
