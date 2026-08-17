"""OpenRouter LLM provider implementation."""

from __future__ import annotations

import json
from typing import Any, Callable, Optional

from ...utils.logger import get_logger
from .base import LLMProvider


class OpenRouterProvider(LLMProvider):
    """Provider for OpenRouter multi-model gateway."""

    name = "openrouter"
    trust = "remote"
    DEFAULT_MODEL = "openrouter/auto"

    def __init__(self, config: dict[str, Any]) -> None:
        super().__init__(config)
        self.model = self.model or self.DEFAULT_MODEL
        self.site_url = config.get("openrouter_site_url", "https://github.com/VaGNaroK/OnyxSH")
        self.site_name = config.get("openrouter_site_name", "OnyxSH")
        self.logger = get_logger("onyxsh.agent.providers.openrouter")

    def _get_headers(self) -> dict[str, str]:
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
            "HTTP-Referer": self.site_url,
            "X-Title": self.site_name,
        }
        return headers

    def complete(
        self,
        messages: list[dict[str, str]],
        tools_schema: Optional[list[dict[str, Any]]] = None,
    ) -> str:
        if not self.api_key:
            raise ValueError("OpenRouter API Key não configurada. Configure em Preferências > IA.")

        import requests

        url = "https://openrouter.ai/api/v1/chat/completions"
        headers = self._get_headers()

        payload: dict[str, Any] = {
            "model": self.model,
            "messages": messages,
            "temperature": 0.2,
            "response_format": {"type": "json_object"},
        }

        try:
            response = requests.post(url, headers=headers, json=payload, timeout=60)
        except requests.RequestException as e:
            raise RuntimeError(f"Erro ao conectar com OpenRouter API: {e}") from e

        if response.status_code != 200:
            raise RuntimeError(f"Erro da OpenRouter API ({response.status_code}): {response.text}")

        data = response.json()
        choices = data.get("choices", [])
        if not choices:
            raise RuntimeError("Nenhuma escolha retornada pelo OpenRouter.")

        return choices[0]["message"]["content"].strip()

    def complete_stream(
        self,
        messages: list[dict[str, str]],
        callback: Callable[[str, bool], None],
        tools_schema: Optional[list[dict[str, Any]]] = None,
    ) -> str:
        if not self.api_key:
            raise ValueError("OpenRouter API Key não configurada.")

        import requests

        url = "https://openrouter.ai/api/v1/chat/completions"
        headers = self._get_headers()

        payload: dict[str, Any] = {
            "model": self.model,
            "messages": messages,
            "temperature": 0.2,
            "stream": True,
        }

        try:
            response = requests.post(url, headers=headers, json=payload, timeout=60, stream=True)
        except requests.RequestException as e:
            raise RuntimeError(f"Erro ao conectar com OpenRouter API: {e}") from e

        if response.status_code != 200:
            raise RuntimeError(f"Erro da OpenRouter API ({response.status_code}): {response.text}")

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
