"""
Unit tests for OSC 52 Clipboard Bridge in OnyxSH.
Validates extraction, decoding, security filtering, and clipboard synchronization.
"""

import base64
import unittest
from unittest.mock import MagicMock, patch

from onyxsh.terminal.osc52_bridge import (
    MAX_PAYLOAD_BYTES,
    apply_osc52_to_clipboard,
    parse_osc52_sequences,
    process_osc52_bytes,
    strip_osc52_sequences,
)


class TestOsc52Bridge(unittest.TestCase):
    def test_parse_osc52_basic_clipboard_bel(self):
        payload = base64.b64encode(b"git status --short").decode("ascii")
        data = f"\x1b]52;c;{payload}\x07".encode("ascii")
        results = parse_osc52_sequences(data)
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][0], "c")
        self.assertEqual(results[0][1], "git status --short")

    def test_parse_osc52_string_terminator(self):
        payload = base64.b64encode(b"cargo build --release").decode("ascii")
        data = f"\x1b]52;c;{payload}\x1b\\".encode("ascii")
        results = parse_osc52_sequences(data)
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][0], "c")
        self.assertEqual(results[0][1], "cargo build --release")

    def test_parse_osc52_primary_selection(self):
        payload = base64.b64encode(b"selected text").decode("ascii")
        data = f"\x1b]52;p;{payload}\x07".encode("ascii")
        results = parse_osc52_sequences(data)
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][0], "p")
        self.assertEqual(results[0][1], "selected text")

    def test_parse_osc52_multiple_targets(self):
        payload = base64.b64encode(b"multi target content").decode("ascii")
        data = f"\x1b]52;cp;{payload}\x07".encode("ascii")
        results = parse_osc52_sequences(data)
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][0], "cp")
        self.assertEqual(results[0][1], "multi target content")

    def test_parse_osc52_empty_target_defaults_to_c(self):
        payload = base64.b64encode(b"default clipboard content").decode("ascii")
        data = f"\x1b]52;;{payload}\x07".encode("ascii")
        results = parse_osc52_sequences(data)
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][0], "c")
        self.assertEqual(results[0][1], "default clipboard content")

    def test_parse_osc52_reject_query_zero_trust(self):
        # Query sequence: \x1b]52;c;?\x07
        data = b"\x1b]52;c;?\x07"
        results = parse_osc52_sequences(data)
        # Must be rejected to prevent malicious reading of user's clipboard
        self.assertEqual(results, [])

    def test_parse_osc52_payload_size_limit(self):
        oversized_payload = b"A" * 200
        data = b"\x1b]52;c;" + oversized_payload + b"\x07"
        results = parse_osc52_sequences(data, max_payload_bytes=100)
        self.assertEqual(results, [])

    def test_parse_osc52_whitespace_in_payload(self):
        # Base64 with whitespace/newlines
        b64_str = "SGVs bG8g\n V29y bGQ="
        data = f"\x1b]52;c;{b64_str}\x07".encode("ascii")
        results = parse_osc52_sequences(data)
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][1], "Hello World")

    def test_parse_osc52_utf8_multibyte(self):
        text = "Código gerado pelo OpenCode: 🚀 — Atenção à acentuação!"
        payload = base64.b64encode(text.encode("utf-8")).decode("ascii")
        data = f"\x1b]52;c;{payload}\x07".encode("utf-8")
        results = parse_osc52_sequences(data)
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][1], text)

    def test_parse_osc52_invalid_base64_graceful(self):
        data = b"\x1b]52;c;???not_valid_base64???\x07"
        results = parse_osc52_sequences(data)
        self.assertEqual(results, [])

    def test_strip_osc52_sequences(self):
        payload = base64.b64encode(b"secret").decode("ascii")
        data = f"Prefix \x1b]52;c;{payload}\x07 Middle \x1b]52;p;{payload}\x1b\\ Suffix".encode(
            "ascii"
        )
        stripped = strip_osc52_sequences(data)
        self.assertEqual(stripped, b"Prefix  Middle  Suffix")

    def test_strip_osc52_no_match(self):
        data = b"Normal terminal output with \x1b[31mcolor\x1b[0m."
        self.assertEqual(strip_osc52_sequences(data), data)

    def test_apply_osc52_to_clipboard_regular(self):
        mock_cb = MagicMock()
        success = apply_osc52_to_clipboard("c", "Test Content", clipboard=mock_cb)
        self.assertTrue(success)
        mock_cb.set.assert_called_once_with("Test Content")

    def test_apply_osc52_to_clipboard_primary(self):
        mock_primary = MagicMock()
        success = apply_osc52_to_clipboard(
            "p", "Primary Content", primary_clipboard=mock_primary
        )
        self.assertTrue(success)
        mock_primary.set.assert_called_once_with("Primary Content")

    def test_apply_osc52_to_clipboard_both(self):
        mock_cb = MagicMock()
        mock_primary = MagicMock()
        success = apply_osc52_to_clipboard(
            "cp", "Dual Content", clipboard=mock_cb, primary_clipboard=mock_primary
        )
        self.assertTrue(success)
        mock_cb.set.assert_called_once_with("Dual Content")
        mock_primary.set.assert_called_once_with("Dual Content")

    def test_apply_osc52_to_clipboard_terminal_resolution(self):
        mock_cb = MagicMock()
        mock_term = MagicMock()
        mock_term.get_clipboard.return_value = mock_cb
        success = apply_osc52_to_clipboard("c", "Term Content", terminal=mock_term)
        self.assertTrue(success)
        mock_cb.set.assert_called_once_with("Term Content")

    def test_apply_osc52_to_clipboard_error_resilience(self):
        mock_cb = MagicMock()
        mock_cb.set.side_effect = RuntimeError("GDK Clipboard error")
        success = apply_osc52_to_clipboard("c", "Crash Test", clipboard=mock_cb)
        self.assertFalse(success)

    def test_process_osc52_bytes_enabled(self):
        mock_cb = MagicMock()
        mock_term = MagicMock()
        mock_term.get_clipboard.return_value = mock_cb

        payload = base64.b64encode(b"npm test").decode("ascii")
        data = f"Start \x1b]52;c;{payload}\x07 End".encode("ascii")

        clean_data = process_osc52_bytes(data, terminal=mock_term, enabled=True)
        self.assertEqual(clean_data, b"Start  End")
        mock_cb.set.assert_called_once_with("npm test")

    def test_process_osc52_bytes_disabled(self):
        mock_cb = MagicMock()
        mock_term = MagicMock()
        mock_term.get_clipboard.return_value = mock_cb

        payload = base64.b64encode(b"npm test").decode("ascii")
        data = f"Start \x1b]52;c;{payload}\x07 End".encode("ascii")

        clean_data = process_osc52_bytes(data, terminal=mock_term, enabled=False)
        self.assertEqual(clean_data, b"Start  End")
        mock_cb.set.assert_not_called()

    def test_handle_osc52_stream_bytes_proxy(self):
        from onyxsh.terminal._highlighter_impl import HighlightedTerminalProxy

        mock_terminal = MagicMock()
        mock_cb = MagicMock()
        mock_terminal.get_clipboard.return_value = mock_cb

        proxy = HighlightedTerminalProxy(mock_terminal, "local", proxy_id=99)
        proxy._settings_manager = MagicMock()
        proxy._settings_manager.get.return_value = True

        payload = base64.b64encode(b"echo 123").decode("ascii")
        data = f"hello \x1b]52;c;{payload}\x07 world".encode("ascii")

        cleaned = proxy._handle_osc52_stream_bytes(data)
        self.assertEqual(cleaned, b"hello  world")
        mock_cb.set.assert_called_once_with("echo 123")


if __name__ == "__main__":
    unittest.main()
