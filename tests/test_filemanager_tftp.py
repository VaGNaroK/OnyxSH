# tests/test_filemanager_tftp.py
"""Unit tests for onyxsh.filemanager.tftp_server.TftpServer and RFC 1350 logic."""

import os
import socket
import struct
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

from onyxsh.filemanager.tftp_server import (
    TftpBindError,
    TftpFileError,
    TftpNetworkError,
    TftpServer,
    TftpServerError,
)


class TestTftpServer(unittest.TestCase):
    """Tests covering TFTP packet parsing, security resolution, and server lifecycle."""

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.upload_dir = Path(self.temp_dir.name) / "upload"
        self.download_dir = Path(self.temp_dir.name) / "download"
        self.upload_dir.mkdir()
        self.download_dir.mkdir()
        self.server = TftpServer()

    def tearDown(self):
        if self.server.is_running:
            self.server.stop()
        self.temp_dir.cleanup()

    def test_parse_request_valid_rrq_octet(self):
        # RRQ opcode = 1, "firmware.bin\0octet\0"
        packet = struct.pack("!H", TftpServer.OP_RRQ) + b"firmware.bin\x00octet\x00"
        opcode, filename, mode = self.server._parse_request(packet)
        self.assertEqual(opcode, TftpServer.OP_RRQ)
        self.assertEqual(filename, "firmware.bin")
        self.assertEqual(mode, "octet")

    def test_parse_request_valid_wrq_netascii(self):
        # WRQ opcode = 2, "config.txt\0netascii\0"
        packet = struct.pack("!H", TftpServer.OP_WRQ) + b"config.txt\x00netascii\x00"
        opcode, filename, mode = self.server._parse_request(packet)
        self.assertEqual(opcode, TftpServer.OP_WRQ)
        self.assertEqual(filename, "config.txt")
        self.assertEqual(mode, "netascii")

    def test_parse_request_malformed(self):
        # Packet too short (< 4 bytes)
        with self.assertRaises(ValueError):
            self.server._parse_request(b"\x00\x01")

        # Missing null terminators / mode
        with self.assertRaises(ValueError):
            self.server._parse_request(struct.pack("!H", 1) + b"file_without_mode")

    def test_resolve_child_security_traversal_prevention(self):
        root = self.upload_dir
        safe_file = root / "allowed.txt"
        safe_file.write_text("ok", encoding="utf-8")

        # 1. Normal safe child
        resolved = self.server._resolve_child(root, "allowed.txt")
        self.assertIsNotNone(resolved)
        self.assertEqual(resolved, safe_file)

        # 2. Path traversal with ..
        self.assertIsNone(self.server._resolve_child(root, "../secret.txt"))
        self.assertIsNone(self.server._resolve_child(root, "../../etc/passwd"))
        self.assertIsNone(self.server._resolve_child(root, "sub/../../outside.txt"))

        # 3. Path with null byte
        self.assertIsNone(self.server._resolve_child(root, "file\x00name.txt"))

        # 4. Empty path
        self.assertIsNone(self.server._resolve_child(root, ""))
        self.assertIsNone(self.server._resolve_child(root, "///"))

    def test_start_validation_errors(self):
        # Invalid upload dir
        with self.assertRaises(TftpFileError):
            self.server.start(port=6969, upload_dir="/non/existent/path/dir", download_dir=str(self.download_dir))

        # Invalid download dir
        with self.assertRaises(TftpFileError):
            self.server.start(port=6969, upload_dir=str(self.upload_dir), download_dir="/non/existent/path/dir")

        # Invalid port
        with self.assertRaises(TftpBindError):
            self.server.start(port=-1, upload_dir=str(self.upload_dir), download_dir=str(self.download_dir))
        with self.assertRaises(TftpBindError):
            self.server.start(port=70000, upload_dir=str(self.upload_dir), download_dir=str(self.download_dir))

    def test_send_error_packet(self):
        mock_sock = MagicMock()
        client = ("127.0.0.1", 54321)

        self.server._send_error(mock_sock, client, TftpServer.ERR_NOT_FOUND)

        mock_sock.sendto.assert_called_once()
        sent_data, dest = mock_sock.sendto.call_args[0]
        self.assertEqual(dest, client)

        opcode, code = struct.unpack("!HH", sent_data[:4])
        self.assertEqual(opcode, TftpServer.OP_ERROR)
        self.assertEqual(code, TftpServer.ERR_NOT_FOUND)
        self.assertIn(b"File not found", sent_data[4:])

    def test_send_ack_packet(self):
        mock_sock = MagicMock()
        client = ("127.0.0.1", 54321)

        self.server._send_ack(mock_sock, client, block=5)

        mock_sock.sendto.assert_called_once()
        sent_data, dest = mock_sock.sendto.call_args[0]
        self.assertEqual(dest, client)

        opcode, block = struct.unpack("!HH", sent_data)
        self.assertEqual(opcode, TftpServer.OP_ACK)
        self.assertEqual(block, 5)

    def test_server_start_and_stop_lifecycle(self):
        running_states = []
        server = TftpServer(on_running_changed=lambda r: running_states.append(r))

        # Find an open port
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.bind(("", 0))
        port = sock.getsockname()[1]
        sock.close()

        server.start(port=port, upload_dir=str(self.upload_dir), download_dir=str(self.download_dir))
        
        # Wait up to 1 second for the server thread to bind and report running
        deadline = time.time() + 1.0
        while not server.is_running and time.time() < deadline:
            time.sleep(0.01)

        self.assertTrue(server.is_running)

        server.stop()
        self.assertFalse(server.is_running)
        self.assertIn(True, running_states)
        self.assertIn(False, running_states)


if __name__ == "__main__":
    unittest.main()
