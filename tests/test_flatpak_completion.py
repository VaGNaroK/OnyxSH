"""Unit tests for Flatpak Completion Specs, Command Manager integration, and PostVerifier."""

import unittest

from onyxsh.agent.verifier import PostVerifier
from onyxsh.data.command_manager_models import get_builtin_commands
from onyxsh.terminal.completion.engine import CompletionEngine
from onyxsh.terminal.completion.specs.registry import SpecRegistry
from onyxsh.ui.dialogs.command_manager_dialog import CommandFormDialog


class TestFlatpakCompletionSpec(unittest.TestCase):
    """Tests for Flatpak command completion specifications."""

    def setUp(self):
        self.registry = SpecRegistry()
        self.engine = CompletionEngine(spec_registry=self.registry)

    def test_flatpak_spec_registered(self):
        spec = self.registry.get_spec("flatpak")
        self.assertIsNotNone(spec, "Flatpak spec should be registered in SpecRegistry")
        self.assertEqual(spec.name, "flatpak")

        subcmds = [s.name for s in spec.subcommands]
        expected_subcmds = [
            "install",
            "uninstall",
            "update",
            "list",
            "search",
            "info",
            "run",
            "remotes",
            "remote-add",
            "remote-delete",
            "remote-modify",
            "remote-ls",
            "ps",
            "kill",
            "override",
            "permission-show",
            "permission-reset",
            "repair",
            "history",
            "config",
        ]
        for subcmd in expected_subcmds:
            self.assertIn(subcmd, subcmds, f"Subcommand '{subcmd}' missing in Flatpak spec")

    def test_flatpak_subcommand_descriptions_present(self):
        """Ensure all Flatpak subcommands and their options have descriptive text."""
        spec = self.registry.get_spec("flatpak")
        self.assertIsNotNone(spec)
        self.assertTrue(spec.description, "Command description should not be empty")

        for subcmd in spec.subcommands:
            self.assertTrue(subcmd.description, f"Subcommand '{subcmd.name}' has empty description")
            for opt in subcmd.options:
                self.assertTrue(opt.description, f"Option '{opt.names}' under '{subcmd.name}' has empty description")

        for opt in spec.global_options:
            self.assertTrue(opt.description, f"Global option '{opt.names}' has empty description")

    def test_flatpak_global_options(self):
        spec = self.registry.get_spec("flatpak")
        self.assertIsNotNone(spec)
        global_opts = [name for opt in spec.global_options for name in opt.names]
        self.assertIn("-y", global_opts)
        self.assertIn("--assumeyes", global_opts)
        self.assertIn("--noninteractive", global_opts)
        self.assertIn("--user", global_opts)
        self.assertIn("--system", global_opts)
        self.assertIn("-v", global_opts)
        self.assertIn("--verbose", global_opts)

    def test_flatpak_install_subcommand_options(self):
        spec = self.registry.get_spec("flatpak")
        install_cmd = next((s for s in spec.subcommands if s.name == "install"), None)
        self.assertIsNotNone(install_cmd)
        opt_names = [name for opt in install_cmd.options for name in opt.names]
        self.assertIn("-y", opt_names)
        self.assertIn("--user", opt_names)
        self.assertIn("--reinstall", opt_names)
        self.assertIn("--from", opt_names)
        self.assertIn("--bundle", opt_names)

    def test_flatpak_uninstall_aliases(self):
        spec = self.registry.get_spec("flatpak")
        uninstall_cmd = next((s for s in spec.subcommands if s.name == "uninstall"), None)
        self.assertIsNotNone(uninstall_cmd)
        self.assertIn("remove", uninstall_cmd.aliases)

    def test_engine_completion_subcommands(self):
        items = self.engine.get_completions("flatpak ins")
        texts = [i.text for i in items]
        self.assertIn("install", texts)

        items_upd = self.engine.get_completions("flatpak upd")
        texts_upd = [i.text for i in items_upd]
        self.assertIn("update", texts_upd)

        items_rem = self.engine.get_completions("flatpak rem")
        texts_rem = [i.text for i in items_rem]
        self.assertIn("remotes", texts_rem)
        self.assertIn("remote-add", texts_rem)

    def test_engine_completion_options(self):
        items = self.engine.get_completions("flatpak install --ass")
        texts = [i.text for i in items]
        self.assertIn("--assumeyes", texts)

        items_unused = self.engine.get_completions("flatpak uninstall --unu")
        texts_unused = [i.text for i in items_unused]
        self.assertIn("--unused", texts_unused)


