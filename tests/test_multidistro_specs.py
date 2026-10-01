"""Unit tests for Multi-Distro Administrative Commands, Specs, and Polkit mappings."""

import unittest
from unittest.mock import patch

from onyxsh.agent.admin_tools import AdminTools
from onyxsh.data.command_manager_models import get_builtin_commands
from onyxsh.terminal.completion.specs.registry import SpecRegistry
from onyxsh.ui.dialogs.command_manager_dialog import CommandFormDialog


class TestMultiDistroSpecs(unittest.TestCase):
    """Tests for completion specs of dnf, yum, and pacman."""

    def setUp(self):
        self.registry = SpecRegistry()

    def test_dnf_spec_registered(self):
        spec = self.registry.get_spec("dnf")
        self.assertIsNotNone(spec)
        self.assertEqual(spec.name, "dnf")

        subcmds = [s.name for s in spec.subcommands]
        self.assertIn("install", subcmds)
        self.assertIn("remove", subcmds)
        self.assertIn("upgrade", subcmds)
        self.assertIn("autoremove", subcmds)
        self.assertIn("clean", subcmds)
        self.assertIn("check-update", subcmds)

        # Check options
        opt_names = [name for opt in spec.global_options for name in opt.names]
        self.assertIn("--assumeyes", opt_names)
        self.assertIn("-y", opt_names)
        self.assertIn("--refresh", opt_names)

    def test_yum_alias_registered(self):
        spec = self.registry.get_spec("yum")
        self.assertIsNotNone(spec)
        subcmds = [s.name for s in spec.subcommands]
        self.assertIn("install", subcmds)

    def test_pacman_spec_registered(self):
        spec = self.registry.get_spec("pacman")
        self.assertIsNotNone(spec)
        self.assertEqual(spec.name, "pacman")

        subcmds = [s.name for s in spec.subcommands]
        self.assertIn("-S", subcmds)
        self.assertIn("-Syu", subcmds)
        self.assertIn("-R", subcmds)
        self.assertIn("-Ss", subcmds)
        self.assertIn("-Si", subcmds)
        self.assertIn("-Q", subcmds)
        self.assertIn("-Sc", subcmds)
        self.assertIn("-U", subcmds)

        # Check options
        opt_names = [name for opt in spec.global_options for name in opt.names]
        self.assertIn("--noconfirm", opt_names)
        self.assertIn("--needed", opt_names)


