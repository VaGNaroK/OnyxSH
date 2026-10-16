"""
Regression tests for bugs found via static analysis (undefined names / scoping).

Covers:
- GitCommitAssistant._worker: gettext `_` shadowed by tuple unpacking (UnboundLocalError).
- TabManager._recreate_widget_from_node: missing `os` import (NameError) and unsafe `cd` quoting.
- Deferred closures inside `except ... as e` blocks referencing `e` after it is deleted.
"""

import ast
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

SRC_ROOT = Path(__file__).resolve().parent.parent / "src" / "onyxsh"


class TestGitAssistantGettextShadowing(unittest.TestCase):
    def test_not_a_repo_reports_translated_message(self):
        from onyxsh.terminal.git_assistant import GitCommitAssistant

        assistant = GitCommitAssistant(MagicMock())
        callback = MagicMock()
        with patch("onyxsh.terminal.git_assistant.get_repo_root", return_value=None):
            assistant._worker("/tmp", "conventional", "pt-BR", callback)

        callback.assert_called_once()
        message, error = callback.call_args[0]
        self.assertIsNone(message)
        self.assertNotIn("local variable", error)
        self.assertIn("Git", error)

    def test_no_changes_reports_translated_message(self):
        from onyxsh.terminal.git_assistant import GitCommitAssistant

        assistant = GitCommitAssistant(MagicMock())
        callback = MagicMock()
        with patch("onyxsh.terminal.git_assistant.get_repo_root", return_value="/repo"), \
             patch("onyxsh.terminal.git_assistant.get_git_status", return_value={}), \
             patch("onyxsh.terminal.git_assistant.get_git_diff", return_value=""):
            assistant._worker("/repo", "conventional", "pt-BR", callback)

        _msg, error = callback.call_args[0]
        self.assertNotIn("local variable", error)
        self.assertIn("commit", error)


class TestTabRestoreFromNode(unittest.TestCase):
    def _make_manager(self):
        from onyxsh.terminal.tabs import TabManager

        mgr = TabManager.__new__(TabManager)
        mgr.logger = MagicMock()
        mgr.terminal_manager = MagicMock()
        mgr.terminal_manager.create_local_terminal.return_value = None
        mgr.terminal_manager.create_ssh_terminal.return_value = None
        return mgr

    def test_local_node_with_working_dir_does_not_raise(self):
        mgr = self._make_manager()
        node = {"type": "terminal", "session_type": "local", "working_dir": "/tmp"}
        result = mgr._recreate_widget_from_node(node)
        self.assertIsNone(result)
        mgr.terminal_manager.create_local_terminal.assert_called_once_with(
            title="Terminal", working_directory="/tmp"
        )

    def test_local_node_with_invalid_working_dir_falls_back(self):
        mgr = self._make_manager()
        node = {"type": "terminal", "working_dir": "/nonexistent/__onyx__"}
        mgr._recreate_widget_from_node(node)  # missing session_type must not KeyError
        mgr.terminal_manager.create_local_terminal.assert_called_once_with(
            title="Terminal", working_directory=None
        )

    def test_ssh_initial_command_is_shell_quoted(self):
        mgr = self._make_manager()
        session = MagicMock()
        session.name = "srv"
        session.is_ssh.return_value = True
        mgr.terminal_manager.settings_manager.get.return_value = True
        node = {
            "type": "terminal",
            "session_type": "ssh",
            "session_name": "srv",
            "working_dir": '/home/u/a"; rm -rf ~; "',
        }
        with patch("onyxsh.terminal.tabs.SessionItem") as session_cls:
            session_cls.from_dict.return_value = session
            node["session_data"] = {"name": "srv"}
            mgr._recreate_widget_from_node(node)

        kwargs = mgr.terminal_manager.create_ssh_terminal.call_args.kwargs
        self.assertEqual(
            kwargs["initial_command"], "cd '/home/u/a\"; rm -rf ~; \"'"
        )


class _ExceptVarClosureVisitor(ast.NodeVisitor):
    """Flags nested functions/lambdas inside `except ... as NAME` that read NAME."""

    def __init__(self):
        self.violations = []

    def visit_ExceptHandler(self, node):
        if node.name:
            for stmt in node.body:
                for sub in ast.walk(stmt):
                    if isinstance(sub, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda)):
                        for inner in ast.walk(sub):
                            if (
                                isinstance(inner, ast.Name)
                                and inner.id == node.name
                                and isinstance(inner.ctx, ast.Load)
                            ):
                                self.violations.append(inner.lineno)
        self.generic_visit(node)



