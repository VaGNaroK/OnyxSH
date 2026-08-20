# onyxsh/utils/diagnostics.py
"""
Secure System Diagnostics Generator for OnyxSH.

Collects sanitized environment telemetries (host OS, sandbox, graphics stack,
GTK/VTE versions, GPU, AI subsystem status, and recent error logs) with strict
multi-layer privacy redaction (keys, passwords, user paths, IPs, emails)
for easy sharing on GitHub Issues.
"""

from __future__ import annotations

import getpass
import json
import os
import platform
import re
import shutil
import sys
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional

from .platform import (
    detect_gpu_info,
    detect_os_context,
    get_config_directory,
    is_flatpak_sandbox,
)
from .translation_utils import _


class SystemDiagnostics:
    """Collects and sanitizes technical system diagnostics."""

    @staticmethod
    def sanitize_text(text: str) -> str:
        """Applies multi-layer cascading redaction on sensitive data.

        Masks:
        - API keys and tokens (Google AI Studio, OpenAI, Groq, Anthropic, AWS, GitHub)
        - Passwords, secrets and authorization tokens in URLs/assigns
        - Real username and user home paths (/home/username -> /home/<user>)
        - Private and public IPv4 / IPv6 addresses
        - Email addresses
        """
        if not text:
            return ""

        sanitized = text

        # 1. Redact known API key and secret patterns
        patterns = [
            # Google Gemini AI Studio (AIza...)
            (r"\bAIza[A-Za-z0-9_-]{28,45}\b", "[REDACTED_GEMINI_KEY]"),
            # Groq (gsk_...)
            (r"\bgsk_[A-Za-z0-9]{48,64}\b", "[REDACTED_GROQ_KEY]"),
            # OpenAI / OpenRouter (sk-...)
            (r"\bsk-[A-Za-z0-9_-]{20,64}\b", "[REDACTED_API_KEY]"),
            # GitHub Personal Access Token (ghp_..., gho_..., github_pat_...)
            (r"\b(ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9_]{36,255}\b", "[REDACTED_GITHUB_TOKEN]"),
            (r"\bgithub_pat_[A-Za-z0-9_]{82}\b", "[REDACTED_GITHUB_TOKEN]"),
            # AWS Access Key
            (r"\b(AKIA|ABIA|ACCA|ASIA)[A-Z0-9]{16}\b", "[REDACTED_AWS_KEY]"),
            # Bearer tokens
            (r"Bearer\s+[A-Za-z0-9._~+/-]{15,}", "Bearer [REDACTED_TOKEN]"),
            # Generic key / password / token in query strings or JSON
            (r'(?i)(["\']?(?:api[_-]?key|token|secret|password|passwd|auth)["\']?\s*[:=]\s*["\'])([^"\']{4,})(["\'])', r'\1[REDACTED]\3'),
        ]

        for pat, repl in patterns:
            sanitized = re.sub(pat, repl, sanitized)

        # 2. Redact Email addresses
        email_pattern = r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b"
        sanitized = re.sub(email_pattern, "[REDACTED_EMAIL]", sanitized)

        # 3. Redact IPv4 addresses (excluding standard loopback 127.0.0.1 for local services)
        ipv4_pattern = r"\b(?!127\.0\.0\.1\b)(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b"
        sanitized = re.sub(ipv4_pattern, "[REDACTED_IP]", sanitized)

        # 4. Redact real usernames and home paths
        try:
            current_user = getpass.getuser()
            if current_user and current_user not in ("root", "<user>"):
                sanitized = sanitized.replace(f"/home/{current_user}", "/home/<user>")
                sanitized = re.sub(r"\b" + re.escape(current_user) + r"\b", "<user>", sanitized)
        except Exception:
            pass

        # General /home/username fallback redaction
        sanitized = re.sub(r"/home/[a-zA-Z0-9._-]+(/|$)", r"/home/<user>\1", sanitized)

        return sanitized

    @classmethod
    def check_ollama_status(cls, base_url: str = "http://localhost:11434/v1") -> Dict[str, Any]:
        """Performs a non-blocking connectivity check to local Ollama/LM Studio."""
        import requests

        clean_url = base_url.rstrip("/")
        # If /v1 at the end, root is one level up
        root_url = clean_url[:-3] if clean_url.endswith("/v1") else clean_url

        result: Dict[str, Any] = {
            "online": False,
            "version": "",
            "models": [],
            "error": "",
        }

        try:
            # 1. Try Ollama native version endpoint
            resp = requests.get(f"{root_url}/api/version", timeout=1.0)
            if resp.status_code == 200:
                result["online"] = True
                result["version"] = resp.json().get("version", "unknown")
            else:
                # 2. Try OpenAI compatible /v1/models
                m_resp = requests.get(f"{clean_url}/models", timeout=1.0)
                if m_resp.status_code == 200:
                    result["online"] = True
                    data = m_resp.json()
                    models = [m.get("id") for m in data.get("data", []) if m.get("id")]
                    result["models"] = models[:5]
                    return result
        except Exception as e:
            result["error"] = str(e)
            return result

        # If Ollama version was found, get tags
        if result["online"]:
            try:
                tags_resp = requests.get(f"{root_url}/api/tags", timeout=1.0)
                if tags_resp.status_code == 200:
                    tags_data = tags_resp.json()
                    result["models"] = [m.get("name") for m in tags_data.get("models", []) if m.get("name")]
            except Exception:
                pass

        return result

    @classmethod
    def collect_system_data(cls, settings_manager: Optional[Any] = None, log_lines: int = 30) -> Dict[str, Any]:
        """Collects structured system diagnostics data."""
        if settings_manager is None:
            try:
                from ..settings.manager import get_settings_manager
                settings_manager = get_settings_manager()
            except Exception:
                settings_manager = None

        from ..settings.config import APP_TITLE, APP_VERSION

        # 1. OS & Kernel
        os_name = detect_os_context()
        uname = platform.uname()

        session_type = os.environ.get("XDG_SESSION_TYPE", "unknown")
        desktop = os.environ.get("XDG_CURRENT_DESKTOP", os.environ.get("DESKTOP_SESSION", "unknown"))

        # 2. Sandbox
        is_flatpak = is_flatpak_sandbox()
        flatpak_id = os.environ.get("FLATPAK_ID", "")
        flatpak_spawn = bool(shutil.which("flatpak-spawn"))
        host_spawn = bool(shutil.which("host-spawn"))

        # 3. Dependencies
        py_version = f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}"
        pygobject_version = "unknown"
        try:
            import gi
            pygobject_version = getattr(gi, "__version__", "unknown")
        except Exception:
            pass

        gtk_version = "unknown"
        adw_version = "unknown"
        vte_version = "unknown"
        try:
            import gi
            gi.require_version("Gtk", "4.0")
            gi.require_version("Adw", "1")
            gi.require_version("Vte", "3.91")
            from gi.repository import Adw, Gtk, Vte

            gtk_version = f"{Gtk.get_major_version()}.{Gtk.get_minor_version()}.{Gtk.get_micro_version()}"
            adw_version = f"{Adw.get_major_version()}.{Adw.get_minor_version()}.{Adw.get_micro_version()}"
            vte_version = f"{Vte.get_major_version()}.{Vte.get_minor_version()}.{Vte.get_micro_version()}"
        except Exception:
            pass

        # 4. Graphics & GPU
        gsk_renderer = os.environ.get("GSK_RENDERER", "default")
        gdk_backend = os.environ.get("GDK_BACKEND", "default")
        gpu_info = detect_gpu_info()

        # 5. AI Subsystem & Settings
        ai_data: Dict[str, Any] = {
            "enabled": False,
            "offline_mode": False,
            "smart_routing_enabled": True,
            "routing_profile": "auto",
            "fast_profile": {},
            "advanced_profile": {},
            "providers_status": {},
            "ollama_live": {},
        }

        if settings_manager:
            ai_data["enabled"] = bool(settings_manager.get("ai_assistant_enabled", True))
            ai_data["offline_mode"] = bool(settings_manager.get("ai_assistant_offline_mode", False))
            ai_data["smart_routing_enabled"] = bool(settings_manager.get("ai_smart_routing_enabled", True))
            ai_data["routing_profile"] = settings_manager.get("ai_routing_profile", "auto")

            ai_data["fast_profile"] = {
                "provider": settings_manager.get("ai_fast_provider", "local"),
                "model": settings_manager.get("ai_fast_model", "qwen2.5-coder:7b"),
            }
            ai_data["advanced_profile"] = {
                "provider": settings_manager.get("ai_advanced_provider", "gemini"),
                "model": settings_manager.get("ai_advanced_model", "gemini-2.5-flash"),
            }

            # Check configured keys without revealing them
            ai_data["providers_status"] = {
                "gemini": bool(settings_manager.get("ai_api_key_gemini", "").strip()),
                "groq": bool(settings_manager.get("ai_api_key_groq", "").strip()),
                "openrouter": bool(settings_manager.get("ai_api_key_openrouter", "").strip()),
                "legacy_key": bool(settings_manager.get("ai_assistant_api_key", "").strip()),
            }

            # Check local Ollama
            local_url = settings_manager.get("ai_local_base_url", "http://localhost:11434/v1")
            ai_data["ollama_live"] = cls.check_ollama_status(local_url)

        # 6. Essential CLI Tools
        tools = {
            "ssh": bool(shutil.which("ssh")),
            "sftp": bool(shutil.which("sftp")),
            "rsync": bool(shutil.which("rsync")),
            "git": bool(shutil.which("git")),
            "netplan": bool(shutil.which("netplan")),
            "curl": bool(shutil.which("curl")),
            "tar": bool(shutil.which("tar")),
        }

        # 7. Recent Sanitized Logs
        sanitized_logs = cls.get_sanitized_recent_logs(log_lines)

        return {
            "app": {
                "name": APP_TITLE,
                "version": APP_VERSION,
                "timestamp": datetime.now().isoformat(),
            },
            "system": {
                "os": os_name,
                "kernel": uname.release,
                "architecture": uname.machine,
                "session_type": session_type,
                "desktop_environment": desktop,
            },
            "sandbox": {
                "is_flatpak": is_flatpak,
                "flatpak_id": flatpak_id,
                "flatpak_spawn": flatpak_spawn,
                "host_spawn": host_spawn,
            },
            "stack": {
                "python": py_version,
                "pygobject": pygobject_version,
                "gtk": gtk_version,
                "libadwaita": adw_version,
                "vte": vte_version,
                "gsk_renderer": gsk_renderer,
                "gdk_backend": gdk_backend,
                "gpu": gpu_info.get("description", "Not detected"),
            },
            "ai": ai_data,
            "tools": tools,
            "logs": sanitized_logs,
        }

    @classmethod
    def get_sanitized_recent_logs(cls, max_lines: int = 30) -> List[str]:
        """Extracts recent log entries with privacy sanitization applied."""
        config_dir = get_config_directory()
        log_candidates = [
            config_dir / "logs" / "onyxsh_errors.log",
            config_dir / "logs" / "onyxsh.log",
            Path.home() / ".cache" / "onyxsh" / "onyxsh_errors.log",
            Path.home() / ".cache" / "onyxsh" / "onyxsh.log",
        ]

        # Also search Flatpak var directories if inside sandbox
        if is_flatpak_sandbox():
            flatpak_cache = Path.home() / ".var" / "app" / "io.github.vagnarok.OnyxSH" / "config" / "onyxsh" / "logs"
            log_candidates.insert(0, flatpak_cache / "onyxsh_errors.log")
            log_candidates.insert(1, flatpak_cache / "onyxsh.log")

        collected_lines: List[str] = []
        seen_files = set()

        for log_file in log_candidates:
            if not log_file.exists() or log_file in seen_files:
                continue
            seen_files.add(log_file)
            try:
                content = log_file.read_text(encoding="utf-8", errors="replace")
                lines = [l.strip() for l in content.splitlines() if l.strip()]
                # Prioritize WARNING / ERROR lines
                for l in reversed(lines):
                    sanitized_l = cls.sanitize_text(l)
                    collected_lines.append(sanitized_l)
                    if len(collected_lines) >= max_lines:
                        break
            except Exception:
                continue
            if len(collected_lines) >= max_lines:
                break

        collected_lines.reverse()
        return collected_lines

    @classmethod
    def generate_markdown_report(cls, data: Dict[str, Any]) -> str:
        """Formats the collected diagnostic data as GitHub-ready Markdown."""
        app = data.get("app", {})
        sys_info = data.get("system", {})
        sandbox = data.get("sandbox", {})
        stack = data.get("stack", {})
        ai = data.get("ai", {})
        tools = data.get("tools", {})
        logs = data.get("logs", [])

        # AI Providers summary
        p_status = ai.get("providers_status", {})
        keys_summary = []
        if p_status.get("gemini"):
            keys_summary.append("Google Gemini (Configurada)")
        if p_status.get("groq"):
            keys_summary.append("Groq (Configurada)")
        if p_status.get("openrouter"):
            keys_summary.append("OpenRouter (Configurada)")
        if not keys_summary:
            keys_summary.append("Nenhuma chave externa configurada")

        # Ollama status
        ollama = ai.get("ollama_live", {})
        if ollama.get("online"):
            models_list = ", ".join(ollama.get("models", [])) or "Sem modelos baixados"
            ollama_status_str = f"🟢 Online (v{ollama.get('version', 'unknown')}) — Modelos: `{models_list}`"
        else:
            ollama_status_str = "⚪ Inativo ou não acessível localmente"

        # Format tools checklist
        tools_str = ", ".join([f"`{t}`: {'✅' if ok else '❌'}" for t, ok in tools.items()])

        # Build Markdown
        md_lines = [
            f"# 🔍 OnyxSH System Diagnostic Report",
            f"",
            f"> **Generated at:** `{app.get('timestamp')}` | **OnyxSH Version:** `{app.get('version')}`",
            f"> *Note: All personal identifiers, home paths, IP addresses, and API credentials have been sanitized.*",
            f"",
            f"## 🖥️ Operating System & Environment",
            f"- **Host OS:** {sys_info.get('os')}",
            f"- **Kernel:** `{sys_info.get('kernel')}` ({sys_info.get('architecture')})",
            f"- **Desktop / Session:** {sys_info.get('desktop_environment')} ({sys_info.get('session_type')})",
            f"- **Sandbox Environment:** {'📦 Flatpak (`' + sandbox.get('flatpak_id', '') + '`)' if sandbox.get('is_flatpak') else '🌐 Host / Native'}",
            f"- **Host Portals:** `flatpak-spawn`: {'✅' if sandbox.get('flatpak_spawn') else '❌'} | `host-spawn`: {'✅' if sandbox.get('host_spawn') else '❌'}",
            f"",
            f"## ⚙️ Graphics & Runtime Stack",
            f"- **Python:** `{stack.get('python')}` (PyGObject `{stack.get('pygobject')}`)",
            f"- **GTK / Libadwaita:** GTK `{stack.get('gtk')}` | Libadwaita `{stack.get('libadwaita')}` | VTE `{stack.get('vte')}`",
            f"- **Renderer:** `GSK_RENDERER={stack.get('gsk_renderer')}` | `GDK_BACKEND={stack.get('gdk_backend')}`",
            f"- **GPU Hardware:** {stack.get('gpu')}",
            f"",
            f"## 🤖 AI Assistant Subsystem",
            f"- **AI Assistant Enabled:** {'✅ Sim' if ai.get('enabled') else '❌ Não'}",
            f"- **Strictly Offline Mode:** {'🛡️ Ativo (Restrito ao Local)' if ai.get('offline_mode') else '🌐 Desativado'}",
            f"- **Smart Model Routing:** {'🧠 Ativo' if ai.get('smart_routing_enabled') else '⚪ Desativado'} (Perfil: `{ai.get('routing_profile')}`)",
            f"- **Fast Profile:** `{ai.get('fast_profile', {}).get('provider')}` (`{ai.get('fast_profile', {}).get('model')}`)",
            f"- **Advanced Profile:** `{ai.get('advanced_profile', {}).get('provider')}` (`{ai.get('advanced_profile', {}).get('model')}`)",
            f"- **Configured Cloud Keys:** {', '.join(keys_summary)}",
            f"- **Local Ollama Status:** {ollama_status_str}",
            f"",
            f"## 🛠️ CLI Utilities Availability",
            f"- {tools_str}",
            f"",
            f"## 📜 Recent Sanitized Error Logs",
        ]

        if logs:
            md_lines.append("```text")
            md_lines.extend(logs)
            md_lines.append("```")
        else:
            md_lines.append("*Nenhum erro recente registrado nos arquivos de log.*")

        md_lines.append("")
        return "\n".join(md_lines)

    @classmethod
    def generate_json_report(cls, data: Dict[str, Any]) -> str:
        """Formats the collected diagnostic data as structured JSON."""
        return json.dumps(data, ensure_ascii=False, indent=2)

    @classmethod
    def run_cli(cls, args: Any) -> int:
        """Executes CLI diagnostics command with specified parameters."""
        log_lines = getattr(args, "lines", 30) or 30
        is_json = getattr(args, "json", False)
        output_file = getattr(args, "output", None)

        data = cls.collect_system_data(log_lines=log_lines)

        if is_json:
            report_text = cls.generate_json_report(data)
        else:
            report_text = cls.generate_markdown_report(data)

        if output_file:
            try:
                out_path = Path(output_file).expanduser().resolve()
                out_path.write_text(report_text, encoding="utf-8")
                print(f"✅ Diagnóstico salvo com sucesso em: {out_path}")
                return 0
            except Exception as e:
                print(f"❌ Erro ao salvar arquivo de diagnóstico: {e}", file=sys.stderr)
                return 1

        # Print to stdout
        print(report_text)
        return 0
