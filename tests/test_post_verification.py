# tests/test_post_verification.py
"""Unit tests for Post-Execution Verification Loop and sanity check inference."""

import unittest
from onyxsh.agent.verifier import (
    PostVerifier,
    VerificationCheck,
    VerificationResult,
    safe_quote_path,
)


class TestPostVerification(unittest.TestCase):
    def setUp(self):
        self.verifier = PostVerifier(is_flatpak=False)

    def test_infer_systemd_service_start(self):
        """Should infer active check and journalctl diagnostic for systemctl start."""
        checks = self.verifier.infer_verifications(["sudo systemctl start nginx"])
        self.assertEqual(len(checks), 1)
        chk = checks[0]
        self.assertEqual(chk.check_type, "service_active")
        self.assertEqual(chk.check_command, "systemctl is-active nginx.service")
        self.assertEqual(chk.expected_exit_code, 0)
        self.assertIn("journalctl -u nginx.service", chk.failure_diagnostic_command)

    def test_infer_systemd_service_enable(self):
        """Should infer enabled check for systemctl enable."""
        checks = self.verifier.infer_verifications(["systemctl enable sshd"])
        self.assertEqual(len(checks), 1)
        chk = checks[0]
        self.assertEqual(chk.check_type, "service_enabled")
        self.assertEqual(chk.check_command, "systemctl is-enabled sshd.service")

    def test_infer_nginx_config_syntax(self):
        """Should infer nginx -t for nginx reload or /etc/nginx edits."""
        checks = self.verifier.infer_verifications(["sudo nginx -s reload"])
        self.assertEqual(len(checks), 1)
        chk = checks[0]
        self.assertEqual(chk.check_type, "syntax_test")
        self.assertEqual(chk.check_command, "nginx -t")

    def test_infer_sshd_config_syntax(self):
        """Should infer sshd -t for sshd_config modifications."""
        checks = self.verifier.infer_verifications(["nano /etc/ssh/sshd_config"])
        self.assertEqual(len(checks), 1)
        chk = checks[0]
        self.assertEqual(chk.check_type, "syntax_test")
        self.assertEqual(chk.check_command, "sshd -t")

    def test_infer_file_permissions(self):
        """Should infer ls -ld for chmod/chown operations."""
        checks = self.verifier.infer_verifications(["chmod 755 /var/www/html"])
        self.assertEqual(len(checks), 1)
        chk = checks[0]
        self.assertEqual(chk.check_type, "path_permissions")
        self.assertIn("ls -ld", chk.check_command)
        self.assertIn("/var/www/html", chk.check_command)

    def test_infer_file_creation(self):
        """Should infer test -d for mkdir and test -e for touch."""
        checks = self.verifier.infer_verifications(["mkdir -p /tmp/test_dir", "touch /tmp/test_file.txt"])
        self.assertEqual(len(checks), 2)
        self.assertEqual(checks[0].check_type, "path_exists")
        self.assertIn("test -d", checks[0].check_command)
        self.assertEqual(checks[1].check_type, "path_exists")
        self.assertIn("test -e", checks[1].check_command)

    def test_infer_file_deletion(self):
        """Should infer test ! -e for rm commands."""
        checks = self.verifier.infer_verifications(["rm -rf /tmp/junk"])
        self.assertEqual(len(checks), 1)
        self.assertEqual(checks[0].check_type, "path_absent")
        self.assertIn("test ! -e", checks[0].check_command)

    def test_infer_package_installation(self):
        """Should infer dpkg -s for apt install."""
        checks = self.verifier.infer_verifications(["sudo apt install -y htop curl"])
        self.assertEqual(len(checks), 2)
        self.assertTrue(all(c.check_type == "package_installed" for c in checks))
        self.assertEqual(checks[0].check_command, "dpkg -s htop")
        self.assertEqual(checks[1].check_command, "dpkg -s curl")

    def test_infer_docker_run(self):
        """Should infer docker ps check for docker run."""
        checks = self.verifier.infer_verifications(["docker run -d --name redis_db redis:alpine"])
        self.assertEqual(len(checks), 1)
        self.assertEqual(checks[0].check_type, "docker_status")
        self.assertIn("docker ps -f name=redis_db", checks[0].check_command)

    def test_infer_ufw_firewall(self):
        """Should infer ufw status verbose for ufw commands."""
        checks = self.verifier.infer_verifications(["sudo ufw allow 8080/tcp"])
        self.assertEqual(len(checks), 1)
        self.assertEqual(checks[0].check_type, "firewall_status")
        self.assertEqual(checks[0].check_command, "ufw status verbose")

    def test_no_verification_for_read_only_commands(self):
        """Read-only diagnostic commands should not generate mutation verification checks."""
        checks = self.verifier.infer_verifications(["ls -la", "df -h", "free -m", "ps aux", "cat /etc/os-release"])
        self.assertEqual(len(checks), 0)

    def test_run_verification_success(self):
        """Successful verification should return success=True and status=success."""
        check = VerificationCheck(
            target_command="echo hello",
            check_command="echo check_ok",
            check_type="generic",
            description="Test check",
            expected_exit_code=0,
        )

        def mock_runner(cmd):
            return 0, "check_ok", ""

        res = self.verifier.run_verification(check, custom_runner=mock_runner)
        self.assertTrue(res.success)
        self.assertEqual(res.exit_code, 0)
        self.assertEqual(check.status, "success")

    def test_run_verification_failure_with_diagnostics(self):
        """Failed verification should capture diagnostics from failure_diagnostic_command."""
        check = VerificationCheck(
            target_command="sudo systemctl restart nginx",
            check_command="systemctl is-active nginx.service",
            check_type="service_active",
            description="Check nginx",
            expected_exit_code=0,
            failure_diagnostic_command="journalctl -u nginx.service -n 25 --no-pager",
        )

        def mock_runner(cmd):
            if "is-active" in cmd:
                return 3, "failed", "service failed to start"
            if "journalctl" in cmd:
                return 0, "Aug 19 10:00:00 server nginx[123]: Address already in use", ""
            return 0, "", ""

        res = self.verifier.run_verification(check, custom_runner=mock_runner)
        self.assertFalse(res.success)
        self.assertEqual(res.exit_code, 3)
        self.assertEqual(check.status, "failed")
        self.assertIsNotNone(res.diagnostic_output)
        self.assertIn("Address already in use", res.diagnostic_output)

    def test_infer_file_permissions_tilde_expansion(self):
        """Should safely expand tilde and $HOME in check command instead of literal quoted tilde."""
        checks = self.verifier.infer_verifications(["chmod +x ~/bloqueio_hosts.sh"])
        self.assertEqual(len(checks), 1)
        chk = checks[0]
        self.assertEqual(chk.check_type, "path_permissions")
        # Check command should use $HOME instead of literal '~' in single quotes
        self.assertIn('"$HOME/bloqueio_hosts.sh"', chk.check_command)
        self.assertNotIn("'~/bloqueio_hosts.sh'", chk.check_command)

    def test_safe_quote_path_dangerous_metacharacters(self):
        """BUG-007: safe_quote_path must properly quote subpaths containing shell metacharacters."""
        # Subshell execution attempt
        quoted = safe_quote_path("~/evil$(reboot)/file.txt")
        self.assertEqual(quoted, '"$HOME"/\'evil$(reboot)/file.txt\'')

        # Backticks attempt
        quoted_bt = safe_quote_path("~/evil`whoami`/file.txt")
        self.assertEqual(quoted_bt, '"$HOME"/\'evil`whoami`/file.txt\'')

        # Semicolon command injection attempt
        quoted_semi = safe_quote_path("~/dir; rm -rf /")
        self.assertEqual(quoted_semi, '"$HOME"/\'dir; rm -rf /\'')

        # Variable expansion in subpath
        quoted_var = safe_quote_path("$HOME/evil$DANGEROUS/path")
        self.assertEqual(quoted_var, '"$HOME"/\'evil$DANGEROUS/path\'')

    def test_safe_quote_path_standard_paths(self):
        """Standard paths and simple tilde/HOME references must expand cleanly."""
        self.assertEqual(safe_quote_path("~"), '"$HOME"')
        self.assertEqual(safe_quote_path("$HOME"), '"$HOME"')
        self.assertEqual(safe_quote_path("~/simple.txt"), '"$HOME/simple.txt"')
        self.assertEqual(safe_quote_path("/var/log/nginx.log"), "/var/log/nginx.log")


if __name__ == "__main__":
    unittest.main()
