"""
Tests for SettingsManager async save ordering and flushing.

Guards against:
- Settings lost on shutdown because the daemon writer thread was never joined.
- An older snapshot overwriting a newer one when writer threads run out of order.
- Dead duplicate method definitions shadowing each other.
"""

import ast
import json
import tempfile
import threading
import unittest
from pathlib import Path

from onyxsh.settings.manager import SettingsManager

MANAGER_SRC = (
    Path(__file__).resolve().parent.parent / "src" / "onyxsh" / "settings" / "manager.py"
)


def _read_settings(path: Path) -> dict:
    with open(path, encoding="utf-8") as f:
        return json.load(f)["settings"]


class TestSettingsSaveFlush(unittest.TestCase):
    def test_wait_for_pending_saves_without_any_save(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            manager = SettingsManager(settings_file=Path(tmpdir) / "settings.json")
            manager.wait_for_pending_saves()
            self.assertTrue(manager.wait_for_pending_saves(timeout=0.1))

    def test_flush_persists_latest_value(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            settings_file = Path(tmpdir) / "settings.json"
            manager = SettingsManager(settings_file=settings_file)
            for size in (10, 11, 12, 13, 14):
                manager.set("font", f"Monospace {size}")
            self.assertTrue(manager.wait_for_pending_saves())
            self.assertEqual(_read_settings(settings_file)["font"], "Monospace 14")

    def test_stale_snapshot_does_not_overwrite_newer(self):
        """Force the older writer to run last; the newer value must survive."""
        with tempfile.TemporaryDirectory() as tmpdir:
            settings_file = Path(tmpdir) / "settings.json"
            manager = SettingsManager(settings_file=settings_file)
            manager.wait_for_pending_saves()

            captured = []
            real_thread = threading.Thread

            class DeferredThread(real_thread):
                def start(self_inner):
                    captured.append(self_inner)

            import onyxsh.settings.manager as mod

            original = mod.threading.Thread
            mod.threading.Thread = DeferredThread
            try:
                manager.set("font", "Monospace 20")  # seq N   (older)
                manager.set("font", "Monospace 30")  # seq N+1 (newer)
            finally:
                mod.threading.Thread = original

            self.assertGreaterEqual(len(captured), 2)
            older, newer = captured[-2], captured[-1]
            # Run newer first, then the stale one.
            real_thread.start(newer)
            newer.join()
            real_thread.start(older)
            older.join()

            self.assertEqual(_read_settings(settings_file)["font"], "Monospace 30")


class TestNoDuplicateMethods(unittest.TestCase):
    def test_settings_manager_has_no_redefined_methods(self):
        tree = ast.parse(MANAGER_SRC.read_text(encoding="utf-8"))
        for cls in (n for n in tree.body if isinstance(n, ast.ClassDef)):
            seen = {}
            for node in cls.body:
                if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    has_decorator = bool(node.decorator_list)
                    if node.name in seen and not has_decorator:
                        self.fail(
                            f"{cls.name}.{node.name} defined twice "
                            f"(lines {seen[node.name]} and {node.lineno})"
                        )
                    seen[node.name] = node.lineno


if __name__ == "__main__":
    unittest.main()