class TestNoDeferredExceptVariableCapture(unittest.TestCase):
    def test_filemanager_closures_do_not_capture_deleted_exception(self):
        target = SRC_ROOT / "filemanager" / "manager.py"
        tree = ast.parse(target.read_text(encoding="utf-8"))
        visitor = _ExceptVarClosureVisitor()
        visitor.visit(tree)
        self.assertEqual(
            visitor.violations,
            [],
            f"Closures referencing a deleted except-variable at lines {visitor.violations}",
        )


class TestSecurityGettextShadowing(unittest.TestCase):
    def test_validate_ssh_key_file_with_unspecified_error_raises_translated_message(self):
        from onyxsh.utils.exceptions import SSHKeyError
        from onyxsh.utils.security import SSHKeyValidator, validate_ssh_key_file
        from onyxsh.utils.translation_utils import _

        with patch.object(SSHKeyValidator, "read_and_validate_ssh_key", return_value=(False, None, None)):
            with self.assertRaises(SSHKeyError) as ctx:
                validate_ssh_key_file("/tmp/fake_key")
            self.assertIn(_("Unknown validation error"), str(ctx.exception))


class TestSessionTreeMoveGettextShadowing(unittest.TestCase):
    def test_perform_move_failure_shows_error_dialog_without_type_error(self):
        from onyxsh.sessions.tree import SessionTreeView
        from onyxsh.utils.translation_utils import _

        tree = SessionTreeView.__new__(SessionTreeView)
        tree.logger = MagicMock()
        tree.operations = MagicMock()
        tree.parent_window = MagicMock()
        session_mock = MagicMock()
        tree.operations.find_session_by_name_and_path.return_value = (session_mock, 42)
        tree.operations.move_session_to_folder.return_value = MagicMock(
            success=False, message="Folder destination invalid"
        )

        tree._perform_move("session|test_server|/root", "/target")
        tree.parent_window._show_error_dialog.assert_called_once()
        title, msg = tree.parent_window._show_error_dialog.call_args[0]
        self.assertIn(_("Move Error"), title)
        self.assertEqual(msg, "Folder destination invalid")

    def test_perform_move_exception_shows_error_dialog_without_type_error(self):
        from onyxsh.sessions.tree import SessionTreeView
        from onyxsh.utils.translation_utils import _

        tree = SessionTreeView.__new__(SessionTreeView)
        tree.logger = MagicMock()
        tree.operations = MagicMock()
        tree.parent_window = MagicMock()
        tree.operations.find_session_by_name_and_path.side_effect = RuntimeError("Storage crash")

        tree._perform_move("session|test_server|/root", "/target")
        tree.parent_window._show_error_dialog.assert_called_once()
        title, msg = tree.parent_window._show_error_dialog.call_args[0]
        self.assertIn(_("Move Error"), title)
        self.assertIn("Storage crash", msg)


class TestHighlightDialogPatternChangedGettextShadowing(unittest.TestCase):
    def test_on_pattern_changed_updates_description_without_type_error(self):
        from onyxsh.ui.dialogs.highlight_dialog import RuleEditDialog

        dialog = RuleEditDialog.__new__(RuleEditDialog)
        dialog._validate_input = MagicMock()
        dialog._pattern_text_view = MagicMock()
        dialog._pattern_text_view.get_text.return_value = r"(\w+) (\d+)"
        dialog._manager = MagicMock()
        dialog._manager.validate_pattern.return_value = (True, None)
        dialog._colors_group = MagicMock()

        dialog._on_pattern_changed(None)

        dialog._colors_group.set_description.assert_called_once()
        desc = dialog._colors_group.set_description.call_args[0][0]
        self.assertIn("2", desc)