class TestMultiDistroCommandManager(unittest.TestCase):
    """Tests for multidistro package manager commands in CommandManager."""

    def setUp(self):
        self.builtins = {cmd.id: cmd for cmd in get_builtin_commands()}

    def test_builtin_commands_contain_multidistro_managers(self):
        self.assertIn("builtin_pacman", self.builtins)
        self.assertIn("builtin_apt", self.builtins)
        self.assertIn("builtin_dnf", self.builtins)

    def test_builtin_apt_attributes(self):
        apt_cmd = self.builtins["builtin_apt"]
        self.assertEqual(apt_cmd.name, "APT")
        self.assertIn("apt", apt_cmd.command_template)
        field_ids = [f.id for f in apt_cmd.form_fields]
        self.assertIn("action", field_ids)
        self.assertIn("package", field_ids)

        # Check that action dropdown has key Debian operations
        action_field = next(f for f in apt_cmd.form_fields if f.id == "action")
        action_keys = [opt[0] for opt in action_field.options]
        self.assertIn("install", action_keys)
        self.assertIn("remove", action_keys)
        self.assertIn("update", action_keys)
        self.assertIn("upgrade -y", action_keys)
        self.assertIn("autoremove", action_keys)
        self.assertIn("clean", action_keys)

    def test_builtin_dnf_attributes(self):
        dnf_cmd = self.builtins["builtin_dnf"]
        self.assertEqual(dnf_cmd.name, "DNF")
        self.assertIn("dnf", dnf_cmd.command_template)
        field_ids = [f.id for f in dnf_cmd.form_fields]
        self.assertIn("action", field_ids)
        self.assertIn("package", field_ids)

        # Check that action dropdown has key Fedora/RHEL operations
        action_field = next(f for f in dnf_cmd.form_fields if f.id == "action")
        action_keys = [opt[0] for opt in action_field.options]
        self.assertIn("install", action_keys)
        self.assertIn("remove", action_keys)
        self.assertIn("upgrade -y", action_keys)
        self.assertIn("check-update", action_keys)
        self.assertIn("autoremove", action_keys)
        self.assertIn("clean all", action_keys)

    def test_command_execution_dialog_builders(self):
        dialog = CommandFormDialog.__new__(CommandFormDialog)

        # APT builder tests
        cmd_update = dialog._build_apt_command({"action": "update", "package": ""})
        self.assertEqual(cmd_update, "sudo apt update")

        cmd_install = dialog._build_apt_command({"action": "install", "package": "git"})
        self.assertEqual(cmd_install, "sudo apt install git")

        # DNF builder tests
        cmd_dnf_check = dialog._build_dnf_command({"action": "check-update", "package": ""})
        self.assertEqual(cmd_dnf_check, "sudo dnf check-update")

        cmd_dnf_install = dialog._build_dnf_command({"action": "install", "package": "nginx"})
        self.assertEqual(cmd_dnf_install, "sudo dnf install nginx")

        # Pacman builder tests
        cmd_pacman_sync = dialog._build_pacman_command({"action": "-Syu", "package": ""})
        self.assertEqual(cmd_pacman_sync, "sudo pacman -Syu")

        cmd_pacman_orphans = dialog._build_pacman_command({"action": "__remove_orphans__", "package": ""})
        self.assertEqual(cmd_pacman_orphans, "sudo pacman -Rns $(pacman -Qdtq)")


class TestMultiDistroAdminTools(unittest.TestCase):
    """Tests for AdminTools package operations with Arch/Fedora/Debian."""

    def setUp(self):
        self.admin_tools = AdminTools()

    @patch("onyxsh.agent.admin_tools.get_package_manager", return_value="apt")
    def test_admin_tools_apt(self, mock_pm):
        valid, err, argv, dry_run = self.admin_tools.validate_action("pkg.clean_cache", {})
        self.assertTrue(valid)
        self.assertEqual(argv, ["apt", "clean"])

        valid, err, argv, dry_run = self.admin_tools.validate_action("pkg.autoremove", {})
        self.assertTrue(valid)
        self.assertEqual(argv, ["apt", "autoremove"])
        self.assertEqual(dry_run, ["apt", "autoremove", "--dry-run"])

    @patch("onyxsh.agent.admin_tools.get_package_manager", return_value="dnf")
    def test_admin_tools_dnf(self, mock_pm):
        valid, err, argv, dry_run = self.admin_tools.validate_action("pkg.clean_cache", {})
        self.assertTrue(valid)
        self.assertEqual(argv, ["dnf", "clean"])

        valid, err, argv, dry_run = self.admin_tools.validate_action("pkg.autoremove", {})
        self.assertTrue(valid)
        self.assertEqual(argv, ["dnf", "autoremove"])

    @patch("onyxsh.agent.admin_tools.get_package_manager", return_value="pacman")
    def test_admin_tools_pacman(self, mock_pm):
        valid, err, argv, dry_run = self.admin_tools.validate_action("pkg.clean_cache", {})
        self.assertTrue(valid)
        self.assertEqual(argv, ["pacman", "-Sc", "--noconfirm"])
        self.assertEqual(dry_run, ["pacman", "-Sc"])

        valid, err, argv, dry_run = self.admin_tools.validate_action("pkg.autoremove", {})
        self.assertTrue(valid)
        self.assertEqual(argv, ["pacman", "-Rns", "--noconfirm"])
        self.assertEqual(dry_run, ["pacman", "-Rns", "--dryrun"])


if __name__ == "__main__":
    unittest.main()
