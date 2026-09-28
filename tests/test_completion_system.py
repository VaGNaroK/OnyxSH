# tests/test_completion_system.py
"""Unit tests for PATH-based system executable detection."""

import os
import stat
import tempfile
import unittest

from onyxsh.terminal.completion.engine import CompletionEngine


class FakeSettings:
    def __init__(self, values):
        self.values = values

    def get(self, key, default=None):
        return self.values.get(key, default)


class TestCompletionSystem(unittest.TestCase):
    def test_prefix_filters_path_executables(self):
        with tempfile.TemporaryDirectory() as tmp:
            for name in ("onyxshtest-alpha", "onyxshtest-beta", "onyxshtest-other"):
                path = os.path.join(tmp, name)
                with open(path, "w", encoding="utf-8") as f:
                    f.write("#!/bin/sh\n")
                os.chmod(path, os.stat(path).st_mode | stat.S_IXUSR)
            engine = CompletionEngine()
            old_path = os.environ.get("PATH", "")
            os.environ["PATH"] = tmp + os.pathsep + old_path
            try:
                engine._system_cache_names = []
                items = engine.get_completions("onyxshtest-a")
                texts = [i.text for i in items]
                self.assertIn("onyxshtest-alpha", texts)
                self.assertNotIn("onyxshtest-other", texts)
                items_all = engine.get_completions("onyxshtest-")
                texts_all = [i.text for i in items_all]
                self.assertIn("onyxshtest-alpha", texts_all)
                self.assertIn("onyxshtest-beta", texts_all)
            finally:
                os.environ["PATH"] = old_path

    def test_specs_rank_above_system(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "apt")
            with open(path, "w", encoding="utf-8") as f:
                f.write("#!/bin/sh\n")
            os.chmod(path, os.stat(path).st_mode | stat.S_IXUSR)
            engine = CompletionEngine()
            old_path = os.environ.get("PATH", "")
            os.environ["PATH"] = tmp + os.pathsep + old_path
            try:
                engine._system_cache_names = []
                items = engine.get_completions("ap")
                texts = [i.text for i in items]
                self.assertIn("apt", texts)
                self.assertEqual(texts.count("apt"), 1)
            finally:
                os.environ["PATH"] = old_path

    def test_disabled_toggle_returns_no_system(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "zzfake-xyz")
            with open(path, "w", encoding="utf-8") as f:
                f.write("#!/bin/sh\n")
            os.chmod(path, os.stat(path).st_mode | stat.S_IXUSR)
            settings = FakeSettings(
                {
                    "autocomplete_enabled": True,
                    "autocomplete_specs_enabled": True,
                    "autocomplete_history_enabled": False,
                    "autocomplete_snippets_enabled": False,
                    "autocomplete_system_enabled": False,
                }
            )
            engine = CompletionEngine(settings_manager=settings)
            old_path = os.environ.get("PATH", "")
            os.environ["PATH"] = tmp + os.pathsep + old_path
            try:
                engine._system_cache_names = []
                items = engine.get_completions("zzfake-xy")
                texts = [i.text for i in items]
                self.assertNotIn("zzfake-xyz", texts)
            finally:
                os.environ["PATH"] = old_path

    def test_invalid_path_dirs_do_not_raise(self):
        engine = CompletionEngine()
        old_path = os.environ.get("PATH", "")
        os.environ["PATH"] = "/nonexistent-onyxsh-dir" + os.pathsep + "/also-missing"
        try:
            engine._system_cache_names = []
            engine._system_cache_key = ""
            names = engine._scan_path_executables()
            self.assertIsInstance(names, list)
        finally:
            os.environ["PATH"] = old_path


if __name__ == "__main__":
    unittest.main()
