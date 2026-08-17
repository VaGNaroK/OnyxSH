"""LLM Providers package for OnyxSH."""

from __future__ import annotations

from typing import Any

from .base import LLMProvider
from .gemini import GeminiProvider
from .groq import GroqProvider
from .ollama import OllamaProvider
from .openrouter import OpenRouterProvider


PROVIDERS: dict[str, type[LLMProvider]] = {
    "gemini": GeminiProvider,
    "groq": GroqProvider,
    "openrouter": OpenRouterProvider,
    "ollama": OllamaProvider,
    "local": OllamaProvider,
}


def get_provider(name: str, config: dict[str, Any]) -> LLMProvider:
    """Instantiate provider by name with configuration dictionary."""
    normalized_name = (name or "").strip().lower()
    provider_cls = PROVIDERS.get(normalized_name)
    if not provider_cls:
        raise ValueError(
            f"Provedor de IA desconhecido '{name}'. Disponíveis: {list(PROVIDERS.keys())}"
        )
    return provider_cls(config)


__all__ = [
    "LLMProvider",
    "GeminiProvider",
    "GroqProvider",
    "OpenRouterProvider",
    "OllamaProvider",
    "get_provider",
    "PROVIDERS",
]
