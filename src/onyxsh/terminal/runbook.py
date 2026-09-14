# onyxsh/terminal/runbook.py
"""
Runbook / Caderno de Bordo engine for OnyxSH.
Structures terminal command executions and outputs with user annotations,
objectives, and conclusions into technical documentation (Markdown, HTML, Log).
"""

import getpass
import html
import os
import re
import time
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional

import gi

gi.require_version("Vte", "3.91")
gi.require_version("Gtk", "4.0")
from gi.repository import GLib, Vte

from ..utils.logger import get_logger
from ..utils.translation_utils import _


@dataclass
class RunbookStep:
    """Represents a single step (command + output + annotation) in a runbook."""

    command: str
    output: str = ""
    exit_code: Optional[int] = 0
    duration_str: str = ""
    timestamp_str: str = ""
    cwd: str = ""
    annotation: str = ""
    included: bool = True

    @property
    def is_success(self) -> bool:
        return self.exit_code == 0 or self.exit_code is None


@dataclass
class RunbookMetadata:
    """Metadata describing the runbook session and operational context."""

    title: str = ""
    author: str = ""
    objective: str = ""
    general_notes: str = ""
    conclusion: str = ""
    status: str = "Concluído"
    session_name: str = ""
    host: str = ""
    date_str: str = ""
    total_commands: int = 0


@dataclass
class RunbookData:
    """Complete container for runbook metadata and its sequential execution steps."""

    metadata: RunbookMetadata = field(default_factory=RunbookMetadata)
    steps: List[RunbookStep] = field(default_factory=list)


