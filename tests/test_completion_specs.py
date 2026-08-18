# tests/test_completion_specs.py
"""Unit tests for declarative command specifications."""

import unittest

from onyxsh.terminal.completion.models import (
    CompletionContext,
    CompletionSource,
    CompletionType,
)
from onyxsh.terminal.completion.specs.apt import get_apt_spec
from onyxsh.terminal.completion.specs.docker import get_docker_spec
from onyxsh.terminal.completion.specs.git import get_git_spec
from onyxsh.terminal.completion.specs.registry import get_spec_registry
from onyxsh.terminal.completion.specs.systemd import (
    get_journalctl_spec,
    get_systemctl_spec,
)


class TestCompletionSpecs(unittest.TestCase):
    """Test suite for command specs."""

    def setUp(self):
        self.registry = get_spec_registry()

    def test_registry_registration(self):
        """Test that core commands are registered."""
        self.assertIsNotNone(self.registry.get_spec("apt"))
        self.assertIsNotNone(self.registry.get_spec("systemctl"))
        self.assertIsNotNone(self.registry.get_spec("journalctl"))
        self.assertIsNotNone(self.registry.get_spec("docker"))
        self.assertIsNotNone(self.registry.get_spec("git"))
        self.assertIsNotNone(self.registry.get_spec("ssh"))
        self.assertIsNotNone(self.registry.get_spec("sudo"))

    def test_apt_subcommands(self):
        """Test apt spec subcommand resolution."""
        spec = get_apt_spec()
        ctx = CompletionContext(
            full_line="apt in",
            line_before_cursor="apt in",
            tokens=["apt", "in"],
            current_word="in",
        )
        items = spec.get_completions(ctx)
        texts = [i.text for i in items]
        self.assertIn("install", texts)

    def test_systemctl_subcommands(self):
        """Test systemctl spec subcommand resolution."""
        spec = get_systemctl_spec()
        ctx = CompletionContext(
            full_line="systemctl res",
            line_before_cursor="systemctl res",
            tokens=["systemctl", "res"],
            current_word="res",
        )
        items = spec.get_completions(ctx)
        texts = [i.text for i in items]
        self.assertIn("restart", texts)

    def test_docker_subcommands(self):
        """Test docker spec resolution."""
        spec = get_docker_spec()
        ctx = CompletionContext(
            full_line="docker r",
            line_before_cursor="docker r",
            tokens=["docker", "r"],
            current_word="r",
        )
        items = spec.get_completions(ctx)
        texts = [i.text for i in items]
        self.assertIn("run", texts)
        self.assertIn("rm", texts)

    def test_git_subcommands(self):
        """Test git spec resolution."""
        spec = get_git_spec()
        ctx = CompletionContext(
            full_line="git com",
            line_before_cursor="git com",
            tokens=["git", "com"],
            current_word="com",
        )
        items = spec.get_completions(ctx)
        texts = [i.text for i in items]
        self.assertIn("commit", texts)


if __name__ == "__main__":
    unittest.main()
