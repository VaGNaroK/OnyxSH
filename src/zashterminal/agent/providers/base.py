"""Base abstract class for LLM providers."""

from __future__ import annotations

from abc import ABC, abstractmethod
from typing import Any, Callable, Optional


class LLMProvider(ABC):
    """Abstract interface for LLM providers supporting structured completions."""

    name: str = "base"
    trust: str = "remote"  # "local" | "remote"

    def __init__(self, config: dict[str, Any]) -> None:
        self.config = config
        self.model = config.get("model", "")
        self.api_key = config.get("api_key", "")

    @abstractmethod
    def complete(
        self,
        messages: list[dict[str, str]],
        tools_schema: Optional[list[dict[str, Any]]] = None,
    ) -> str:
        """
        Execute a completion request synchronously or via thread pool.

        Args:
            messages: Formatted message sequence [{"role": "user"|"system"|"assistant", "content": "..."}].
            tools_schema: Optional schema describing available tools.

        Returns:
            str: Raw LLM output string (expected to be JSON or text).
        """
        raise NotImplementedError

    def complete_stream(
        self,
        messages: list[dict[str, str]],
        callback: Callable[[str, bool], None],
        tools_schema: Optional[list[dict[str, Any]]] = None,
    ) -> str:
        """
        Stream completion chunks via callback.

        Default implementation falls back to standard complete().
        """
        res = self.complete(messages, tools_schema)
        callback(res, True)
        return res

    def preload(self) -> bool:
        """
        Preload the model into memory/VRAM for immediate availability.

        Default implementation is a no-op for remote cloud providers.
        Returns True if successful or not needed.
        """
        return True

    def unload(self) -> bool:
        """
        Unload the model from memory/VRAM to free resources.

        Default implementation is a no-op for remote cloud providers.
        Returns True if successful or not needed.
        """
        return True

    def is_loaded(self) -> bool:
        """
        Check if the model is currently active/loaded in memory/VRAM.

        Default implementation returns True for remote cloud providers.
        """
        return True