class TestFileManagerPromptForNewItem(unittest.TestCase):
    def test_prompt_for_new_item_is_callable_alias(self):
        from onyxsh.filemanager.manager import FileManager

        self.assertTrue(hasattr(FileManager, "_prompt_for_new_item"))
        self.assertIs(FileManager._prompt_for_new_item, FileManager._show_create_named_item_dialog)

    def test_on_create_folder_action_invokes_dialog(self):
        from onyxsh.filemanager.manager import FileManager
        from onyxsh.utils.translation_utils import _

        fm = FileManager.__new__(FileManager)
        fm.current_path = "/home/user"
        fm._show_create_named_item_dialog = MagicMock()

        fm._on_create_folder_action()

        fm._show_create_named_item_dialog.assert_called_once()
        kwargs = fm._show_create_named_item_dialog.call_args.kwargs
        self.assertEqual(kwargs["heading"], _("Create Folder"))
        self.assertTrue(callable(kwargs["callback"]))

    def test_on_create_file_action_invokes_dialog(self):
        from onyxsh.filemanager.manager import FileManager
        from onyxsh.utils.translation_utils import _

        fm = FileManager.__new__(FileManager)
        fm.current_path = "/home/user"
        fm._show_create_named_item_dialog = MagicMock()

        fm._on_create_file_action()

        fm._show_create_named_item_dialog.assert_called_once()
        kwargs = fm._show_create_named_item_dialog.call_args.kwargs
        self.assertEqual(kwargs["heading"], _("Create File"))
        self.assertTrue(callable(kwargs["callback"]))


class TestAgentOrchestratorFSArgMapping(unittest.IsolatedAsyncioTestCase):
    async def test_fs_tools_argv_parameter_mapping(self):
        from onyxsh.agent.models import ActionStep
        from onyxsh.agent.orchestrator import AgentOrchestrator

        orchestrator = AgentOrchestrator.__new__(AgentOrchestrator)
        orchestrator.tool_registry = MagicMock()
        orchestrator.tool_registry.invoke = unittest.mock.AsyncMock()

        # 1. fs.read_file
        step1 = ActionStep(
            step_id="step1",
            tool="fs.read_file",
            argv=["/etc/hosts"],
            description="read file",
            risk=0,
        )
        await orchestrator.execute_step(step1)
        orchestrator.tool_registry.invoke.assert_awaited_with(
            "fs.read_file", {"path": "/etc/hosts"}, ctx=unittest.mock.ANY
        )

        # 2. fs.search_text
        step2 = ActionStep(
            step_id="step2",
            tool="fs.search_text",
            argv=["/var/log", "ERROR"],
            description="search error",
            risk=0,
        )
        await orchestrator.execute_step(step2)
        orchestrator.tool_registry.invoke.assert_awaited_with(
            "fs.search_text", {"path": "/var/log", "pattern": "ERROR"}, ctx=unittest.mock.ANY
        )

        # 3. fs.propose_edit
        step3 = ActionStep(
            step_id="step3",
            tool="fs.propose_edit",
            argv=["/tmp/main.py", "print('hello')"],
            description="propose edit",
            risk=1,
        )
        await orchestrator.execute_step(step3)
        orchestrator.tool_registry.invoke.assert_awaited_with(
            "fs.propose_edit",
            {"path": "/tmp/main.py", "new_content": "print('hello')"},
            ctx=unittest.mock.ANY,
        )

        # 4. fs.apply_staged
        step4 = ActionStep(
            step_id="step4",
            tool="fs.apply_staged",
            argv=["/tmp/main.py", "/staging/main.py.staged"],
            description="apply staged",
            risk=1,
        )
        await orchestrator.execute_step(step4)
        orchestrator.tool_registry.invoke.assert_awaited_with(
            "fs.apply_staged",
            {"path": "/tmp/main.py", "target_path": "/tmp/main.py", "staged_path": "/staging/main.py.staged"},
            ctx=unittest.mock.ANY,
        )


class TestXDGPlatformDirectories(unittest.TestCase):
    def test_platform_info_and_helpers_honor_xdg_env_vars(self):
        from onyxsh.utils.platform import (
            PlatformInfo,
            get_cache_directory,
            get_config_directory,
            get_data_directory,
        )

        custom_env = {
            "XDG_CONFIG_HOME": "/custom/config",
            "XDG_DATA_HOME": "/custom/data",
            "XDG_CACHE_HOME": "/custom/cache",
        }
        with patch.dict("os.environ", custom_env):
            info = PlatformInfo()
            self.assertEqual(info.config_dir, Path("/custom/config/onyxsh"))
            self.assertEqual(info.data_dir, Path("/custom/data/onyxsh"))
            self.assertEqual(info.cache_dir, Path("/custom/cache/onyxsh"))

            self.assertEqual(get_config_directory(), Path("/custom/config/onyxsh"))
            self.assertEqual(get_data_directory(), Path("/custom/data/onyxsh"))
            self.assertEqual(get_cache_directory(), Path("/custom/cache/onyxsh"))


if __name__ == "__main__":
    unittest.main()

