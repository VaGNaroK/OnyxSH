# onyxsh/terminal/ai_assistant.py

"""AI assistant integration for OnyxSH terminals."""

from __future__ import annotations

import json
import os
import re
import threading
import weakref
from typing import Any, Callable, Dict, List, Optional, Tuple

import gi

gi.require_version("GLib", "2.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, GLib, GObject

from ..data.ai_history_manager import get_ai_history_manager
from ..utils.logger import get_logger
from ..utils.translation_utils import _

# Lazy-loaded requests module (avoid import overhead on startup)
_requests_module = None


def _get_requests():
    """Get the requests module, importing lazily on first use."""
    global _requests_module
    if _requests_module is None:
        import requests
        _requests_module = requests
    return _requests_module


# Pre-compiled regex patterns for text formatting
_INLINE_CODE_PATTERN = re.compile(r"`([^`]+)`")
_PLUS_WHITESPACE_PATTERN = re.compile(r"\s*\+\s*")
_SEMICOLON_NEWLINE_PATTERN = re.compile(r";\s*\n")
_SEMICOLON_SENTENCE_PATTERN = re.compile(r";\s*(?=[A-ZÁÀÃÂÉÊÍÓÔÕÚÜÇ0-9])")
_BOLD_ASTERISK_PATTERN = re.compile(r"\*\*([^*]+)\*\*")
_BOLD_UNDERSCORE_PATTERN = re.compile(r"__([^_]+)__")
_NUMBERED_LIST_START_PATTERN = re.compile(r"(?<!\n)(\d+\.)")
_NUMBERED_LIST_FIX_PATTERN = re.compile(r"\n\s*(\d+)\s*(?=\n\d)\n")
_DASH_LIST_PATTERN = re.compile(r"\n\s*-\s+")
_ASTERISK_LIST_PATTERN = re.compile(r"\n\s*\*\s+")
_MULTIPLE_NEWLINES_PATTERN = re.compile(r"\n{3,}")


class TerminalAiAssistant(GObject.Object):
    """Coordinates conversations with an external AI service."""

    __gsignals__ = {
        # Signal emitted when streaming message chunks arrive
        # Args: (chunk: str, is_done: bool)
        "streaming-chunk": (GObject.SignalFlags.RUN_FIRST, None, (str, bool)),
        # Signal emitted when a full response is ready
        # Args: (reply: str, commands: list)
        "response-ready": (GObject.SignalFlags.RUN_FIRST, None, (str, object)),
        # Signal emitted on error
        # Args: (error_message: str)
        "error": (GObject.SignalFlags.RUN_FIRST, None, (str,)),
    }

    DEFAULT_GEMINI_MODEL = "gemini-2.5-flash"
    DEFAULT_GROQ_MODEL = "llama-3.1-8b-instant"
    DEFAULT_OPENROUTER_MODEL = "openrouter/polaris-alpha"
    DEFAULT_LOCAL_MODEL = "llama3.2"

    # PROMPT OTIMIZADO, DIDÁTICO E COM CONSCIÊNCIA DE TERMINAL
    _SYSTEM_PROMPT_TEMPLATE = (
        "You are an expert Linux systems engineer and interactive terminal assistant inside the OnyxSH emulator running on {os_context}."
        " Your mission is to provide clear, production-ready, safe, and logically ordered command-line solutions."
        "\n\n"
        "**CRITICAL RULES:**\n"
        "1. **OUTPUT FORMAT:** Respond with RAW JSON only. Do NOT wrap root response in markdown code blocks like ```json ... ```.\n"
        '2. **JSON STRUCTURE:** {{ "reply": "<comprehensive explanation with formatted markdown>", "commands": ["<cmd1>", "<cmd2>"] }}\n'
        "3. **LANGUAGE & FULL LOCALIZATION:** You MUST respond entirely and strictly in {language}. Every part of the response — including explanatory texts, markdown headings, transitional phrases, step lists (1, 2, 3...), code comments (# ...), log messages, and user-facing CLI output strings (`echo \"...\"`, `log_message ...`) — MUST be written in {language}. Never leave numbered steps, bullet points, or instructions in English.\n"
        "4. **TERMINAL AWARENESS:** The user is ALREADY working inside the OnyxSH terminal emulator. Never instruct the user to 'Open the terminal (Ctrl+Alt+T)' or open graphical desktop text editors unless explicitly asked. Always provide direct CLI solutions.\n"
        "5. **DYNAMIC PATHS & MODERN STANDARDS:** Always use `$HOME`, `~`, or relative paths. NEVER invent fake hardcoded user paths like `/home/usuario/` or `/home/user/`. Use modern system standards for {os_context} (e.g. `systemd`, `systemctl`, `journalctl`, `apt`, `flatpak`, `ip`, `ss`), avoiding deprecated legacy tools (`/etc/init.d/`, `update-rc.d`, `ifconfig`, `netstat`).\n"
        "6. **SCRIPT CREATION VIA CLI:** When providing commands to create files/scripts in the terminal, use atomic heredoc blocks (e.g. `cat << 'EOF' > ~/myscript.sh\\n#!/usr/bin/env bash\\n...\\nEOF\\nchmod +x ~/myscript.sh`) instead of splitting lines across multiple `echo >>` commands.\n"
        "7. **PACKAGE MANAGEMENT & UPDATES:** When upgrading system packages while excluding or holding specific packages (like the Linux kernel), use official native package manager holding mechanisms (e.g. `sudo apt-mark hold linux-image-generic linux-headers-generic && sudo apt upgrade -y` on Debian/Ubuntu/Mint, or `sudo dnf upgrade -x 'kernel*'` on Fedora) instead of fragile parsing hacks like `apt list --upgradable | grep ... > file`.\n"
        "\n"
        "**FIELD DETAILS & BEST PRACTICES:**\n"
        "- 'reply': Didactic, well-structured explanation in Markdown. When creating scripts, configurations, or programs, ALWAYS display the full, production-ready, well-commented script inside a Markdown code block with syntax highlighting so the user can easily read, verify, and understand it.\n"
        "- 'commands': A curated list of executable, standalone shell commands representing the single best recommended path in strictly chronological execution order (e.g., create script -> chmod +x -> execute). Do NOT wrap commands in `echo '...'` unless creating a file. If the user is asking an informational, theoretical, or comparative question without a specific task to perform (e.g. 'what are the differences between X and Y?', 'what are the ways to do Z?'), leave 'commands' as an empty array [].\n"
    )


    @staticmethod
    def _detect_os_context() -> str:
        """Detects the real host OS name and base to give context to the AI."""
        try:
            from ..utils.platform import detect_os_context
            return detect_os_context()
        except Exception:
            return "Linux"


    @classmethod
    def _get_system_prompt(cls) -> str:
        """Get the system prompt with the system's default language and OS context."""
        import locale

        try:
            # Get the system language
            lang_code = locale.getdefaultlocale()[0] or "en_US"
            # Map common locale codes to language names
            lang_map = {
                "pt": "Portuguese (Português do Brasil)",
                "en": "English",
                "es": "Spanish (Español)",
                "fr": "French (Français)",
                "de": "German (Deutsch)",
                "it": "Italian (Italiano)",
                "zh": "Chinese",
                "ja": "Japanese",
                "ko": "Korean",
                "ru": "Russian",
                "ar": "Arabic",
                "nl": "Dutch",
                "pl": "Polish",
                "tr": "Turkish",
                "uk": "Ukrainian",
                "cs": "Czech",
                "sv": "Swedish",
                "da": "Danish",
                "fi": "Finnish",
                "no": "Norwegian",
                "hu": "Hungarian",
                "ro": "Romanian",
                "bg": "Bulgarian",
                "el": "Greek",
                "he": "Hebrew",
                "hr": "Croatian",
                "sk": "Slovak",
                "et": "Estonian",
                "is": "Icelandic",
            }
            lang_prefix = lang_code.split("_")[0].lower()
            language = lang_map.get(lang_prefix, "English")
        except Exception:
            language = "English"

        os_context = cls._detect_os_context()

        return cls._SYSTEM_PROMPT_TEMPLATE.format(
            language=language, os_context=os_context
        )

    def __init__(self, window, settings_manager, terminal_manager):
        super().__init__()
        self.logger = get_logger("onyxsh.terminal.ai_assistant")
        self._window_ref = weakref.ref(window) if window is not None else None
        self.settings_manager = settings_manager
        self.terminal_manager = terminal_manager
        self._conversations: Dict[int, List[Dict[str, str]]] = {}
        self._terminal_refs: Dict[int, weakref.ReferenceType] = {}
        self._inflight: Dict[int, bool] = {}
        self._lock = threading.RLock()
        self._history_manager_instance = None  # Lazy loaded via property
        self._router_instance = None  # Lazy loaded via property
        # Callbacks for streaming updates
        self._streaming_callback: Optional[Callable[[str, bool], None]] = None

    @property
    def _history_manager(self):
        """Lazy load the AI history manager on first access."""
        if self._history_manager_instance is None:
            self._history_manager_instance = get_ai_history_manager()
        return self._history_manager_instance

    @property
    def router(self):
        """Lazy load the SmartRouter instance."""
        if self._router_instance is None:
            from ..agent.router import SmartRouter
            self._router_instance = SmartRouter(self.settings_manager)
        return self._router_instance

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------
    def is_enabled(self) -> bool:
        return self.settings_manager.get("ai_assistant_enabled", False)

    def is_offline_mode(self) -> bool:
        """Returns whether strictly offline local-only mode is active."""
        return bool(self.settings_manager.get("ai_assistant_offline_mode", False))

    def set_offline_mode(self, enabled: bool) -> None:
        """Enables or disables strictly offline local-only mode."""
        self.settings_manager.set("ai_assistant_offline_mode", bool(enabled))

    def get_routing_profile(self) -> str:
        """Returns the current routing profile string (e.g. 'auto', 'fast', 'advanced')."""
        return self.settings_manager.get("ai_routing_profile", "auto")

    def set_routing_profile(self, profile: str) -> None:
        """Sets the active routing profile."""
        self.settings_manager.set("ai_routing_profile", str(profile).lower())

    def missing_configuration(self, prompt: str = "") -> List[str]:
        missing = []
        if self.is_offline_mode():
            base_url = self.settings_manager.get("ai_local_base_url", "").strip()
            if not base_url:
                missing.append("base_url")
            return missing

        route = self.router.resolve_route(prompt=prompt, is_offline_mode=False)
        if not route.provider:
            missing.append("provider")
            return missing

        if route.provider in {"groq", "gemini", "openrouter"}:
            if not route.api_key:
                missing.append("api_key")
        elif route.provider == "local":
            base_url = self.settings_manager.get("ai_local_base_url", "").strip()
            if not base_url:
                missing.append("base_url")
        return missing

    def get_last_route_decision(self):
        """Returns the most recent RouteDecision calculated by the smart router."""
        return getattr(self, "_last_route_decision", None)

    def request_assistance(
        self,
        terminal,
        prompt: str,
        streaming_callback: Optional[Callable[[str, bool], None]] = None,
    ) -> bool:
        """Kick off an assistant request for the provided terminal."""
        if not prompt:
            return False
        if not self.is_enabled():
            self._queue_toast(
                "Enable the AI assistant in Preferences before requesting help."
            )
            return False
        try:
            terminal_id = self._ensure_terminal_reference(terminal)
        except ValueError:
            self._queue_toast("Unable to identify the active terminal.")
            return False

        with self._lock:
            if self._inflight.get(terminal_id):
                self._queue_toast(
                    "The assistant is still processing the previous request."
                )
                return False
            self._inflight[terminal_id] = True
            self._streaming_callback = streaming_callback

        # Save user message to history
        self._history_manager.add_user_message(prompt)

        worker = threading.Thread(
            target=self._process_request_thread, args=(terminal_id, prompt), daemon=True
        )
        worker.start()
        return True

    def request_assistance_simple(
        self,
        prompt: str,
        streaming_callback: Optional[Callable[[str, bool], None]] = None,
    ) -> bool:
        """
        Request assistance without a specific terminal context.
        Used by the AI overlay panel.
        """
        if not prompt:
            return False
        if not self.is_enabled():
            self._queue_toast(
                "Enable the AI assistant in Preferences before requesting help."
            )
            return False

        # Use a special terminal_id for non-terminal requests
        terminal_id = -1  # Special ID for overlay panel

        with self._lock:
            if self._inflight.get(terminal_id):
                self._queue_toast(
                    "The assistant is still processing the previous request."
                )
                return False
            self._inflight[terminal_id] = True
            self._streaming_callback = streaming_callback

        # Save user message to history
        self._history_manager.add_user_message(prompt)
        self.logger.info(f"[AIAssistant] request_assistance_simple: Starting worker thread for prompt='{prompt[:60]}...'")

        worker = threading.Thread(
            target=self._process_request_thread, args=(terminal_id, prompt), daemon=True
        )
        worker.start()
        return True

    def clear_conversation_for_terminal(self, terminal) -> None:
        terminal_id = getattr(terminal, "terminal_id", None)
        if terminal_id is None:
            return
        with self._lock:
            self._cleanup_terminal_state(terminal_id)

    def clear_all_conversations(self) -> None:
        with self._lock:
            self._conversations.clear()
            self._terminal_refs.clear()
            self._inflight.clear()

    def preload_model_async(self) -> None:
        """Trigger background preload of the configured local model into VRAM."""
        if not self.is_enabled():
            return
        if not self.settings_manager.get("ai_preload_local_model", True):
            return

        provider_name = self.settings_manager.get("ai_assistant_provider", "").strip().lower()
        if provider_name not in ("local", "ollama"):
            return

        try:
            from ..core.tasks import AsyncTaskManager
            AsyncTaskManager.get().submit_io(self._preload_model_worker)
        except Exception as e:
            self.logger.debug("Failed to submit async preload task: %s", e)

    def _preload_model_worker(self) -> None:
        """Worker executed in background IO thread pool to preload model."""
        try:
            provider_name = self.settings_manager.get("ai_assistant_provider", "").strip().lower()
            if provider_name in ("local", "ollama") or self.is_offline_mode():
                config = {
                    "provider": "local",
                    "model": self.settings_manager.get("ai_assistant_model", "").strip() or self.DEFAULT_LOCAL_MODEL,
                    "local_base_url": self.settings_manager.get("ai_local_base_url", "http://localhost:11434/v1").strip(),
                }
            else:
                config = self._load_configuration()
            if config.get("provider") in ("local", "ollama"):
                from ..agent.providers import get_provider
                provider = get_provider(config.get("provider", "ollama"), config)
                provider.preload()
        except Exception as e:
            self.logger.debug("Async model preload failed: %s", e)

    def unload_model(self) -> bool:
        """Unload local model from VRAM immediately."""
        if not self.settings_manager.get("ai_unload_on_exit", True):
            return True
        provider_name = self.settings_manager.get("ai_assistant_provider", "").strip().lower()
        if provider_name not in ("local", "ollama") and not self.is_offline_mode():
            return True
        try:
            config = {
                "provider": "local",
                "model": self.settings_manager.get("ai_assistant_model", "").strip() or self.DEFAULT_LOCAL_MODEL,
                "local_base_url": self.settings_manager.get("ai_local_base_url", "http://localhost:11434/v1").strip(),
            }
            from ..agent.providers import get_provider
            provider = get_provider(config.get("provider", "ollama"), config)
            return provider.unload()
        except Exception as e:
            self.logger.debug("Model unload failed: %s", e)
            return False

    def handle_setting_changed(self, key: str, old_value: Any, new_value: Any) -> None:
        if key == "ai_assistant_enabled":
            if not new_value:
                self.clear_all_conversations()
                self.unload_model()
            else:
                self.preload_model_async()
        elif key in {
            "ai_assistant_provider",
            "ai_assistant_model",
            "ai_local_base_url",
        }:
            self.clear_all_conversations()
            # If changing provider/model, unload previous if local and preload new if local
            if (old_value and str(old_value).lower() in ("local", "ollama")) or key in ("ai_assistant_model", "ai_local_base_url"):
                self.unload_model()
            if self.is_enabled() and self.settings_manager.get("ai_assistant_provider", "").lower() in ("local", "ollama"):
                self.preload_model_async()
        elif key in {
            "ai_assistant_api_key",
            "ai_openrouter_site_url",
            "ai_openrouter_site_name",
        }:
            self.clear_all_conversations()
        elif key == "ai_preload_local_model":
            if new_value:
                self.preload_model_async()
        elif key == "ai_context_size":
            if self.is_enabled() and self.settings_manager.get("ai_assistant_provider", "").lower() in ("local", "ollama"):
                self.preload_model_async()

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------
    def _ensure_terminal_reference(self, terminal) -> int:
        terminal_id = getattr(terminal, "terminal_id", None)
        if terminal_id is None:
            raise ValueError("terminal is missing a terminal_id attribute")

        if terminal_id not in self._terminal_refs:
            self._terminal_refs[terminal_id] = weakref.ref(
                terminal,
                lambda _ref, tid=terminal_id: self._cleanup_terminal_state(tid),
            )
        return terminal_id

    def _process_request_thread(self, terminal_id: int, prompt: str) -> None:
        try:
            messages = self._build_messages(terminal_id, prompt)
            config = self._load_configuration(prompt)
            primary_provider = config.get("provider", "local")
            self.logger.info(
                f"[AIAssistant] Dispatching to provider='{primary_provider}', model='{config.get('model')}', "
                f"streaming={bool(self._streaming_callback)}, key_set={bool(config.get('api_key'))}"
            )

            content = None
            fallback_notice = ""
            try:
                # 1. Primary remote request
                if primary_provider == "local" and self._streaming_callback:
                    content = self._perform_local_streaming_request(config, messages)
                elif primary_provider == "groq" and self._streaming_callback:
                    content = self._perform_groq_streaming_request(config, messages)
                else:
                    content = self._perform_request(config, messages)
            except Exception as primary_error:
                # If remote provider failed, attempt graceful fallback to local Ollama / LM Studio
                if primary_provider != "local":
                    self.logger.warning(
                        f"[AIAssistant] Primary provider '{primary_provider}' failed: {primary_error}. Attempting automatic fallback to local LLM..."
                    )
                    local_config = config.copy()
                    local_config["provider"] = "local"
                    local_config["model"] = self.settings_manager.get("ai_fast_model", self.DEFAULT_LOCAL_MODEL)
                    local_config["local_base_url"] = self.settings_manager.get(
                        "ai_local_base_url", "http://localhost:11434/v1"
                    )

                    # Update route decision state
                    if hasattr(self, "_last_route_decision") and self._last_route_decision:
                        self._last_route_decision.is_fallback = True
                        self._last_route_decision.fallback_reason = str(primary_error)
                        self._last_route_decision.provider = "local"
                        self._last_route_decision.model = local_config["model"]

                    err_summary = str(primary_error)
                    if "API key not valid" in err_summary or "API_KEY_INVALID" in err_summary or "400" in err_summary:
                        reason_msg = _("Chave de API do provedor em nuvem inválida ou expirada")
                    elif "Read timed out" in err_summary or "timeout" in err_summary.lower():
                        reason_msg = _("Tempo limite de conexão esgotado (timeout)")
                    elif "404" in err_summary:
                        reason_msg = _("Modelo em nuvem não disponível")
                    elif "429" in err_summary:
                        reason_msg = _("Limite de cota de requisições excedido")
                    else:
                        reason_msg = err_summary[:100]

                    prov_title = primary_provider.capitalize()
                    if primary_provider == "gemini":
                        prov_title = "Google Gemini"
                    elif primary_provider == "groq":
                        prov_title = "Groq"

                    fallback_notice = (
                        f"> ⚠️ **" + _("Modo de Contingência (Fallback Automático Ativado)") + "**\n"
                        f"> " + _("Não foi possível conectar ao **{provider}** ({reason}). A resposta foi gerada localmente através do **{model}**.").format(
                            provider=prov_title,
                            reason=reason_msg,
                            model=local_config["model"],
                        ) + "\n\n---\n\n"
                    )

                    # Stream notice immediately to UI if streaming
                    if self._streaming_callback:
                        GLib.idle_add(self._streaming_callback, fallback_notice, False)

                    try:
                        if self._streaming_callback:
                            content = self._perform_local_streaming_request(local_config, messages)
                        else:
                            content = self._perform_local_request(local_config, messages)
                    except Exception as local_err:
                        self.logger.error(f"[AIAssistant] Local fallback also failed: {local_err}")
                        raise primary_error from local_err
                else:
                    raise primary_error

            reply, commands, code_snippets = self._parse_assistant_payload(content)

            # Ensure fallback notice is prepended to reply
            if fallback_notice and not reply.startswith(fallback_notice.strip()[:30]):
                reply = fallback_notice + reply

            self._record_assistant_message(terminal_id, reply)

            # Save to history with commands (convert dicts to strings for storage)
            command_strings_for_history = [
                cmd.get("command", "") if isinstance(cmd, dict) else str(cmd)
                for cmd in (commands or [])
                if (isinstance(cmd, dict) and cmd.get("command")) or isinstance(cmd, str)
            ]

            used_provider = config.get("provider", "")
            used_model = config.get("model", "")
            if hasattr(self, "_last_route_decision") and self._last_route_decision:
                used_provider = self._last_route_decision.provider
                used_model = self._last_route_decision.model

            self._history_manager.add_assistant_message(
                reply,
                command_strings_for_history,
                model=used_model,
                provider=used_provider,
            )

            GLib.idle_add(
                self._display_assistant_reply,
                terminal_id,
                reply,
                commands,
                code_snippets,
            )
        except Exception as exc:  # pylint: disable=broad-except
            self.logger.error(f"[AIAssistant] Request failed: {exc}", exc_info=True)
            error_message = "Sorry, I couldn't complete the request: {}".format(exc)
            self._record_assistant_message(terminal_id, error_message)
            GLib.idle_add(
                self._display_error_reply,
                terminal_id,
                error_message,
            )
            # Emit error signal
            GLib.idle_add(self.emit, "error", error_message)
        finally:
            with self._lock:
                self._inflight.pop(terminal_id, None)
                self._streaming_callback = None

    def _build_messages(self, terminal_id: int, prompt: str) -> List[Dict[str, str]]:
        with self._lock:
            history = self._conversations.setdefault(terminal_id, [])
            history.append({"role": "user", "content": prompt})

            system_prompt = self._get_system_prompt()
            context_size = int(self.settings_manager.get("ai_context_size", 8192))

            # Approximate budget: reserve 1000 tokens for output generation
            # 1 token ~= 3.5 characters
            total_char_budget = max(4000, int((context_size - 1000) * 3.5))
            current_chars = len(system_prompt)

            # Fit as many recent messages from history as possible within budget
            selected_history: List[Dict[str, str]] = []
            for msg in reversed(history):
                msg_len = len(msg.get("content", ""))
                if current_chars + msg_len > total_char_budget and selected_history:
                    # Budget reached, stop adding older history
                    break
                selected_history.append(msg)
                current_chars += msg_len

            selected_history.reverse()

            messages: List[Dict[str, str]] = [
                {"role": "system", "content": system_prompt}
            ]
            messages.extend(selected_history)
            return messages

    def _load_configuration(self, prompt: str = "") -> Dict[str, Any]:
        route = self.router.resolve_route(
            prompt=prompt,
            is_offline_mode=self.is_offline_mode(),
        )
        self._last_route_decision = route

        config: Dict[str, Any] = {
            "provider": route.provider,
            "model": route.model,
            "api_key": route.api_key,
            "context_size": int(self.settings_manager.get("ai_context_size", 8192)),
            "openrouter_site_url": route.openrouter_site_url or self.settings_manager.get(
                "ai_openrouter_site_url", ""
            ).strip(),
            "openrouter_site_name": route.openrouter_site_name or self.settings_manager.get(
                "ai_openrouter_site_name", ""
            ).strip(),
            "local_base_url": route.base_url or self.settings_manager.get(
                "ai_local_base_url", "http://localhost:11434/v1"
            ).strip(),
            "route_decision": route,
        }

        if not config["provider"]:
            raise RuntimeError(
                _("Select a provider in Preferences > Terminal > AI Assistant.")
            )

        if config["provider"] == "groq" and not config["model"]:
            config["model"] = self.DEFAULT_GROQ_MODEL
        elif config["provider"] == "gemini" and not config["model"]:
            config["model"] = self.DEFAULT_GEMINI_MODEL
        elif config["provider"] == "openrouter" and not config["model"]:
            config["model"] = self.DEFAULT_OPENROUTER_MODEL
        elif config["provider"] == "local" and not config["model"]:
            config["model"] = self.DEFAULT_LOCAL_MODEL

        return config

    def _perform_request(
        self, config: Dict[str, str], messages: List[Dict[str, str]]
    ) -> str:
        provider_name = config.get("provider", "gemini")
        if self.is_offline_mode() and provider_name != "local":
            raise RuntimeError(
                _("Modo Estritamente Offline ativo: conexões com provedores de nuvem estão bloqueadas.")
            )
        try:
            from ..agent.providers import get_provider
            provider = get_provider(provider_name, config)
            return provider.complete(messages)
        except Exception as e:
            self.logger.warning("Agent provider dispatch failed, using fallback: %s", e)
            if provider_name == "groq":
                return self._perform_groq_request(config, messages)
            if provider_name == "gemini":
                return self._perform_gemini_request(config, messages)
            if provider_name == "openrouter":
                return self._perform_openrouter_request(config, messages)
            if provider_name == "local":
                return self._perform_local_request(config, messages)
            raise

    def _perform_streaming_request(
        self, config: Dict[str, str], messages: List[Dict[str, str]]
    ) -> str:
        """Perform a streaming request, sending chunks via callback."""
        provider_name = config.get("provider", "gemini")
        if self.is_offline_mode() and provider_name != "local":
            raise RuntimeError(
                _("Modo Estritamente Offline ativo: conexões com provedores de nuvem estão bloqueadas.")
            )
        try:
            from ..agent.providers import get_provider
            provider = get_provider(provider_name, config)
            if self._streaming_callback:
                return provider.complete_stream(messages, self._streaming_callback)
            return provider.complete(messages)
        except Exception as e:
            self.logger.error(f"[AIAssistant] Agent provider '{provider_name}' streaming failed: {e}", exc_info=True)
            if provider_name == "local":
                return self._perform_local_streaming_request(config, messages)
            if provider_name == "openrouter":
                return self._perform_openrouter_streaming_request(config, messages)
            if provider_name == "groq":
                return self._perform_groq_streaming_request(config, messages)
            if provider_name == "gemini":
                return self._perform_gemini_request(config, messages)
            return self._perform_request(config, messages)

    def _perform_local_request(
        self, config: Dict[str, str], messages: List[Dict[str, str]]
    ) -> str:
        """Perform request to local OpenAI-compatible API (Ollama, LM Studio, etc.)."""
        requests = _get_requests()

        base_url = config.get("local_base_url", "http://localhost:11434/v1").rstrip("/")
        model = config.get("model", "").strip() or self.DEFAULT_LOCAL_MODEL

        payload_messages = self._build_openai_messages(messages)
        url = f"{base_url}/chat/completions"
        payload: Dict[str, Any] = {
            "model": model,
            "messages": payload_messages,
            "stream": False,
        }

        headers = {"Content-Type": "application/json"}
        api_key = config.get("api_key", "").strip()
        if api_key:
            headers["Authorization"] = f"Bearer {api_key}"

        try:
            response = requests.post(url, headers=headers, json=payload, timeout=120)
        except requests.RequestException as exc:
            raise RuntimeError(f"Failed to query the local AI service: {exc}") from exc

        if response.status_code >= 400:
            raise RuntimeError(
                f"HTTP error {response.status_code}: {response.text.strip()}"
            )

        try:
            response_data = response.json()
        except ValueError as exc:
            raise RuntimeError("Local AI returned an invalid JSON response.") from exc

        choices = response_data.get("choices") or []
        if not choices:
            raise RuntimeError("The server response did not contain any suggestions.")

        message = choices[0].get("message") if isinstance(choices[0], dict) else None
        content = message.get("content") if isinstance(message, dict) else None
        if isinstance(content, list):
            content = "\n".join(
                part.get("text", "")
                for part in content
                if isinstance(part, dict) and part.get("text")
            )
        if not isinstance(content, str) or not content.strip():
            raise RuntimeError("Local AI did not return any usable content.")
        return content.strip()

    def _perform_local_streaming_request(
        self, config: Dict[str, str], messages: List[Dict[str, str]]
    ) -> str:
        """Perform streaming request to local OpenAI-compatible API."""
        requests = _get_requests()

        base_url = config.get("local_base_url", "http://localhost:11434/v1").rstrip("/")
        model = config.get("model", "").strip() or self.DEFAULT_LOCAL_MODEL

        payload_messages = self._build_openai_messages(messages)
        url = f"{base_url}/chat/completions"
        payload: Dict[str, Any] = {
            "model": model,
            "messages": payload_messages,
            "stream": True,
        }

        headers = {"Content-Type": "application/json"}
        api_key = config.get("api_key", "").strip()
        if api_key:
            headers["Authorization"] = f"Bearer {api_key}"

        try:
            response = requests.post(
                url, headers=headers, json=payload, timeout=120, stream=True
            )
        except requests.RequestException as exc:
            raise RuntimeError(f"Failed to query the local AI service: {exc}") from exc

        if response.status_code >= 400:
            raise RuntimeError(
                f"HTTP error {response.status_code}: {response.text.strip()}"
            )

        full_content = ""
        for line in response.iter_lines():
            if not line:
                continue
            line_str = line.decode("utf-8").strip()
            if line_str.startswith("data: "):
                data_str = line_str[6:]
                if data_str == "[DONE]":
                    break
                try:
                    data = json.loads(data_str)
                    choices = data.get("choices", [])
                    if choices:
                        delta = choices[0].get("delta", {})
                        chunk = delta.get("content", "")
                        if chunk:
                            full_content += chunk
                            if self._streaming_callback:
                                GLib.idle_add(self._streaming_callback, chunk, False)
                except json.JSONDecodeError:
                    continue

        if self._streaming_callback:
            GLib.idle_add(self._streaming_callback, "", True)

        return full_content

    def _perform_groq_streaming_request(
        self, config: Dict[str, str], messages: List[Dict[str, str]]
    ) -> str:
        """Perform streaming request to Groq API."""
        requests = _get_requests()

        api_key = config.get("api_key", "").strip()
        if not api_key:
            raise RuntimeError("Configure the Groq API key in Preferences.")

        model = config.get("model", "").strip() or self.DEFAULT_GROQ_MODEL

        payload_messages = self._build_openai_messages(messages)
        url = "https://api.groq.com/openai/v1/chat/completions"
        payload: Dict[str, Any] = {
            "model": model,
            "messages": payload_messages,
            "stream": True,
        }

        headers = {
            "Content-Type": "application/json",
            "Authorization": f"Bearer {api_key}",
        }

        try:
            response = requests.post(
                url, headers=headers, json=payload, timeout=60, stream=True
            )
        except requests.RequestException as exc:
            raise RuntimeError(f"Failed to query the Groq service: {exc}") from exc

        if response.status_code >= 400:
            raise RuntimeError(self._format_openrouter_error(response))

        full_content = ""
        for line in response.iter_lines():
            if not line:
                continue
            line_str = line.decode("utf-8").strip()
            if line_str.startswith("data: "):
                data_str = line_str[6:]
                if data_str == "[DONE]":
                    break
                try:
                    data = json.loads(data_str)
                    choices = data.get("choices", [])
                    if choices:
                        delta = choices[0].get("delta", {})
                        chunk = delta.get("content", "")
                        if chunk:
                            full_content += chunk
                            if self._streaming_callback:
                                GLib.idle_add(self._streaming_callback, chunk, False)
                except json.JSONDecodeError:
                    continue

        if self._streaming_callback:
            GLib.idle_add(self._streaming_callback, "", True)

        return full_content

    def _perform_openrouter_streaming_request(
        self, config: Dict[str, str], messages: List[Dict[str, str]]
    ) -> str:
        """Perform streaming request to OpenRouter API."""
        requests = _get_requests()

        api_key = config.get("api_key", "").strip()
        if not api_key:
            raise RuntimeError("Configure the OpenRouter API key in Preferences.")

        model = config.get("model", "").strip() or self.DEFAULT_OPENROUTER_MODEL
        payload_messages = self._build_openai_messages(messages)
        url = "https://openrouter.ai/api/v1/chat/completions"
        payload: Dict[str, Any] = {
            "model": model,
            "messages": payload_messages,
            "stream": True,
        }

        headers = {
            "Content-Type": "application/json",
            "Authorization": f"Bearer {api_key}",
        }
        site_url = config.get("openrouter_site_url")
        site_name = config.get("openrouter_site_name")
        if site_url:
            headers["HTTP-Referer"] = site_url
        if site_name:
            headers["X-Title"] = site_name

        try:
            response = requests.post(
                url, headers=headers, json=payload, timeout=60, stream=True
            )
        except requests.RequestException as exc:
            raise RuntimeError(
                f"Failed to query the OpenRouter service: {exc}"
            ) from exc

        if response.status_code >= 400:
            raise RuntimeError(
                f"HTTP error {response.status_code}: {response.text.strip()}"
            )

        full_content = ""
        for line in response.iter_lines():
            if not line:
                continue
            line_str = line.decode("utf-8").strip()
            if line_str.startswith("data: "):
                data_str = line_str[6:]
                if data_str == "[DONE]":
                    break
                try:
                    data = json.loads(data_str)
                    choices = data.get("choices", [])
                    if choices:
                        delta = choices[0].get("delta", {})
                        chunk = delta.get("content", "")
                        if chunk:
                            full_content += chunk
                            if self._streaming_callback:
                                GLib.idle_add(self._streaming_callback, chunk, False)
                except json.JSONDecodeError:
                    continue

        if self._streaming_callback:
            GLib.idle_add(self._streaming_callback, "", True)

        return full_content

    def _perform_gemini_request(
        self, config: Dict[str, str], messages: List[Dict[str, str]]
    ) -> str:
        requests = _get_requests()

        api_key = config.get("api_key", "").strip()
        if not api_key:
            raise RuntimeError("Configure the Gemini API key in Preferences.")

        model = config.get("model", "").strip() or self.DEFAULT_GEMINI_MODEL

        system_instruction, contents = self._build_gemini_conversation(messages)
        payload: Dict[str, Any] = {"contents": contents}
        if system_instruction:
            payload["system_instruction"] = {"parts": [{"text": system_instruction}]}

        headers = {
            "Content-Type": "application/json",
            "x-goog-api-key": api_key,
        }

        candidate_models = [model]
        for m in ("gemini-2.5-flash", "gemini-2.0-flash", "gemini-2.0-flash-exp", "gemini-1.5-pro", "gemini-1.5-flash-8b"):
            if m not in candidate_models:
                candidate_models.append(m)

        last_error = ""
        for m_name in candidate_models:
            url = f"https://generativelanguage.googleapis.com/v1beta/models/{m_name}:generateContent"
            try:
                response = requests.post(url, headers=headers, json=payload, timeout=120)
            except requests.RequestException as exc:
                raise RuntimeError(f"Failed to query the Gemini service: {exc}") from exc

            if response.status_code == 200:
                try:
                    response_data = response.json()
                except ValueError as exc:
                    raise RuntimeError("Gemini returned an invalid JSON response.") from exc

                candidates = response_data.get("candidates") or []
                if not candidates:
                    raise RuntimeError("The server response did not contain any suggestions.")

                collected: List[str] = []
                for candidate in candidates:
                    content = candidate.get("content") if isinstance(candidate, dict) else None
                    parts = content.get("parts") if isinstance(content, dict) else None
                    if not parts:
                        continue
                    for part in parts:
                        if isinstance(part, dict) and part.get("text"):
                            collected.append(part["text"])

                if collected:
                    return "\n".join(collected)
            elif response.status_code == 404:
                last_error = response.text
                continue
            else:
                raise RuntimeError(self._format_openrouter_error(response))

        raise RuntimeError(f"Gemini API error (404 on all models): {last_error}")

    def _perform_groq_request(
        self, config: Dict[str, str], messages: List[Dict[str, str]]
    ) -> str:
        requests = _get_requests()

        api_key = config.get("api_key", "").strip()
        if not api_key:
            raise RuntimeError("Configure the Groq API key in Preferences.")

        model = config.get("model", "").strip() or self.DEFAULT_GROQ_MODEL

        payload_messages = self._build_openai_messages(messages)
        url = "https://api.groq.com/openai/v1/chat/completions"
        payload: Dict[str, Any] = {"model": model, "messages": payload_messages}

        headers = {
            "Content-Type": "application/json",
            "Authorization": f"Bearer {api_key}",
        }

        try:
            response = requests.post(url, headers=headers, json=payload, timeout=60)
        except requests.RequestException as exc:
            raise RuntimeError(f"Failed to query the Groq service: {exc}") from exc

        if response.status_code >= 400:
            raise RuntimeError(self._format_openrouter_error(response))

        try:
            response_data = response.json()
        except ValueError as exc:
            raise RuntimeError("Groq returned an invalid JSON response.") from exc

        choices = response_data.get("choices") or []
        if not choices:
            raise RuntimeError("The server response did not contain any suggestions.")

        message = choices[0].get("message") if isinstance(choices[0], dict) else None
        content = message.get("content") if isinstance(message, dict) else None
        if isinstance(content, list):
            content = "\n".join(
                part.get("text", "")
                for part in content
                if isinstance(part, dict) and part.get("text")
            )
        if not isinstance(content, str) or not content.strip():
            raise RuntimeError("Groq did not return any usable content.")
        return content.strip()

    def _perform_openrouter_request(
        self, config: Dict[str, str], messages: List[Dict[str, str]]
    ) -> str:
        requests = _get_requests()

        api_key = config.get("api_key", "").strip()
        if not api_key:
            raise RuntimeError("Configure the OpenRouter API key in Preferences.")

        model = config.get("model", "").strip() or self.DEFAULT_OPENROUTER_MODEL
        payload_messages = self._build_openai_messages(messages)
        url = "https://openrouter.ai/api/v1/chat/completions"
        payload: Dict[str, Any] = {"model": model, "messages": payload_messages}

        headers = {
            "Content-Type": "application/json",
            "Authorization": f"Bearer {api_key}",
        }
        site_url = config.get("openrouter_site_url")
        site_name = config.get("openrouter_site_name")
        if site_url:
            headers["HTTP-Referer"] = site_url
        if site_name:
            headers["X-Title"] = site_name

        try:
            response = requests.post(url, headers=headers, json=payload, timeout=60)
        except requests.RequestException as exc:
            raise RuntimeError(
                f"Failed to query the OpenRouter service: {exc}"
            ) from exc

        if response.status_code >= 400:
            raise RuntimeError(
                f"HTTP error {response.status_code}: {response.text.strip()}"
            )

        try:
            response_data = response.json()
        except ValueError as exc:
            raise RuntimeError("OpenRouter returned an invalid JSON response.") from exc

        choices = response_data.get("choices") or []
        if not choices:
            raise RuntimeError("The server response did not contain any suggestions.")

        message = choices[0].get("message") if isinstance(choices[0], dict) else None
        content = message.get("content") if isinstance(message, dict) else None
        if isinstance(content, list):
            content = "\n".join(
                part.get("text", "")
                for part in content
                if isinstance(part, dict) and part.get("text")
            )
        if not isinstance(content, str) or not content.strip():
            raise RuntimeError("OpenRouter did not return any usable content.")
        return content.strip()

    def _format_openrouter_error(self, response: Any) -> str:
        """Format error from HTTP response. Response type is requests.Response."""
        status = response.status_code
        fallback = response.text.strip() or _("Unknown error.")
        try:
            payload = response.json()
        except ValueError:
            return _("OpenRouter respondeu com HTTP {status}: {message}").format(
                status=status, message=fallback
            )

        error_obj = payload.get("error")
        if not isinstance(error_obj, dict):
            return _("OpenRouter respondeu com HTTP {status}: {message}").format(
                status=status, message=fallback
            )

        message = error_obj.get("message")
        metadata = error_obj.get("metadata", {})
        provider_name = metadata.get("provider_name")
        raw_detail = metadata.get("raw")
        details = []
        if provider_name:
            details.append(provider_name)
        if raw_detail:
            details.append(raw_detail)
        extra = f" ({' | '.join(details)})" if details else ""

        clean_message = message or fallback
        return _("OpenRouter respondeu com HTTP {status}: {message}{detail}").format(
            status=status,
            message=clean_message,
            detail=extra,
        )

    def _build_gemini_conversation(
        self, messages: List[Dict[str, str]]
    ) -> Tuple[str, List[Dict[str, Any]]]:
        system_instruction = ""
        contents: List[Dict[str, Any]] = []
        for message in messages:
            role = message.get("role", "user")
            text = message.get("content", "")
            if not text:
                continue
            if role == "system" and not system_instruction:
                system_instruction = text
                continue
            mapped_role = "model" if role == "assistant" else "user"
            contents.append({
                "role": mapped_role,
                "parts": [{"text": text}],
            })
        if not contents:
            contents.append({"role": "user", "parts": [{"text": ""}]})
        return system_instruction, contents

    def _build_openai_messages(
        self, messages: List[Dict[str, str]]
    ) -> List[Dict[str, str]]:
        formatted: List[Dict[str, str]] = []
        for message in messages:
            role = message.get("role", "user")
            content = message.get("content", "")
            if not isinstance(content, str) or not content:
                continue
            role_mapped = role
            if role not in {"system", "user", "assistant"}:
                role_mapped = "user"
            formatted.append({"role": role_mapped, "content": content})
        return formatted

    def _parse_assistant_payload(
        self, content: str
    ) -> Tuple[str, List[Dict[str, Any]], List[Dict[str, str]]]:
        if not content:
            return "", [], []

        clean_content = content.strip()
        if clean_content.startswith("```"):
            first_nl = clean_content.find("\n")
            if first_nl != -1:
                clean_content = clean_content[first_nl + 1 :]
            if clean_content.endswith("```"):
                clean_content = clean_content[:-3].strip()

        reply_text = ""
        commands: List[Dict[str, Any]] = []
        code_snippets: List[Dict[str, str]] = []

        # Strategy 1: Standard JSON parsing (strict=False permits control characters)
        try:
            payload = json.loads(clean_content, strict=False)
            if isinstance(payload, dict):
                reply_text = (
                    payload.get("summary")
                    or payload.get("reply")
                    or payload.get("content")
                    or payload.get("message")
                    or ""
                )
                commands_field = (
                    payload.get("steps")
                    if "steps" in payload
                    else payload.get("commands", [])
                )
                commands = self._normalize_commands(commands_field)
                return reply_text, commands, code_snippets
        except Exception:
            pass

        # Strategy 2: Structural Key-to-Key extraction for malformed JSON from local models
        match_reply = re.search(
            r'["\'](?:summary|reply|content|message)["\']\s*:\s*["\']', clean_content
        )
        if match_reply:
            val_start = match_reply.end()
            # Look for the beginning of the commands/steps field
            match_cmds = re.search(
                r'["\']\s*,\s*["\'](?:commands|steps|tools|cmd|plan_id)["\']\s*:\s*(\[[\s\S]*?\])',
                clean_content[val_start:],
                re.DOTALL,
            )
            if match_cmds:
                raw_reply = clean_content[val_start : val_start + match_cmds.start()]
                raw_cmds = match_cmds.group(1)
                raw_reply = raw_reply.rstrip('"\n\r\t ')
                reply_text = self._unescape_json_string(raw_reply)
                try:
                    parsed_cmds = json.loads(raw_cmds, strict=False)
                    commands = self._normalize_commands(parsed_cmds)
                except Exception:
                    cmd_matches = re.findall(r'"((?:[^"\\]|\\.)*)"', raw_cmds)
                    commands = [
                        {"command": self._unescape_json_string(c), "description": ""}
                        for c in cmd_matches
                        if c
                    ]
                return reply_text, commands, code_snippets
            else:
                raw_reply = clean_content[val_start:].rstrip('"} \n\r\t')
                reply_text = self._unescape_json_string(raw_reply)
                return reply_text, commands, code_snippets

        # Strategy 3: Pure Markdown Fallback
        reply_text = content
        code_block_pattern = r"```(?:bash|sh|zsh|shell)?\n(.*?)```"
        matches = re.findall(code_block_pattern, content, re.DOTALL)
        for match in matches:
            body = match.strip()
            if body:
                for line in body.splitlines():
                    cmd_str = line.strip()
                    if (
                        cmd_str
                        and not cmd_str.startswith("#")
                        and not cmd_str.startswith("//")
                    ):
                        commands.append({
                            "command": cmd_str,
                            "description": "",
                        })

        return reply_text, commands, code_snippets

    @staticmethod
    def _unescape_json_string(val: str) -> str:
        """Properly unescape JSON string characters and repair code fence formatting."""
        if not val:
            return ""
        val = re.sub(r'\\u([0-9a-fA-F]{4})', lambda m: chr(int(m.group(1), 16)), val)
        val = (
            val.replace('\\"', '"')
            .replace("\\n", "\n")
            .replace("\\t", "\t")
            .replace("\\r", "\r")
            .replace("\\\\", "\\")
        )
        # Fix code fences missing newlines (e.g. ```bash\echo or ```sh\cat)
        val = re.sub(r'```([a-zA-Z0-9_-]+)\\(?=[^\n\r])', r'```\1\n', val)
        # Fix inline code fence blocks without newlines (e.g. ```bash echo ... ```)
        val = re.sub(r'```([a-zA-Z0-9_-]+)[ \t]+([^\n\r]+)```', r'```\1\n\2\n```', val)
        # Fix trailing code fence directly attached to code
        val = re.sub(r'([^\n])```$', r'\1\n```', val)
        return val

    def _clean_response(self, raw_content: str) -> str:
        """Remove markdown code fences, comments, and trailing commas from JSON."""
        clean = raw_content.strip()

        # Remove ```json no início e ``` no fim
        if clean.startswith("```"):
            first_newline = clean.find("\n")
            if first_newline != -1:
                clean = clean[first_newline + 1 :]
            if clean.endswith("```"):
                clean = clean[:-3]

        return self._strip_json_comments_and_commas(clean)

    @staticmethod
    def _strip_json_comments_and_commas(text: str) -> str:
        """Removes // and # comments and trailing commas from JSON/JSONC text."""
        lines = []
        for line in text.splitlines():
            in_quote = False
            quote_char = None
            cleaned_chars = []
            i = 0
            while i < len(line):
                ch = line[i]
                if ch in ('"', "'") and (i == 0 or line[i - 1] != "\\"):
                    if not in_quote:
                        in_quote = True
                        quote_char = ch
                    elif quote_char == ch:
                        in_quote = False
                        quote_char = None
                    cleaned_chars.append(ch)
                elif not in_quote and ch == "/" and i + 1 < len(line) and line[i + 1] == "/":
                    break
                elif not in_quote and ch == "#" and (i == 0 or line[i - 1].isspace() or line[i - 1] in (",", "{", "[")):
                    break
                else:
                    cleaned_chars.append(ch)
                i += 1
            lines.append("".join(cleaned_chars))
        cleaned = "\n".join(lines)
        cleaned = re.sub(r"/\*[\s\S]*?\*/", "", cleaned)
        cleaned = re.sub(r",\s*([}\]])", r"\1", cleaned)
        return cleaned.strip()

    def _extract_json_object(self, text: str) -> Optional[Dict[str, Any]]:
        cleaned_text = self._strip_json_comments_and_commas(text)
        start = cleaned_text.find("{")
        while start != -1:
            brace_level = 0
            for end in range(start, len(cleaned_text)):
                char = cleaned_text[end]
                if char == "{":
                    brace_level += 1
                elif char == "}":
                    brace_level -= 1
                    if brace_level == 0:
                        candidate = cleaned_text[start : end + 1]
                        try:
                            return json.loads(candidate)
                        except json.JSONDecodeError:
                            break
            start = cleaned_text.find("{", start + 1)
        return None

    def _normalize_commands(self, value: Any) -> List[Dict[str, Any]]:
        commands: List[Dict[str, Any]] = []
        if isinstance(value, list):
            for item in value:
                if isinstance(item, str) and item.strip():
                    commands.append({"command": item.strip(), "description": ""})
                elif isinstance(item, dict):
                    candidate = item.get("command") or item.get("cmd")
                    if not candidate and "argv" in item:
                        argv_val = item["argv"]
                        if isinstance(argv_val, list):
                            candidate = " ".join(str(tok) for tok in argv_val)
                        elif isinstance(argv_val, str):
                            candidate = argv_val
                    if not candidate and "tool" in item:
                        candidate = item["tool"]
                    description = item.get("description") or ""
                    if isinstance(candidate, str) and candidate.strip():
                        item_dict = dict(item)
                        item_dict["command"] = candidate.strip()
                        item_dict["description"] = description.strip() if isinstance(description, str) else ""
                        commands.append(item_dict)
                elif hasattr(item, "to_dict"):
                    d = item.to_dict()
                    cmd_str = " ".join(item.argv) if getattr(item, "argv", None) else getattr(item, "tool", "")
                    d["command"] = cmd_str
                    commands.append(d)
        elif isinstance(value, str) and value.strip():
            commands.append({"command": value.strip(), "description": ""})
        return commands

    def _record_assistant_message(self, terminal_id: int, message: str) -> None:
        with self._lock:
            history = self._conversations.setdefault(terminal_id, [])
            history.append({"role": "assistant", "content": message})

    def _display_assistant_reply(
        self,
        terminal_id: int,
        reply: str,
        commands: List[Dict[str, Any]],
        code_snippets: List[Dict[str, str]],
    ) -> bool:
        # Extract command items/strings for the signal
        command_list: List[Any] = []
        for cmd in commands:
            if isinstance(cmd, dict):
                if "step_id" in cmd or "risk" in cmd or "approval" in cmd:
                    command_list.append(cmd)
                else:
                    command_list.append(cmd.get("command", ""))
            elif isinstance(cmd, str):
                command_list.append(cmd)

        # Emit response-ready signal for the chat panel
        self.emit("response-ready", reply, command_list)

        # For terminal_id == -1 (overlay panel), skip terminal output
        if terminal_id == -1:
            return False

        terminal = self._get_terminal(terminal_id)
        window = self._window_ref()
        if not terminal or not window:
            # Fallback to terminal output if window not available
            if terminal:
                terminal.feed(
                    ("\n[AI Assistant] {}\n".format(reply.strip())).encode("utf-8")
                )
                for info in commands:
                    command_text = info.get("command") if isinstance(info, dict) else ""
                    if command_text:
                        terminal.feed(
                            (
                                "[AI Assistant] Command: {}\n".format(command_text)
                            ).encode("utf-8")
                        )
                for snippet in code_snippets:
                    code_text = snippet.get("code") if isinstance(snippet, dict) else ""
                    if code_text:
                        terminal.feed(
                            (
                                "[AI Assistant] Code suggestion:\n{}\n".format(
                                    code_text
                                )
                            ).encode("utf-8")
                        )
            return False

        try:
            formatted_reply = self._format_reply_for_dialog(reply)
            window.show_ai_response_dialog(
                terminal, formatted_reply, commands, code_snippets
            )
        except Exception as exc:  # pylint: disable=broad-except
            self.logger.error("Failed to show AI response dialog: %s", exc)
            terminal.feed(
                (
                    "\n[AI Assistant] {}\n".format(self._format_reply_for_dialog(reply))
                ).encode("utf-8")
            )
        return False

    @staticmethod
    def _format_reply_for_dialog(text: str) -> str:
        """Improve readability by normalizing inline code and list formatting."""
        if not isinstance(text, str):
            return ""

        cleaned = text
        cleaned = cleaned.replace("\r\n", "\n")
        cleaned = cleaned.replace("\\n", "\n").replace("\\t", "\t")
        cleaned = _INLINE_CODE_PATTERN.sub(r"\1", cleaned)
        cleaned = _PLUS_WHITESPACE_PATTERN.sub(" ", cleaned)
        cleaned = _SEMICOLON_NEWLINE_PATTERN.sub("\n", cleaned)
        cleaned = _SEMICOLON_SENTENCE_PATTERN.sub(".\n", cleaned)
        cleaned = _BOLD_ASTERISK_PATTERN.sub(r"\1", cleaned)
        cleaned = _BOLD_UNDERSCORE_PATTERN.sub(r"\1", cleaned)
        cleaned = _NUMBERED_LIST_START_PATTERN.sub(r"\n\1", cleaned)
        cleaned = _NUMBERED_LIST_FIX_PATTERN.sub(r"\n\1", cleaned)
        cleaned = _DASH_LIST_PATTERN.sub("\n• ", cleaned)
        cleaned = _ASTERISK_LIST_PATTERN.sub("\n• ", cleaned)
        cleaned = _MULTIPLE_NEWLINES_PATTERN.sub("\n\n", cleaned)

        lines = []
        previous_blank = False
        for raw_line in cleaned.splitlines():
            line = raw_line.strip()
            if not line:
                if not previous_blank:
                    lines.append("")
                    previous_blank = True
                continue
            lines.append(line)
            previous_blank = False

        return "\n".join(lines).strip()

    def _display_error_reply(self, terminal_id: int, message: str) -> bool:
        self._queue_toast(message)
        return False

    def _get_terminal(self, terminal_id: int):
        ref = self._terminal_refs.get(terminal_id)
        return ref() if ref else None

    def _cleanup_terminal_state(self, terminal_id: int) -> None:
        self._conversations.pop(terminal_id, None)
        self._terminal_refs.pop(terminal_id, None)
        self._inflight.pop(terminal_id, None)

    def _queue_toast(self, message: str) -> None:
        def _show_toast():
            window = self._window_ref()
            if window and hasattr(window, "toast_overlay"):
                toast = Adw.Toast(title=message)
                window.toast_overlay.add_toast(toast)
            return False

        GLib.idle_add(_show_toast)
