# onyxsh/terminal/completion/specs/__init__.py
"""Command specifications package for intelligent autocomplete."""

from .base import CommandSpec, OptionSpec, SubcommandSpec
from .registry import SpecRegistry, get_spec_registry

__all__ = [
    "CommandSpec",
    "SubcommandSpec",
    "OptionSpec",
    "SpecRegistry",
    "get_spec_registry",
]
