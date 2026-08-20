# onyxsh/terminal/exporter.py
"""
Terminal content exporter supporting multiple formats:
- Plain Text (.txt)
- Log File (.log)
- Markdown (.md) with structured metadata and code fences
- Styled HTML (.html) with dark theme and syntax styling
- Asciinema v2 (.cast / .asciinema) structured recording format
"""

import json
import os
import pathlib
import time
from typing import Any, Dict, List, Optional, Tuple

import gi

gi.require_version("Vte", "3.91")
gi.require_version("Gtk", "4.0")
from gi.repository import Gio, GLib, Gtk, Vte

from ..utils.logger import get_logger
from ..utils.translation_utils import _


class TerminalExporter:
    """Handles extraction, formatting and exporting of terminal buffer content."""

    def __init__(self) -> None:
        self.logger = get_logger("onyxsh.terminal.exporter")

    def get_terminal_text(
        self, terminal: Vte.Terminal, selection_only: bool = False
    ) -> str:
        """
        Extracts plain text content from the terminal.

        Args:
            terminal: The Vte.Terminal instance.
            selection_only: If True, extracts only currently selected text.
        """
        if selection_only and hasattr(terminal, "get_has_selection") and terminal.get_has_selection():
            try:
                text = terminal.get_text_format(Vte.Format.TEXT)
                if text:
                    return text
            except Exception:
                pass

        # Full scrollback history extraction via write_contents_sync
        try:
            stream = Gio.MemoryOutputStream.new_resizable()
            flags = getattr(Vte, "WriteFlags", None)
            default_flag = getattr(flags, "DEFAULT", 0) if flags else 0
            if hasattr(terminal, "write_contents_sync") and terminal.write_contents_sync(stream, default_flag, None):
                stream.close()
                bytes_data = stream.steal_as_bytes()
                if bytes_data and len(bytes_data.get_data()) > 0:
                    text = bytes_data.get_data().decode("utf-8", errors="replace")
                    if text.strip():
                        return text
        except Exception as e:
            self.logger.warning(f"write_contents_sync failed, falling back to get_text_format: {e}")

        try:
            return terminal.get_text_format(Vte.Format.TEXT) or ""
        except Exception as e:
            self.logger.error(f"Failed to extract terminal text: {e}")
            return ""

    def get_terminal_html(
        self, terminal: Vte.Terminal, selection_only: bool = False
    ) -> str:
        """
        Extracts HTML formatted content with ANSI styling from the terminal.
        """
        if selection_only and hasattr(terminal, "get_has_selection") and terminal.get_has_selection():
            try:
                return terminal.get_text_format(Vte.Format.HTML) or ""
            except Exception:
                pass

        plain_text = self.get_terminal_text(terminal, selection_only)
        return f"<pre>{GLib.markup_escape_text(plain_text)}</pre>"

    def extract_metadata(self, terminal: Vte.Terminal) -> Dict[str, Any]:
        """Gathers runtime metadata for the export header."""
        cwd = "~"
        host = "localhost"
        session_name = _("Local Terminal")

        if hasattr(terminal, "get_current_directory_uri"):
            try:
                uri = terminal.get_current_directory_uri()
                if uri and uri.startswith("file://"):
                    from urllib.parse import unquote, urlparse

                    parsed = urlparse(uri)
                    clean_path = unquote(parsed.path).split("?")[0].rstrip("/")
                    home_dir = str(pathlib.Path.home()).rstrip("/")
                    if clean_path == home_dir:
                        cwd = "~"
                    elif clean_path.startswith(home_dir + "/"):
                        cwd = f"~/{clean_path[len(home_dir)+1:]}"
                    else:
                        cwd = clean_path or "/"
            except Exception:
                pass

        if hasattr(terminal, "onyxsh_session"):
            sess = getattr(terminal, "onyxsh_session", None)
            if sess:
                session_name = getattr(sess, "name", session_name)
                host = getattr(sess, "host", host)

        cols = terminal.get_column_count() if hasattr(terminal, "get_column_count") else 80
        rows = terminal.get_row_count() if hasattr(terminal, "get_row_count") else 24

        return {
            "session_name": session_name,
            "host": host,
            "cwd": cwd,
            "cols": max(20, cols),
            "rows": max(5, rows),
            "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
            "unix_timestamp": int(time.time()),
        }

    def export_plain_text(
        self, terminal: Vte.Terminal, selection_only: bool = False
    ) -> str:
        """Exports raw plain text."""
        raw_text = self.get_terminal_text(terminal, selection_only)
        # Strip trailing blank lines
        lines = raw_text.splitlines()
        while lines and not lines[-1].strip():
            lines.pop()
        return "\n".join(lines) + "\n"

    def export_log(
        self, terminal: Vte.Terminal, selection_only: bool = False
    ) -> str:
        """Exports as a structured log file with header."""
        meta = self.extract_metadata(terminal)
        log_title = _("OnyxSH Terminal Output Log")
        session_label = _("Session")
        host_label = _("Host")
        cwd_label = _("CWD")
        date_label = _("Date")
        dim_label = _("Dimensions")

        header = (
            f"================================================================================\n"
            f" {log_title}\n"
            f" {session_label}: {meta['session_name']} | {host_label}: {meta['host']} | {cwd_label}: {meta['cwd']}\n"
            f" {date_label}: {meta['timestamp']} | {dim_label}: {meta['cols']}x{meta['rows']}\n"
            f"================================================================================\n\n"
        )
        content = self.export_plain_text(terminal, selection_only)
        return header + content

    def export_markdown(
        self, terminal: Vte.Terminal, selection_only: bool = False
    ) -> str:
        """Exports as a formatted Markdown document."""
        meta = self.extract_metadata(terminal)
        content = self.export_plain_text(terminal, selection_only)
        line_count = len(content.splitlines())

        md_title = _("OnyxSH Terminal Export")
        session_label = _("Session:")
        host_label = _("Host:")
        dir_label = _("Directory:")
        date_label = _("Date / Time:")
        dim_label = _("Dimensions:")
        lines_label = _("Total Lines:")

        md_header = (
            f"# 🖥️ {md_title}\n\n"
            f"- **{session_label}** `{meta['session_name']}`\n"
            f"- **{host_label}** `{meta['host']}`\n"
            f"- **{dir_label}** `{meta['cwd']}`\n"
            f"- **{date_label}** `{meta['timestamp']}`\n"
            f"- **{dim_label}** `{meta['cols']} x {meta['rows']}`\n"
            f"- **{lines_label}** `{line_count}`\n\n"
            f"---\n\n"
            f"```bash\n"
        )
        md_footer = "\n```\n"
        return md_header + content.rstrip("\n") + md_footer

    def export_html(
        self, terminal: Vte.Terminal, selection_only: bool = False
    ) -> str:
        """Exports as a standalone, modern dark-themed HTML document."""
        meta = self.extract_metadata(terminal)
        vte_html = self.get_terminal_html(terminal, selection_only)

        # Extract inner pre content if wrapped in <pre>
        inner_content = vte_html
        if inner_content.startswith("<pre>") and inner_content.endswith("</pre>"):
            inner_content = inner_content[5:-6]

        html_title = _("OnyxSH Terminal Export")
        copy_label = _("Copy Output")
        copied_label = _("Copied!")
        session_label = _("Session:")
        host_label = _("Host:")
        dir_label = _("Directory:")
        date_label = _("Date:")
        dim_label = _("Dimensions:")

        html_template = f"""<!DOCTYPE html>
<html lang="pt-BR">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{html_title} - {meta['session_name']}</title>
  <style>
    :root {{
      --bg-color: #121418;
      --card-bg: #1c1f26;
      --border-color: #2e3440;
      --text-color: #e5e9f0;
      --text-dim: #8892b0;
      --accent: #5e81ac;
      --accent-hover: #81a1c1;
    }}
    * {{
      box-sizing: border-box;
      margin: 0;
      padding: 0;
    }}
    body {{
      background-color: var(--bg-color);
      color: var(--text-color);
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu, Cantarell, "Open Sans", sans-serif;
      padding: 24px;
      line-height: 1.5;
    }}
    .container {{
      max-width: 1200px;
      margin: 0 auto;
    }}
    .header-card {{
      background-color: var(--card-bg);
      border: 1px solid var(--border-color);
      border-radius: 10px;
      padding: 18px 24px;
      margin-bottom: 20px;
      display: flex;
      justify-content: space-between;
      align-items: center;
      flex-wrap: wrap;
      gap: 12px;
    }}
    .header-title {{
      display: flex;
      align-items: center;
      gap: 10px;
      font-size: 1.25rem;
      font-weight: 600;
    }}
    .metadata-grid {{
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
      gap: 10px;
      width: 100%;
      margin-top: 10px;
      font-size: 0.875rem;
      color: var(--text-dim);
    }}
    .metadata-item span {{
      color: var(--text-color);
      font-weight: 500;
    }}
    .copy-btn {{
      background-color: var(--accent);
      color: white;
      border: none;
      border-radius: 6px;
      padding: 8px 16px;
      font-size: 0.875rem;
      font-weight: 500;
      cursor: pointer;
      transition: background 0.2s;
    }}
    .copy-btn:hover {{
      background-color: var(--accent-hover);
    }}
    .terminal-container {{
      background-color: #0b0c10;
      border: 1px solid var(--border-color);
      border-radius: 10px;
      overflow: hidden;
      box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    }}
    .terminal-header {{
      background-color: #1a1c23;
      padding: 10px 16px;
      display: flex;
      align-items: center;
      gap: 8px;
      border-bottom: 1px solid var(--border-color);
    }}
    .circle {{
      width: 12px;
      height: 12px;
      border-radius: 50%;
      display: inline-block;
    }}
    .circle.red {{ background-color: #ff5f56; }}
    .circle.yellow {{ background-color: #ffbd2e; }}
    .circle.green {{ background-color: #27c93f; }}
    .terminal-title {{
      color: var(--text-dim);
      font-size: 0.8rem;
      font-family: monospace;
      margin-left: 8px;
    }}
    pre.terminal-content {{
      padding: 20px;
      overflow-x: auto;
      font-family: "Cascadia Code", "Fira Code", "JetBrains Mono", "Source Code Pro", Consolas, Menlo, Monaco, monospace;
      font-size: 13.5px;
      line-height: 1.45;
      tab-size: 8;
      white-space: pre;
    }}
  </style>
</head>
<body>
  <div class="container">
    <div class="header-card">
      <div class="header-title">
        <span>⚡ {html_title}</span>
      </div>
      <button class="copy-btn" onclick="copyTerminalText()">📋 {copy_label}</button>
      <div class="metadata-grid">
        <div class="metadata-item">{session_label} <span>{meta['session_name']}</span></div>
        <div class="metadata-item">{host_label} <span>{meta['host']}</span></div>
        <div class="metadata-item">{dir_label} <span>{meta['cwd']}</span></div>
        <div class="metadata-item">{date_label} <span>{meta['timestamp']}</span></div>
        <div class="metadata-item">{dim_label} <span>{meta['cols']} x {meta['rows']}</span></div>
      </div>
    </div>

    <div class="terminal-container">
      <div class="terminal-header">
        <span class="circle red"></span>
        <span class="circle yellow"></span>
        <span class="circle green"></span>
        <span class="terminal-title">{meta['host']} — {meta['cwd']}</span>
      </div>
      <pre id="terminal-pre" class="terminal-content">{inner_content}</pre>
    </div>
  </div>

  <script>
    function copyTerminalText() {{
      const el = document.getElementById('terminal-pre');
      navigator.clipboard.writeText(el.innerText).then(() => {{
        const btn = document.querySelector('.copy-btn');
        btn.innerText = '✅ {copied_label}';
        setTimeout(() => {{ btn.innerText = '📋 {copy_label}'; }}, 2000);
      }});
    }}
  </script>
</body>
</html>
"""
        return html_template

    def export_asciinema(
        self, terminal: Vte.Terminal, selection_only: bool = False
    ) -> str:
        """
        Exports the terminal scrollback in standard Asciinema v2 format (.cast / .asciinema).
        Compatible with `asciinema play` and asciinema web player.
        """
        meta = self.extract_metadata(terminal)
        header = {
            "version": 2,
            "width": meta["cols"],
            "height": meta["rows"],
            "timestamp": meta["unix_timestamp"],
            "title": f"OnyxSH Export - {meta['session_name']} ({meta['host']})",
            "env": {
                "SHELL": os.environ.get("SHELL", "/bin/bash"),
                "TERM": os.environ.get("TERM", "xterm-256color"),
            },
        }

        lines_output = [json.dumps(header)]

        raw_text = self.get_terminal_text(terminal, selection_only)
        content_lines = raw_text.splitlines()

        # Generate sequential timed events with a small virtual delta
        current_time = 0.05
        for line in content_lines:
            event = [round(current_time, 3), "o", line + "\r\n"]
            lines_output.append(json.dumps(event))
            current_time += 0.02

        return "\n".join(lines_output) + "\n"

    def format_content(
        self,
        terminal: Vte.Terminal,
        format_id: str,
        selection_only: bool = False,
    ) -> Tuple[str, str, str]:
        """
        Returns (content_string, default_extension, mime_type) for the requested format.

        Format IDs: 'txt', 'log', 'md', 'html', 'cast'
        """
        fmt = format_id.lower().lstrip(".")
        if fmt == "txt":
            return self.export_plain_text(terminal, selection_only), ".txt", "text/plain"
        elif fmt == "log":
            return self.export_log(terminal, selection_only), ".log", "text/plain"
        elif fmt == "md" or fmt == "markdown":
            return self.export_markdown(terminal, selection_only), ".md", "text/markdown"
        elif fmt == "html":
            return self.export_html(terminal, selection_only), ".html", "text/html"
        elif fmt in ("cast", "asciinema"):
            return self.export_asciinema(terminal, selection_only), ".cast", "application/json"
        else:
            # Default fallback to plain text
            return self.export_plain_text(terminal, selection_only), ".txt", "text/plain"


_global_exporter: Optional[TerminalExporter] = None


def get_terminal_exporter() -> TerminalExporter:
    """Returns singleton instance of TerminalExporter."""
    global _global_exporter
    if _global_exporter is None:
        _global_exporter = TerminalExporter()
    return _global_exporter
