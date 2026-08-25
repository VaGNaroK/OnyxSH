"""Unit tests for security utilities and HostnameValidator (BUG-008)."""

import threading
import unittest
from onyxsh.utils.security import (
    HostnameValidator,
    InputSanitizer,
    SSHKeyValidator,
    SecurityConfig,
)


class TestSecurityUtils(unittest.TestCase):
    def test_resolve_hostname_localhost(self):
        """Should successfully resolve localhost to an IP address."""
        ip = HostnameValidator.resolve_hostname("localhost", timeout=2.0)
        self.assertIsNotNone(ip)
        self.assertIn(ip, ("127.0.0.1", "::1"))

    def test_resolve_hostname_from_worker_thread(self):
        """BUG-008: resolve_hostname must work safely inside worker threads without SIGALRM errors."""
        result_holder = []
        error_holder = []

        def worker():
            try:
                ip = HostnameValidator.resolve_hostname("localhost", timeout=2.0)
                result_holder.append(ip)
            except Exception as e:
                error_holder.append(e)

        thread = threading.Thread(target=worker)
        thread.start()
        thread.join(timeout=5.0)

        self.assertEqual(len(error_holder), 0, f"Thread threw exception: {error_holder}")
        self.assertEqual(len(result_holder), 1)
        self.assertIsNotNone(result_holder[0])

    def test_resolve_hostname_invalid_domain(self):
        """Non-existent domains should safely return None without throwing exceptions."""
        res = HostnameValidator.resolve_hostname("invalid.domain.that.does.not.exist.example.test", timeout=1.0)
        self.assertIsNone(res)

    def test_is_private_ip(self):
        self.assertTrue(HostnameValidator.is_private_ip("127.0.0.1"))
        self.assertTrue(HostnameValidator.is_private_ip("10.0.0.1"))
        self.assertTrue(HostnameValidator.is_private_ip("192.168.1.1"))
        self.assertFalse(HostnameValidator.is_private_ip("8.8.8.8"))
        self.assertFalse(HostnameValidator.is_private_ip("invalid-ip"))

    def test_sanitize_hostname(self):
        self.assertEqual(InputSanitizer.sanitize_hostname("server-01.prod.com"), "server-01.prod.com")
        self.assertEqual(InputSanitizer.sanitize_hostname("server$01;rm -rf"), "server01rm-rf")

    def test_is_valid_hostname(self):
        self.assertTrue(HostnameValidator.is_valid_hostname("my-server.example.com"))
        self.assertTrue(HostnameValidator.is_valid_hostname("localhost"))
        self.assertFalse(HostnameValidator.is_valid_hostname("bad hostname with spaces"))
        self.assertFalse(HostnameValidator.is_valid_hostname("-leading-dash.com"))


if __name__ == "__main__":
    unittest.main()
