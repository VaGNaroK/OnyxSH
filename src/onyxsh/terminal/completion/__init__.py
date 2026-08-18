# onyxsh/terminal/completion/__init__.py
"""
Intelligent Autocomplete and Command Suggestion Package for OnyxSH.
"""

from .engine import CompletionEngine, get_completion_engine
from .models import (
    CompletionContext,
    CompletionItem,
    CompletionSource,
    CompletionType,
)
from .popup import CompletionPopup

__all__ = [
    "CompletionItem",
    "CompletionContext",
    "CompletionType",
    "CompletionSource",
    "CompletionEngine",
    "get_completion_engine",
    "CompletionPopup",
]
