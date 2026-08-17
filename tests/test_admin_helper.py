"""Tests for onyxsh-admin-helper parameter validation and execution dispatch."""

import unittest
from onyxsh.admin.helper import validate_and_build_command


class TestAdminHelper(unittest.TestCase):
    def setUp(self):
        self.actions_def = {
            "logs.vacuum": {
                "description": "Limpar logs",
                "argv": ["journalctl", "--vacuum-time={period}"],
                "params": {
                    "period": "^[0-9]+[dwmy]$"
                }
            },
            "flatpak.unused": {
                "description": "Desinstalar flatpaks",
                "argv": ["flatpak", "uninstall", "--unused", "-y"],
                "params": {}
            }
        }

    def test_validate_valid_admin_action(self):
        argv, desc = validate_and_build_command(
            "logs.vacuum",
            {"period": "14d"},
            self.actions_def,
        )
        self.assertEqual(argv, ["journalctl", "--vacuum-time=14d"])
        self.assertEqual(desc, "Limpar logs")

    def test_validate_unauthorized_action(self):
        with self.assertRaises(PermissionError):
            validate_and_build_command(
                "rm.all",
                {},
                self.actions_def,
            )

    def test_validate_parameter_regex_rejection(self):
        with self.assertRaises(ValueError):
            validate_and_build_command(
                "logs.vacuum",
                {"period": "7d; rm -rf /"},
                self.actions_def,
            )

    def test_validate_unexpected_parameter_rejection(self):
        with self.assertRaises(ValueError):
            validate_and_build_command(
                "flatpak.unused",
                {"extra": "value"},
                self.actions_def,
            )


if __name__ == "__main__":
    unittest.main()