class TestFlatpakCommandManager(unittest.TestCase):
    """Tests for Flatpak integration in the Command Manager UI models and builders."""

    def setUp(self):
        self.builtins = {cmd.id: cmd for cmd in get_builtin_commands()}

    def test_builtin_flatpak_registered(self):
        self.assertIn("builtin_flatpak", self.builtins)
        cmd = self.builtins["builtin_flatpak"]
        self.assertEqual(cmd.name, "Flatpak")
        self.assertIn("flatpak", cmd.command_template)

        field_ids = [f.id for f in cmd.form_fields]
        self.assertIn("action", field_ids)
        self.assertIn("package", field_ids)

        action_field = next(f for f in cmd.form_fields if f.id == "action")
        action_keys = [opt[0] for opt in action_field.options]
        self.assertIn("install", action_keys)
        self.assertIn("uninstall", action_keys)
        self.assertIn("update", action_keys)
        self.assertIn("list --app", action_keys)
        self.assertIn("search", action_keys)
        self.assertIn("repair", action_keys)

    def test_build_flatpak_command_with_app(self):
        dialog = CommandFormDialog.__new__(CommandFormDialog)
        built = dialog._build_flatpak_command({"action": "install", "package": "org.gnome.Calculator"})
        self.assertEqual(built, "flatpak install org.gnome.Calculator")

        built_run = dialog._build_flatpak_command({"action": "run", "package": "io.github.vagnarok.OnyxSH"})
        self.assertEqual(built_run, "flatpak run io.github.vagnarok.OnyxSH")

    def test_build_flatpak_command_no_app_required(self):
        dialog = CommandFormDialog.__new__(CommandFormDialog)
        built_update = dialog._build_flatpak_command({"action": "update", "package": ""})
        self.assertEqual(built_update, "flatpak update")

        built_list = dialog._build_flatpak_command({"action": "list --app", "package": ""})
        self.assertEqual(built_list, "flatpak list --app")

        built_repair = dialog._build_flatpak_command({"action": "repair", "package": ""})
        self.assertEqual(built_repair, "flatpak repair")

        built_remotes = dialog._build_flatpak_command({"action": "remotes", "package": ""})
        self.assertEqual(built_remotes, "flatpak remotes")


class TestFlatpakPostVerifier(unittest.TestCase):
    """Tests for Flatpak post-execution verification rules in PostVerifier."""

    def setUp(self):
        self.verifier = PostVerifier(is_flatpak=False)

    def test_infer_flatpak_install_verification(self):
        checks = self.verifier.infer_verifications(["flatpak install flathub org.gnome.Calculator"])
        self.assertTrue(len(checks) > 0)
        check = checks[0]
        self.assertEqual(check.check_type, "package_installed")
        self.assertIn("flatpak info org.gnome.Calculator", check.check_command)
        self.assertEqual(check.expected_exit_code, 0)

    def test_infer_flatpak_install_with_flags(self):
        checks = self.verifier.infer_verifications(["flatpak install -y --user flathub io.github.vagnarok.OnyxSH"])
        self.assertTrue(len(checks) > 0)
        check = checks[0]
        self.assertIn("flatpak info io.github.vagnarok.OnyxSH", check.check_command)

    def test_infer_flatpak_uninstall_verification(self):
        checks = self.verifier.infer_verifications(["flatpak uninstall -y org.gnome.Calculator"])
        self.assertTrue(len(checks) > 0)
        check = checks[0]
        self.assertEqual(check.check_type, "package_absent")
        self.assertIn("flatpak info org.gnome.Calculator", check.check_command)
        self.assertEqual(check.expected_exit_code, 1)

    def test_infer_flatpak_update_verification(self):
        checks = self.verifier.infer_verifications(["flatpak update -y"])
        self.assertTrue(len(checks) > 0)
        check = checks[0]
        self.assertEqual(check.check_type, "package_installed")
        self.assertIn("flatpak list --app", check.check_command)


if __name__ == "__main__":
    unittest.main()
