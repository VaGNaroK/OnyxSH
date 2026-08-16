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
        self.context_size = int(config.get("context_size") or 8192)
        self.logger = get_logger("zashterminal.agent.providers.ollama")

    def _get_native_base_url(self) -> str:
        """Get the base URL for native Ollama API endpoints (strip /v1 suffix)."""
        if self.base_url.endswith("/v1"):
            return self.base_url[:-3]
        return self.base_url

    def _get_headers(self) -> dict[str, str]:
        headers = {"Content-Type": "application/json"}
        if self.api_key:
            headers["Authorization"] = f"Bearer {self.api_key}"
        return headers

    def preload(self, keep_alive: int | str = -1) -> bool:
        """
        Preload the model into GPU VRAM in background.

        Using keep_alive=-1 retains the model indefinitely in VRAM until explicitly unloaded.
        """
        import requests

        native_url = f"{self._get_native_base_url()}/api/generate"
        payload = {
            "model": self.model,
            "keep_alive": keep_alive,
            "options": {
                "num_ctx": self.context_size,
            },
        }
        try:
            self.logger.info(f"Preloading local model {self.model} into VRAM (keep_alive={keep_alive}, num_ctx={self.context_size})...")
            resp = requests.post(native_url, json=payload, headers=self._get_headers(), timeout=30)
            if resp.status_code == 200:
                self.logger.info(f"Local model {self.model} preloaded into VRAM successfully.")
                return True
            self.logger.warning(f"Preload returned status {resp.status_code}: {resp.text}")
            return False
        except Exception as e:
            self.logger.warning(f"Failed to preload local model {self.model} into VRAM: {e}")
            return False

    def unload(self) -> bool:
        """
        Unload the model from GPU VRAM immediately.

        Using keep_alive=0 causes Ollama to release the model from memory.
        """
        import requests

        native_url = f"{self._get_native_base_url()}/api/generate"
        payload = {
            "model": self.model,
            "keep_alive": 0,
        }
        try:
            self.logger.info(f"Unloading local model {self.model} from VRAM...")
            resp = requests.post(native_url, json=payload, headers=self._get_headers(), timeout=5)
            if resp.status_code == 200:
                self.logger.info(f"Local model {self.model} unloaded from VRAM successfully.")
                return True
            self.logger.warning(f"Unload returned status {resp.status_code}: {resp.text}")
            return False
        except Exception as e:
            self.logger.warning(f"Failed to unload local model {self.model} from VRAM: {e}")
            return False

    def is_loaded(self) -> bool:
        """Check if the model is currently active/loaded in VRAM."""
        import requests

        native_url = f"{self._get_native_base_url()}/api/ps"
        try:
            resp = requests.get(native_url, headers=self._get_headers(), timeout=3)
            if resp.status_code == 200:
                data = resp.json()
                models = data.get("models", [])
                for m in models:
                    name = m.get("name", "")
                    model_id = m.get("model", "")
                    if self.model in (name, model_id) or name.startswith(f"{self.model}:"):
                        return True
            return False
        except Exception:
            return False

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
            "options": {
                "num_predict": 4096,
                "num_ctx": self.context_size,
            },
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
            "options": {
                "num_predict": 4096,
                "num_ctx": self.context_size,
            },
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
