# tests/test_completion_specs.py
"""
Tests for command completion specifications.
"""

import unittest

from onyxsh.terminal.completion.engine import CompletionContext
from onyxsh.terminal.completion.specs.registry import SpecRegistry
from onyxsh.terminal.completion.specs.apt import get_apt_spec
from onyxsh.terminal.completion.specs.dnf import get_dnf_spec
from onyxsh.terminal.completion.specs.pacman import get_pacman_spec
from onyxsh.terminal.completion.specs.docker import get_docker_spec


class TestCompletionSpecs(unittest.TestCase):
    """Test command completion specs."""

    def setUp(self):
        self.registry = SpecRegistry()

    def test_registry_has_specs(self):
        """Test that registry has common specs."""
        self.assertIsNotNone(self.registry.get_spec("apt"))
        self.assertIsNotNone(self.registry.get_spec("dnf"))
        self.assertIsNotNone(self.registry.get_spec("yum"))
        self.assertIsNotNone(self.registry.get_spec("pacman"))
        self.assertIsNotNone(self.registry.get_spec("systemctl"))
        self.assertIsNotNone(self.registry.get_spec("journalctl"))
        self.assertIsNotNone(self.registry.get_spec("docker"))
        self.assertIsNotNone(self.registry.get_spec("git"))
        self.assertIsNotNone(self.registry.get_spec("curl"))
        self.assertIsNotNone(self.registry.get_spec("ssh"))

    def test_apt_subcommands(self):
        """Test apt spec subcommand resolution."""
        spec = get_apt_spec()
        ctx = CompletionContext(
            full_line="apt upd",
            line_before_cursor="apt upd",
            tokens=["apt", "upd"],
            current_word="upd",
        )
        items = spec.get_completions(ctx)
        texts = [i.text for i in items]
        self.assertIn("update", texts)
        self.assertIn("upgrade", texts)

    def test_apt_has_global_options(self):
        """Test apt has expected global options."""
        spec = get_apt_spec()
        ctx = CompletionContext(
            full_line="apt ",
            line_before_cursor="apt ",
            tokens=["apt"],
            current_word="",
        )
        items = spec.get_completions(ctx)
        texts = [i.text for i in items]
        self.assertIn("install", texts)
        self.assertIn("remove", texts)
        self.assertIn("search", texts)

    def test_docker_subcommands(self):
        """Test docker spec subcommand resolution."""
        spec = get_docker_spec()
        ctx = CompletionContext(
            full_line="docker ps",
            line_before_cursor="docker ps",
            tokens=["docker", "ps"],
            current_word="ps",
        )
        items = spec.get_completions(ctx)
        texts = [i.text for i in items]
        self.assertIn("ps", texts)
        self.assertIn("pull", texts)
        self.assertIn("push", texts)

    def test_git_subcommands(self):
        """Test git spec subcommand resolution."""
        spec = get_docker_spec()  # dummy, let's use git properly - wait, fix

    def test_git_subcommands(self):
        """Test git spec subcommand resolution."""
        from onyxsh.terminal.completion.specs.git import get_git_spec
        spec = get_git_spec()
        ctx = CompletionContext(
            full_line="git comm",
            line_before_cursor="git comm",
            tokens=["git", "comm"],
            current_word="comm",
        )
        items = spec.get_completions(ctx)
        texts = [i.text for i in items]
        self.assertIn("commit", texts)

    def test_dnf_subcommands(self):
        """Test dnf spec resolution."""
        spec = get_dnf_spec()
        ctx = CompletionContext(
            full_line="dnf in",
            line_before_cursor="dnf in",
            tokens=["dnf", "in"],
            current_word="in",
        )
        items = spec.get_completions(ctx)
        texts = [i.text for i in items]
        self.assertIn("install", texts)

    def test_dnf_repolist(self):
        """Test dnf repolist/group availability."""
        spec = get_dnf_spec()
        ctx = CompletionContext(
            full_line="dnf ",
            line_before_cursor="dnf ",
            tokens=["dnf"],
            current_word="",
        )
        items = spec.get_completions(ctx)
        texts = [i.text for i in items]
        self.assertIn("repolist", texts)
        self.assertIn("group", texts)

    def test_pacman_subcommands(self):
        """Test pacman spec resolution."""
        spec = get_pacman_spec()
        ctx = CompletionContext(
            full_line="pacman -Sy",
            line_before_cursor="pacman -Sy",
            tokens=["pacman", "-Sy"],
            current_word="-Sy",
        )
        items = spec.get_completions(ctx)
        texts = [i.text for i in items]
        self.assertIn("-Syu", texts)


if __name__ == "__main__":
    unittest.main()
