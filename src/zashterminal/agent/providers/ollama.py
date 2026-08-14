"""Local Ollama / OpenAI-compatible LLM provider implementation."""

from __future__ import annotations

import json
from typing import Any, Callable, Optional

from ...utils.logger import get_logger
from .base import LLMProvider


class OllamaProvider(LLMProvider):
    """Provider for Local LLMs (Ollama, LM Studio, vLLM) with local trust scope."""

    name = "ollama"
    trust = "local"
    DEFAULT_MODEL = "llama3.2"
    DEFAULT_BASE_URL = "http://localhost:11434/v1"

    def __init__(self, config: dict[str, Any]) -> None:
        super().__init__(config)
        self.model = self.model or self.DEFAULT_MODEL
        self.base_url = (config.get("local_base_url") or self.DEFAULT_BASE_URL).rstrip("/")
        self.logger = get_logger("zashterminal.agent.providers.ollama")

    def _get_headers(self) -> dict[str, str]:
        headers = {"Content-Type": "application/json"}
        if self.api_key:
            headers["Authorization"] = f"Bearer {self.api_key}"
        return headers

    def complete(
        self,
        messages: list[dict[str, str]],
        tools_schema: Optional[list[dict[str, Any]]] = None,
    ) -> str:
        import requests

        url = f"{self.base_url}/chat/completions"
        headers = self._get_headers()

        payload: dict[str, Any] = {
            "model": self.model,
            "messages": messages,
            "temperature": 0.2,
            "max_tokens": 4096,
            "options": {"num_predict": 4096},
            "response_format": {"type": "json_object"},
            "stream": False,
        }

        try:
            response = requests.post(url, headers=headers, json=payload, timeout=120)
        except requests.RequestException as e:
            raise RuntimeError(f"Erro ao conectar com Ollama local ({url}): {e}") from e

        if response.status_code != 200:
            raise RuntimeError(f"Erro do serviço Ollama ({response.status_code}): {response.text}")

        data = response.json()
        choices = data.get("choices", [])
        if not choices:
            raise RuntimeError("Nenhuma escolha retornada pelo Ollama.")

        message = choices[0].get("message", {})
        content = message.get("content", "")
        return content.strip()

    def complete_stream(
        self,
        messages: list[dict[str, str]],
        callback: Callable[[str, bool], None],
        tools_schema: Optional[list[dict[str, Any]]] = None,
    ) -> str:
        import requests

        url = f"{self.base_url}/chat/completions"
        headers = self._get_headers()

        payload: dict[str, Any] = {
            "model": self.model,
            "messages": messages,
            "temperature": 0.2,
            "max_tokens": 4096,
            "options": {"num_predict": 4096},
            "stream": True,
        }

        try:
            response = requests.post(url, headers=headers, json=payload, timeout=120, stream=True)
        except requests.RequestException as e:
            raise RuntimeError(f"Erro ao conectar com Ollama local: {e}") from e

        if response.status_code != 200:
            raise RuntimeError(f"Erro do serviço Ollama ({response.status_code}): {response.text}")

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
