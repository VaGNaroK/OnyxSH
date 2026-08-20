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
    _discovered_models_cache: dict[str, tuple[float, list[str]]] = {}

    def __init__(self, config: dict[str, Any]) -> None:
        super().__init__(config)
        self.logger = get_logger("onyxsh.agent.providers.gemini")
        self.model = self.model or self.DEFAULT_MODEL

    @classmethod
    def discover_available_models(cls, api_key: str, force_refresh: bool = False) -> list[str]:
        """Discovers available models for the given API key from Google AI Studio.

        Returns a list of model IDs supporting generateContent (e.g. ['gemini-2.5-flash', ...]).
        """
        import time
        import requests

        default_candidates = [
            "gemini-2.5-flash",
            "gemini-2.0-flash",
            "gemini-2.0-flash-exp",
            "gemini-1.5-flash",
            "gemini-1.5-pro",
            "gemini-1.5-flash-8b",
        ]

        if not api_key:
            return default_candidates

        now = time.time()
        if not force_refresh and api_key in cls._discovered_models_cache:
            cached_time, cached_models = cls._discovered_models_cache[api_key]
            if now - cached_time < 3600:  # Cache for 1 hour
                return cached_models

        url = "https://generativelanguage.googleapis.com/v1beta/models"
        headers = {
            "Content-Type": "application/json",
            "x-goog-api-key": api_key.strip(),
        }

        try:
            resp = requests.get(url, headers=headers, timeout=10)
            if resp.status_code == 200:
                data = resp.json()
                raw_models = data.get("models", [])
                supported = []
                for m in raw_models:
                    methods = m.get("supportedGenerationMethods", [])
                    if "generateContent" in methods:
                        name = m.get("name", "")
                        if name.startswith("models/"):
                            name = name[7:]
                        if name and "embedding" not in name.lower() and "aqa" not in name.lower():
                            supported.append(name)

                # Prioritize flash, then pro, newest first
                def _model_sort_key(name: str) -> int:
                    n = name.lower()
                    if "2.5-flash" in n:
                        return 0
                    if "2.0-flash" in n:
                        return 1
                    if "1.5-flash" in n:
                        return 2
                    if "2.5-pro" in n:
                        return 3
                    if "1.5-pro" in n:
                        return 4
                    return 10

                supported.sort(key=_model_sort_key)
                if supported:
                    cls._discovered_models_cache[api_key] = (now, supported)
                    return supported
            elif resp.status_code == 400 or resp.status_code == 403:
                # Key is invalid or unauthorized
                return []
        except Exception:
            pass

        # Fallback to standard candidate list if discovery request fails
        default_candidates = [
            "gemini-2.5-flash",
            "gemini-2.0-flash",
            "gemini-2.0-flash-exp",
            "gemini-1.5-flash",
            "gemini-1.5-pro",
            "gemini-1.5-flash-8b",
        ]
        return default_candidates

    def _get_candidate_models(self) -> list[str]:
        """Returns the list of candidate models, discovered dynamically when possible."""
        candidates = []
        if self.model:
            candidates.append(self.model)

        if self.api_key:
            discovered = self.discover_available_models(self.api_key)
            for m in discovered:
                if m not in candidates:
                    candidates.append(m)

        for m in (
            "gemini-2.5-flash",
            "gemini-2.0-flash",
            "gemini-2.0-flash-exp",
            "gemini-1.5-pro",
            "gemini-1.5-flash",
            "gemini-1.5-flash-8b",
        ):
            if m not in candidates:
                candidates.append(m)

        return candidates

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

        system_instruction, contents = self._convert_messages_to_gemini(messages)

        payload: dict[str, Any] = {
            "contents": contents,
            "generationConfig": {
                "temperature": 0.2,
            },
        }
        if system_instruction:
            payload["system_instruction"] = system_instruction

        headers = {
            "Content-Type": "application/json",
            "x-goog-api-key": self.api_key,
        }

        candidate_models = self._get_candidate_models()

        last_error = ""
        for model_name in candidate_models:
            url = f"https://generativelanguage.googleapis.com/v1beta/models/{model_name}:generateContent"
            self.logger.info(
                f"[GeminiProvider] Complete request: model='{model_name}', key_prefix='{self.api_key[:4]}...', msgs={len(messages)}"
            )
            try:
                response = requests.post(url, headers=headers, json=payload, timeout=120)
            except requests.RequestException as e:
                self.logger.error(f"[GeminiProvider] Network error with {model_name}: {e}")
                raise RuntimeError(f"Erro ao conectar com Gemini API: {e}") from e

            if response.status_code == 200:
                self.model = model_name
                data = response.json()
                candidates = data.get("candidates", [])
                if not candidates:
                    raise RuntimeError("Nenhuma resposta retornada pela Gemini API.")

                parts = candidates[0].get("content", {}).get("parts", [])
                if not parts:
                    raise RuntimeError("Conteúdo vazio retornado pela Gemini API.")

                return parts[0].get("text", "").strip()
            elif response.status_code == 404:
                self.logger.warning(f"[GeminiProvider] Model '{model_name}' returned 404, trying next candidate...")
                last_error = response.text
                continue
            else:
                self.logger.error(f"[GeminiProvider] API Error ({response.status_code}): {response.text}")
                raise RuntimeError(f"Erro da Gemini API ({response.status_code}): {response.text}")

        raise RuntimeError(f"Erro da Gemini API (404 em todos os modelos testados): {last_error}")

    def complete_stream(
        self,
        messages: list[dict[str, str]],
        callback: Callable[[str, bool], None],
        tools_schema: Optional[list[dict[str, Any]]] = None,
    ) -> str:
        if not self.api_key:
            raise ValueError("Gemini API Key não configurada.")

        import requests

        candidate_models = self._get_candidate_models()

        system_instruction, contents = self._convert_messages_to_gemini(messages)

        payload: dict[str, Any] = {
            "contents": contents,
            "generationConfig": {
                "temperature": 0.2,
            },
        }
        if system_instruction:
            payload["system_instruction"] = system_instruction

        headers = {
            "Content-Type": "application/json",
            "x-goog-api-key": self.api_key,
        }

        for model_name in candidate_models:
            url = f"https://generativelanguage.googleapis.com/v1beta/models/{model_name}:streamGenerateContent?alt=sse"
            self.logger.info(
                f"[GeminiProvider] Streaming request: model='{model_name}', key_prefix='{self.api_key[:4]}...', msgs={len(messages)}"
            )

            try:
                response = requests.post(url, headers=headers, json=payload, timeout=(15, 120), stream=True)
                if response.status_code == 404:
                    self.logger.warning(f"[GeminiProvider] Model '{model_name}' returned 404 on stream, trying next candidate...")
                    continue
                if response.status_code != 200:
                    self.logger.warning(
                        f"[GeminiProvider] Streaming returned HTTP {response.status_code}: {response.text}. Fallback to generateContent."
                    )
                    full_text = self.complete(messages, tools_schema)
                    callback(full_text, False)
                    callback("", True)
                    return full_text

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

                if accumulated:
                    self.model = model_name
                    callback("", True)
                    return "".join(accumulated).strip()
            except Exception as e:
                self.logger.warning(
                    f"[GeminiProvider] Streaming failed ({e}) on model '{model_name}', trying fallback..."
                )

        full_text = self.complete(messages, tools_schema)
        callback(full_text, False)
        callback("", True)
        return full_text