class RunbookGenerator:
    """Engine for generating and exporting runbooks from terminal sessions."""

    def __init__(self) -> None:
        self.logger = get_logger("onyxsh.terminal.runbook")

    def from_terminal(
        self,
        terminal: Vte.Terminal,
        selection_only: bool = False,
    ) -> RunbookData:
        """
        Extracts semantic commands and outputs from a terminal session,
        falling back to text buffer parsing if semantic commands are unavailable.
        """
        metadata = self._extract_metadata(terminal)
        steps: List[RunbookStep] = []

        # 1. Attempt extraction via SemanticTracker
        try:
            from .semantic_tracker import SemanticTracker
            tracker = SemanticTracker()
            state = tracker.get_or_create_state(terminal)

            if state and state.commands:
                col_count = (
                    terminal.get_column_count()
                    if hasattr(terminal, "get_column_count")
                    else 120
                )
                for cmd in state.commands:
                    cmd_text = cmd.command_text or tracker.extract_command_text(terminal, cmd)
                    if not cmd_text.strip():
                        continue

                    # Extract output for this command
                    out_text = cmd.output_cache or ""
                    if not out_text and cmd.output_end_row is not None and cmd.output_start_row is not None:
                        try:
                            if hasattr(terminal, "get_text_range_format"):
                                res = terminal.get_text_range_format(
                                    Vte.Format.TEXT,
                                    cmd.output_start_row,
                                    0,
                                    cmd.output_end_row + 1,
                                    col_count,
                                )
                                out_text = res[0] if isinstance(res, tuple) else (res or "")
                            elif hasattr(terminal, "get_text_range"):
                                res = terminal.get_text_range(
                                    cmd.output_start_row,
                                    0,
                                    cmd.output_end_row + 1,
                                    col_count,
                                    None,
                                    None,
                                )
                                out_text = res[0] if isinstance(res, tuple) else (res or "")
                        except Exception as e:
                            self.logger.debug(f"Failed to read output range for command: {e}")

                    # Clean up output
                    clean_output = self._sanitize_text(out_text)

                    # Remove the command itself from start of output if present
                    if clean_output.startswith(cmd_text):
                        clean_output = clean_output[len(cmd_text):].lstrip("\r\n")

                    step = RunbookStep(
                        command=cmd_text.strip(),
                        output=clean_output.rstrip("\r\n"),
                        exit_code=cmd.exit_code,
                        duration_str=cmd.formatted_duration,
                        timestamp_str=time.strftime(
                            "%H:%M:%S", time.localtime(cmd.start_time)
                        )
                        if cmd.start_time
                        else "",
                        cwd=cmd.cwd or metadata.host,
                        annotation="",
                        included=True,
                    )
                    steps.append(step)
        except Exception as e:
            self.logger.warning(f"Semantic command extraction failed: {e}")

        # 2. Fallback if no semantic commands could be extracted
        if not steps:
            from .exporter import get_terminal_exporter
            exporter = get_terminal_exporter()
            raw_text = exporter.get_terminal_text(terminal, selection_only=selection_only)
            steps = self._parse_fallback_steps(raw_text)

        metadata.total_commands = len(steps)
        return RunbookData(metadata=metadata, steps=steps)

    def _extract_metadata(self, terminal: Vte.Terminal) -> RunbookMetadata:
        """Gathers runtime metadata for runbook header."""
        from .exporter import get_terminal_exporter
        exporter = get_terminal_exporter()
        raw_meta = exporter.extract_metadata(terminal)

        # Determine default author
        author = ""
        try:
            author = getpass.getuser()
        except Exception:
            author = os.environ.get("USER", "operador")

        # Current timestamp
        now_str = time.strftime("%Y-%m-%d %H:%M:%S")

        default_title = _("Procedimento Operacional / Runbook")

        return RunbookMetadata(
            title=default_title,
            author=author,
            objective="",
            general_notes="",
            conclusion="",
            status=_("Concluído"),
            session_name=raw_meta.get("session_name", _("Sessão Local")),
            host=raw_meta.get("host", "localhost"),
            date_str=now_str,
            total_commands=0,
        )

    def _parse_fallback_steps(self, raw_text: str) -> List[RunbookStep]:
        """
        Fallback parser that segments raw terminal scrollback into steps
        using common prompt regex patterns.
        """
        if not raw_text or not raw_text.strip():
            return []

        # Common prompt patterns: user@host:path$ cmd or prompt ending in $ / # / % / >
        prompt_regex = re.compile(
            r"^(?:\[.*?\]|[\w\.\-]+@[\w\.\-]+:[^\$#\n]*[\$#]|[\w\.\-]+:[^\$#\n]*[\$#]|bash-[0-9\.]+[\$#]|[\$#%>])\s+(.+)$",
            re.MULTILINE,
        )

        steps: List[RunbookStep] = []
        matches = list(prompt_regex.finditer(raw_text))

        if matches:
            for i, match in enumerate(matches):
                cmd_line = match.group(1).strip()
                start_out = match.end()
                end_out = matches[i + 1].start() if i + 1 < len(matches) else len(raw_text)
                out_segment = raw_text[start_out:end_out].strip()

                steps.append(
                    RunbookStep(
                        command=cmd_line,
                        output=out_segment,
                        exit_code=0,
                        duration_str="",
                        timestamp_str="",
                        cwd="",
                        annotation="",
                        included=True,
                    )
                )
        else:
            # Single raw block
            steps.append(
                RunbookStep(
                    command=_("Comandos da Sessão"),
                    output=raw_text.strip(),
                    exit_code=0,
                    duration_str="",
                    timestamp_str="",
                    cwd="",
                    annotation="",
                    included=True,
                )
            )

        return steps

    def _sanitize_text(self, text: str) -> str:
        """Removes ANSI escape codes and normalizes newlines."""
        if not text:
            return ""
        # Strip ANSI escape sequences
        ansi_regex = re.compile(r"\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])")
        cleaned = ansi_regex.sub("", text)
        return cleaned.replace("\r\n", "\n").replace("\r", "\n")

    def render_markdown(
        self,
        runbook: RunbookData,
        collapse_long_outputs: bool = True,
        max_preview_lines: int = 15,
    ) -> str:
        """
        Renders the runbook as a professional, documentation-ready Markdown file.
        Includes title, author, badges, objectives, step-by-step with annotations, and conclusion.
        """
        meta = runbook.metadata
        included_steps = [s for s in runbook.steps if s.included]

        title = meta.title.strip() or _("Caderno de Bordo / Runbook")
        status_badge = meta.status or _("Concluído")

        lines: List[str] = []
        lines.append(f"# 📘 {title}\n")

        # Metadata Header Table / Badges
        lines.append("### 📌 " + _("Informações da Sessão"))
        lines.append(f"- **{_('Status:')}** `{status_badge}`")
        if meta.author:
            lines.append(f"- **{_('Operador / Autor:')}** `{meta.author}`")
        lines.append(f"- **{_('Host:')}** `{meta.host}`")
        if meta.session_name:
            lines.append(f"- **{_('Sessão:')}** `{meta.session_name}`")
        lines.append(f"- **{_('Data / Hora:')}** `{meta.date_str}`")
        lines.append(f"- **{_('Total de Etapas Registradas:')}** `{len(included_steps)}`\n")

        # Objective / Context (if provided)
        if meta.objective and meta.objective.strip():
            lines.append("## 🎯 " + _("Objetivo / Contexto"))
            lines.append(f"{meta.objective.strip()}\n")

        # General Notes (if provided)
        if meta.general_notes and meta.general_notes.strip():
            lines.append("## 📝 " + _("Observações Preliminares"))
            lines.append(f"{meta.general_notes.strip()}\n")

        # Step by Step
        lines.append("## 🚀 " + _("Passo a Passo da Execução"))

        if not included_steps:
            lines.append(f"*{_('Nenhuma etapa selecionada para este documento.')}*\n")
        else:
            for idx, step in enumerate(included_steps, start=1):
                status_icon = "🟢" if step.is_success else "🔴"
                exit_badge = f"`exit {step.exit_code}`" if step.exit_code is not None else ""
                duration_badge = f"`⏱ {step.duration_str}`" if step.duration_str else ""
                time_badge = f"`{step.timestamp_str}`" if step.timestamp_str else ""

                badges = " ".join(b for b in [exit_badge, duration_badge, time_badge] if b)

                lines.append(f"### {idx}. `{step.command}` {status_icon}")
                if badges:
                    lines.append(f"> {badges}\n")

                # User Annotation for this step
                if step.annotation and step.annotation.strip():
                    lines.append(f"> 💬 **{_('Anotação:')}** {step.annotation.strip()}\n")

                # Output formatting
                out_content = step.output.strip()
                if out_content:
                    out_lines = out_content.splitlines()
                    if collapse_long_outputs and len(out_lines) > max_preview_lines:
                        lines.append(
                            f"<details>\n<summary>📄 <b>{_('Saída do Comando')}</b> "
                            f"({len(out_lines)} {_('linhas')})</summary>\n"
                        )
                        lines.append("```bash")
                        lines.append(out_content)
                        lines.append("```\n</details>\n")
                    else:
                        lines.append("```bash")
                        lines.append(out_content)
                        lines.append("```\n")
                else:
                    lines.append(f"*{_('(Sem saída gerada)')}*\n")

        # Conclusion / Post-Mortem (if provided)
        if meta.conclusion and meta.conclusion.strip():
            lines.append("## 🏁 " + _("Conclusão & Lições Aprendidas"))
            lines.append(f"{meta.conclusion.strip()}\n")

        lines.append("---\n*Documento gerado automaticamente pelo OnyxSH Caderno de Bordo.*")
        return "\n".join(lines) + "\n"

    def render_html(
        self,
        runbook: RunbookData,
        collapse_long_outputs: bool = True,
        max_preview_lines: int = 15,
    ) -> str:
        """
        Renders the runbook as a self-contained, responsive, dark-mode HTML document
        optimized for viewing and PDF printing (@media print).
        """
        meta = runbook.metadata
        included_steps = [s for s in runbook.steps if s.included]

        title = html.escape(meta.title.strip() or _("Caderno de Bordo / Runbook"))
        status_text = html.escape(meta.status or _("Concluído"))
        author = html.escape(meta.author or "N/A")
        host = html.escape(meta.host or "localhost")
        session_name = html.escape(meta.session_name or "")
        date_str = html.escape(meta.date_str or "")

        # Build steps HTML
        steps_html = []
        for idx, step in enumerate(included_steps, start=1):
            cmd_escaped = html.escape(step.command)
            status_color = "#27c93f" if step.is_success else "#ff5f56"
            status_tag = f"exit {step.exit_code}" if step.exit_code is not None else "OK"
            duration_badge = f'<span class="badge">⏱ {html.escape(step.duration_str)}</span>' if step.duration_str else ""
            time_badge = f'<span class="badge">🕒 {html.escape(step.timestamp_str)}</span>' if step.timestamp_str else ""

            annotation_box = ""
            if step.annotation and step.annotation.strip():
                ann_escaped = html.escape(step.annotation.strip()).replace("\n", "<br>")
                annotation_box = f"""
                <div class="annotation-box">
                  <strong>💬 {_('Anotação:')}</strong> {ann_escaped}
                </div>
                """

            output_escaped = html.escape(step.output.strip())
            out_lines_count = len(step.output.strip().splitlines()) if step.output.strip() else 0

            if not output_escaped:
                out_block = f'<div class="no-output"><em>{_("(Sem saída gerada)")}</em></div>'
            elif collapse_long_outputs and out_lines_count > max_preview_lines:
                out_block = f"""
                <details class="collapsible-output" open>
                  <summary>📄 {_('Saída do Comando')} ({out_lines_count} {_('linhas')})</summary>
                  <pre class="terminal-box"><code>{output_escaped}</code></pre>
                </details>
                """
            else:
                out_block = f'<pre class="terminal-box"><code>{output_escaped}</code></pre>'

            step_card = f"""
            <div class="step-card">
              <div class="step-header">
                <span class="step-num">{idx}</span>
                <span class="step-cmd"><code>{cmd_escaped}</code></span>
                <span class="badge" style="background-color: {status_color}; color: #000; font-weight: bold;">{status_tag}</span>
                {duration_badge}
                {time_badge}
              </div>
              {annotation_box}
              {out_block}
            </div>
            """
            steps_html.append(step_card)

        steps_joined = "\n".join(steps_html) if steps_html else f"<p><em>{_('Nenhuma etapa selecionada.')}</em></p>"

        # Objective block
        objective_section = ""
        if meta.objective and meta.objective.strip():
            obj_escaped = html.escape(meta.objective.strip()).replace("\n", "<br>")
            objective_section = f"""
            <div class="card section-card">
              <h2>🎯 {_('Objetivo / Contexto')}</h2>
              <p>{obj_escaped}</p>
            </div>
            """

        # General notes block
        notes_section = ""
        if meta.general_notes and meta.general_notes.strip():
            notes_escaped = html.escape(meta.general_notes.strip()).replace("\n", "<br>")
            notes_section = f"""
            <div class="card section-card">
              <h2>📝 {_('Observações Preliminares')}</h2>
              <p>{notes_escaped}</p>
            </div>
            """

        # Conclusion block
        conclusion_section = ""
        if meta.conclusion and meta.conclusion.strip():
            conc_escaped = html.escape(meta.conclusion.strip()).replace("\n", "<br>")
            conclusion_section = f"""
            <div class="card section-card">
              <h2>🏁 {_('Conclusão & Lições Aprendidas')}</h2>
              <p>{conc_escaped}</p>
            </div>
            """

        html_template = f"""<!DOCTYPE html>
<html lang="pt-BR">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title} - OnyxSH</title>
  <style>
    :root {{
      --bg: #111318;
      --card-bg: #1b1e26;
      --border: #2d3342;
      --text: #e6edf3;
      --text-muted: #8b949e;
      --accent: #3584e4;
      --accent-badge: #263852;
      --annotation-bg: #1c2a38;
      --annotation-border: #3584e4;
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      background-color: var(--bg);
      color: var(--text);
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
      padding: 32px 20px;
      line-height: 1.6;
    }}
    .container {{
      max-width: 1040px;
      margin: 0 auto;
    }}
    .card {{
      background-color: var(--card-bg);
      border: 1px solid var(--border);
      border-radius: 10px;
      padding: 24px;
      margin-bottom: 20px;
      box-shadow: 0 4px 12px rgba(0,0,0,0.3);
    }}
    .header-card {{
      display: flex;
      flex-direction: column;
      gap: 16px;
    }}
    .title-row {{
      display: flex;
      justify-content: space-between;
      align-items: center;
      flex-wrap: wrap;
      gap: 12px;
    }}
    h1 {{ font-size: 1.6rem; color: #fff; font-weight: 700; }}
    h2 {{ font-size: 1.25rem; color: #fff; margin-bottom: 12px; font-weight: 600; }}
    .meta-grid {{
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
      gap: 12px;
      font-size: 0.9rem;
      border-top: 1px solid var(--border);
      padding-top: 16px;
    }}
    .meta-item {{ color: var(--text-muted); }}
    .meta-item strong {{ color: var(--text); }}
    .badge {{
      display: inline-block;
      padding: 3px 8px;
      border-radius: 6px;
      font-size: 0.78rem;
      background-color: var(--accent-badge);
      color: #90c2ff;
    }}
    .step-card {{
      background-color: var(--card-bg);
      border: 1px solid var(--border);
      border-radius: 8px;
      margin-bottom: 16px;
      overflow: hidden;
    }}
    .step-header {{
      background-color: #20242e;
      padding: 12px 18px;
      display: flex;
      align-items: center;
      gap: 12px;
      flex-wrap: wrap;
      border-bottom: 1px solid var(--border);
    }}
    .step-num {{
      font-weight: 700;
      color: var(--accent);
      background-color: rgba(53, 132, 228, 0.15);
      border-radius: 50%;
      width: 26px;
      height: 26px;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      font-size: 0.85rem;
    }}
    .step-cmd code {{
      font-family: "Cascadia Code", "Fira Code", monospace;
      font-size: 0.95rem;
      color: #79c0ff;
    }}
    .annotation-box {{
      background-color: var(--annotation-bg);
      border-left: 4px solid var(--annotation-border);
      padding: 12px 16px;
      font-size: 0.9rem;
      color: #d2e7ff;
      border-bottom: 1px solid var(--border);
    }}
    pre.terminal-box {{
      background-color: #0b0d13;
      padding: 16px;
      overflow-x: auto;
      font-family: "Cascadia Code", "Fira Code", monospace;
      font-size: 0.85rem;
      line-height: 1.45;
      color: #c9d1d9;
    }}
    details.collapsible-output summary {{
      background-color: #141720;
      padding: 8px 16px;
      cursor: pointer;
      font-size: 0.85rem;
      color: var(--text-muted);
      border-bottom: 1px solid var(--border);
      user-select: none;
    }}
    .no-output {{
      padding: 12px 16px;
      font-size: 0.85rem;
      color: var(--text-muted);
    }}
    .footer {{
      text-align: center;
      font-size: 0.8rem;
      color: var(--text-muted);
      margin-top: 32px;
      padding-top: 16px;
      border-top: 1px solid var(--border);
    }}
    @media print {{
      body {{ background-color: #fff; color: #000; padding: 0; }}
      .card, .step-card {{ border: 1px solid #ccc; box-shadow: none; background: #fff; color: #000; }}
      .step-header {{ background-color: #f2f2f2; border-bottom: 1px solid #ccc; }}
      pre.terminal-box {{ background-color: #fafafa; color: #000; border: 1px solid #eee; }}
      details.collapsible-output summary {{ display: none; }}
      details.collapsible-output[open] pre {{ display: block; }}
    }}
  </style>
</head>
<body>
  <div class="container">
    <div class="card header-card">
      <div class="title-row">
        <h1>📘 {title}</h1>
        <span class="badge" style="background-color: #27c93f; color: #000; font-weight: bold; font-size: 0.85rem;">{status_text}</span>
      </div>
      <div class="meta-grid">
        <div class="meta-item"><strong>{_('Operador:')}</strong> {author}</div>
        <div class="meta-item"><strong>{_('Host:')}</strong> {host}</div>
        <div class="meta-item"><strong>{_('Data:')}</strong> {date_str}</div>
        <div class="meta-item"><strong>{_('Etapas:')}</strong> {len(included_steps)}</div>
      </div>
    </div>

    {objective_section}
    {notes_section}

    <div class="card">
      <h2>🚀 {_('Passo a Passo da Execução')}</h2>
      {steps_joined}
    </div>

    {conclusion_section}

    <div class="footer">
      OnyxSH — {_('Caderno de Bordo & Runbook Operacional')}
    </div>
  </div>
</body>
</html>
"""
        return html_template

    def render_log(self, runbook: RunbookData) -> str:
        """Renders runbook as a structured plain text log report."""
        meta = runbook.metadata
        included_steps = [s for s in runbook.steps if s.included]

        title = meta.title.strip() or _("Caderno de Bordo / Runbook")
        lines: List[str] = [
            "=" * 80,
            f" {title} [{meta.status}]",
            f" {_('Operador:')} {meta.author} | {_('Host:')} {meta.host} | {_('Data:')} {meta.date_str}",
            "=" * 80,
            "",
        ]

        if meta.objective and meta.objective.strip():
            lines.append(f"[{_('OBJETIVO / CONTEXTO')}]")
            lines.append(meta.objective.strip())
            lines.append("")

        lines.append(f"[{_('PASSOS EXECUTADOS')} ({len(included_steps)})]")
        lines.append("-" * 80)

        for idx, step in enumerate(included_steps, start=1):
            status = f"exit {step.exit_code}" if step.exit_code is not None else "OK"
            dur = f" ({step.duration_str})" if step.duration_str else ""
            lines.append(f"#{idx} $ {step.command} [{status}]{dur}")

            if step.annotation and step.annotation.strip():
                lines.append(f"   💬 {_('Nota:')} {step.annotation.strip()}")

            if step.output.strip():
                lines.append("   --- Saída ---")
                for out_line in step.output.strip().splitlines():
                    lines.append(f"   | {out_line}")
                lines.append("   -------------\n")
            else:
                lines.append(f"   {_('(Sem saída)')}\n")

        if meta.conclusion and meta.conclusion.strip():
            lines.append(f"[{_('CONCLUSÃO & OBSERVAÇÕES')}]")
            lines.append(meta.conclusion.strip())
            lines.append("")

        lines.append("=" * 80)
        return "\n".join(lines) + "\n"


_global_runbook_generator: Optional[RunbookGenerator] = None


def get_runbook_generator() -> RunbookGenerator:
    """Returns singleton instance of RunbookGenerator."""
    global _global_runbook_generator
    if _global_runbook_generator is None:
        _global_runbook_generator = RunbookGenerator()
    return _global_runbook_generator
