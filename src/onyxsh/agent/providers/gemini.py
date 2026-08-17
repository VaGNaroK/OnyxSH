"""Google Gemini LLM provider implementation."""

from __future__ import annotations

import json
from typing import Any, Callable, Optional

from ...utils.logger import get_logger
from .base import LLMProvider


class GeminiProvider(LLMProvider):
    """Provider for Google Gemini API."""

    name = "gemini"
    trust = "remote"
    DEFAULT_MODEL = "gemini-2.5-flash"

    def __init__(self, config: dict[str, Any]) -> None:
        super().__init__(config)
        self.model = self.model or self.DEFAULT_MODEL
        self.logger = get_logger("onyxsh.agent.providers.gemini")

    def _convert_messages_to_gemini(
        self,
        messages: list[dict[str, str]],
    ) -> tuple[Optional[dict[str, Any]], list[dict[str, Any]]]:
        system_instruction = None
        contents: list[dict[str, Any]] = []

        for msg in messages:
            role = msg.get("role", "user")
            content = msg.get("content", "")
            if role == "system":
                system_instruction = {"parts": [{"text": content}]}
            else:
                gemini_role = "user" if role == "user" else "model"
                contents.append({
                    "role": gemini_role,
                    "parts": [{"text": content}],
                })

        return system_instruction, contents

    def complete(
        self,
        messages: list[dict[str, str]],
        tools_schema: Optional[list[dict[str, Any]]] = None,
    ) -> str:
        if not self.api_key:
            raise ValueError("Gemini API Key não configurada. Configure em Preferências > IA.")

        import requests

        url = f"https://generativelanguage.googleapis.com/v1beta/models/{self.model}:generateContent?key={self.api_key}"
        headers = {"Content-Type": "application/json"}

        system_instruction, contents = self._convert_messages_to_gemini(messages)

        payload: dict[str, Any] = {
            "contents": contents,
            "generationConfig": {
                "temperature": 0.2,
                "responseMimeType": "application/json",
            },
        }
        if system_instruction:
            payload["systemInstruction"] = system_instruction

        try:
            response = requests.post(url, headers=headers, json=payload, timeout=60)
        except requests.RequestException as e:
            raise RuntimeError(f"Erro ao conectar com Gemini API: {e}") from e

        if response.status_code != 200:
            raise RuntimeError(f"Erro da Gemini API ({response.status_code}): {response.text}")

        data = response.json()
        candidates = data.get("candidates", [])
        if not candidates:
            raise RuntimeError("Nenhuma resposta retornada pela Gemini API.")

        parts = candidates[0].get("content", {}).get("parts", [])
        if not parts:
            raise RuntimeError("Conteúdo vazio retornado pela Gemini API.")

        return parts[0].get("text", "").strip()

    def complete_stream(
        self,
        messages: list[dict[str, str]],
        callback: Callable[[str, bool], None],
        tools_schema: Optional[list[dict[str, Any]]] = None,
    ) -> str:
        if not self.api_key:
            raise ValueError("Gemini API Key não configurada.")

        import requests

        url = f"https://generativelanguage.googleapis.com/v1beta/models/{self.model}:streamGenerateContent?alt=sse&key={self.api_key}"
        headers = {"Content-Type": "application/json"}

        system_instruction, contents = self._convert_messages_to_gemini(messages)

        payload: dict[str, Any] = {
            "contents": contents,
            "generationConfig": {
                "temperature": 0.2,
            },
        }
        if system_instruction:
            payload["systemInstruction"] = system_instruction

        try:
            response = requests.post(url, headers=headers, json=payload, timeout=60, stream=True)
        except requests.RequestException as e:
            raise RuntimeError(f"Erro ao conectar com Gemini API: {e}") from e

        if response.status_code != 200:
            raise RuntimeError(f"Erro da Gemini API ({response.status_code}): {response.text}")

        accumulated = []
        for line in response.iter_lines():
            if not line:
                continue
            line_str = line.decode("utf-8", errors="replace")
            if line_str.startswith("data: "):
                data_content = line_str[6:].strip()
                try:
                    chunk_json = json.loads(data_content)
                    candidates = chunk_json.get("candidates", [])
                    if candidates:
                        parts = candidates[0].get("content", {}).get("parts", [])
                        if parts:
                            part_text = parts[0].get("text", "")
                            if part_text:
                                accumulated.append(part_text)
                                callback(part_text, False)
                except Exception:
                    continue

        callback("", True)
        return "".join(accumulated).strip()
