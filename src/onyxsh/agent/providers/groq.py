"""Groq LLM provider implementation."""

from __future__ import annotations

import json
from typing import Any, Callable, Optional

from ...utils.logger import get_logger
from .base import LLMProvider


class GroqProvider(LLMProvider):
    """Provider for Groq Cloud API."""

    name = "groq"
    trust = "remote"
    DEFAULT_MODEL = "openai/gpt-oss-120b"
    _discovered_models_cache: dict[str, tuple[float, list[str]]] = {}

    def __init__(self, config: dict[str, Any]) -> None:
        super().__init__(config)
        self.model = self.model or self.DEFAULT_MODEL
        self.logger = get_logger("onyxsh.agent.providers.groq")

    @classmethod
    def discover_available_models(cls, api_key: str, force_refresh: bool = False) -> list[str]:
        """Discovers available models for the given API key from Groq API."""
        import time
        import requests

        default_candidates = [
            "openai/gpt-oss-120b",
            "qwen/qwen3.8-27b",
            "openai/gpt-oss-20b",
            "qwen/qwen3.6-27b",
            "groq/compound",
            "groq/compound-mini",
        ]

        if not api_key:
            return default_candidates

        now = time.time()
        if not force_refresh and api_key in cls._discovered_models_cache:
            cached_time, cached_models = cls._discovered_models_cache[api_key]
            if now - cached_time < 3600:
                return cached_models

        url = "https://api.groq.com/openai/v1/models"
        headers = {
            "Authorization": f"Bearer {api_key.strip()}",
            "Content-Type": "application/json",
        }

        try:
            resp = requests.get(url, headers=headers, timeout=10)
            if resp.status_code == 200:
                data = resp.json()
                models = [
                    m["id"]
                    for m in data.get("data", [])
                    if isinstance(m, dict)
                    and "id" in m
                    and not any(x in m["id"] for x in ["whisper", "guard", "orpheus"])
                ]
                if models:
                    cls._discovered_models_cache[api_key] = (now, models)
                    return models
        except Exception:
            pass

        return default_candidates

    def complete(
        self,
        messages: list[dict[str, str]],
        tools_schema: Optional[list[dict[str, Any]]] = None,
    ) -> str:
        if not self.api_key:
            raise ValueError("Groq API Key não configurada. Configure em Preferências > IA.")

        import requests

        url = "https://api.groq.com/openai/v1/chat/completions"
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }

        payload: dict[str, Any] = {
            "model": self.model,
            "messages": messages,
            "temperature": 0.2,
        }

        # Groq requires 'json' to be in the prompt messages if response_format is json_object
        has_json = any("json" in (m.get("content") or "").lower() for m in messages)
        if has_json:
            payload["response_format"] = {"type": "json_object"}

        try:
            response = requests.post(url, headers=headers, json=payload, timeout=60)
        except requests.RequestException as e:
            raise RuntimeError(f"Erro ao conectar com Groq API: {e}") from e

        if response.status_code != 200:
            raise RuntimeError(f"Erro da Groq API ({response.status_code}): {response.text}")

        data = response.json()
        choices = data.get("choices", [])
        if not choices:
            raise RuntimeError("Resposta vazia da Groq API.")

        msg = choices[0].get("message", {})
        content = msg.get("content")
        if content is None:
            content = msg.get("reasoning", "")
        return (content or "").strip()

    def complete_stream(
        self,
        messages: list[dict[str, str]],
        callback: Callable[[str, bool], None],
        tools_schema: Optional[list[dict[str, Any]]] = None,
    ) -> str:
        if not self.api_key:
            raise ValueError("Groq API Key não configurada.")

        import requests

        url = "https://api.groq.com/openai/v1/chat/completions"
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }

        payload: dict[str, Any] = {
            "model": self.model,
            "messages": messages,
            "temperature": 0.2,
            "stream": True,
        }

        has_json = any("json" in (m.get("content") or "").lower() for m in messages)
        if has_json:
            payload["response_format"] = {"type": "json_object"}

        try:
            response = requests.post(url, headers=headers, json=payload, timeout=60, stream=True)
        except requests.RequestException as e:
            raise RuntimeError(f"Erro ao conectar com Groq API: {e}") from e

        if response.status_code != 200:
            raise RuntimeError(f"Erro da Groq API ({response.status_code}): {response.text}")

        accumulated = []
        for line in response.iter_lines():
            if not line:
                continue
            line_str = line.decode("utf-8", errors="replace")
            if line_str.startswith("data: "):
                data_content = line_str[6:].strip()
                if data_content == "[DONE]":
                    break
                try:
                    chunk_json = json.loads(data_content)
                    delta = chunk_json.get("choices", [{}])[0].get("delta", {})
                    content_chunk = delta.get("content", "")
                    if content_chunk:
                        accumulated.append(content_chunk)
                        callback(content_chunk, False)
                except Exception:
                    continue

        callback("", True)
        return "".join(accumulated).strip()

