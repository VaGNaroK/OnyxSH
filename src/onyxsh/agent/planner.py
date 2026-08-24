"""Plan parser and validator for converting LLM output into ActionPlan instances."""

from __future__ import annotations

import json
import re
import uuid
from typing import Optional, Union

from .models import ActionPlan, ActionStep, RiskLevel


_JSON_BLOCK_PATTERN = re.compile(r"```(?:json)?\s*(\{[\s\S]*?\})\s*```", re.IGNORECASE)
_CODE_BLOCK_PATTERN = re.compile(r"```(\w*)\n?(.*?)```", re.DOTALL)


def strip_json_comments_and_commas(text: str) -> str:
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


def extract_json_object(raw_text: str) -> Optional[dict]:
    """Attempt to extract a JSON dictionary from raw model text, stripping comments."""
    if not raw_text or not raw_text.strip():
        return None

    cleaned = strip_json_comments_and_commas(raw_text.strip())

    # 1. Direct JSON parse
    try:
        data = json.loads(cleaned)
        if isinstance(data, dict):
            return data
    except Exception:
        pass

    # 2. Extract from markdown code fence
    match = _JSON_BLOCK_PATTERN.search(cleaned)
    if match:
        try:
            data = json.loads(strip_json_comments_and_commas(match.group(1)))
            if isinstance(data, dict):
                return data
        except Exception:
            pass

    # 3. Find outermost curly braces { ... }
    first_brace = cleaned.find("{")
    last_brace = cleaned.rfind("}")
    if first_brace != -1 and last_brace != -1 and last_brace > first_brace:
        try:
            data = json.loads(strip_json_comments_and_commas(cleaned[first_brace : last_brace + 1]))
            if isinstance(data, dict):
                return data
        except Exception:
            pass

    return None


def is_multi_line_script(code_body: str) -> bool:
    """Detects if a code block is a multi-line script rather than a list of CLI commands."""
    lines = [line.strip() for line in code_body.splitlines() if line.strip()]
    if not lines:
        return False

    if lines[0].startswith("#!"):
        return True

    script_keywords = (
        r'^\s*(?:if\s+\[|if\s+\[\[|if\s+test|\b(?:then|else|elif|fi|do|done|esac)\b|case\s+.*in\b)',
        r'^\s*(?:function\s+\w+|\w+\s*\(\))\s*\{?',
        r'^\s*(?:\d+|\*|[a-zA-Z])\)\s*$',
        r'^\s*;;(?:\&)?\s*$',
        r'^\s*(?:local|declare|typeset)\s+\w+=?',
    )
    structural_count = 0
    for line in lines:
        for pat in script_keywords:
            if re.search(pat, line):
                structural_count += 1
                break

    return structural_count >= 2


def is_valid_cli_command(line: str) -> bool:
    """Validates whether a line is a genuine standalone executable command."""
    cleaned = line.strip()
    if not cleaned:
        return False

    if cleaned.startswith("#") or cleaned.startswith("//") or cleaned.startswith(";"):
        return False

    if cleaned in (
        "{", "}", "(", ")", "[", "]", ";;", ";;&", ";&", "fi", "then",
        "else", "elif", "do", "done", "esac"
    ):
        return False

    if re.match(r'^(?:\d+|\*|[a-zA-Z])\)\s*$', cleaned):
        return False

    if re.match(r'^(?:function\s+\w+|\w+\s*\(\))\s*\{?\s*$', cleaned):
        return False

    if re.match(r'^(?:if\b|while\b|for\b|until\b|case\b|select\b)', cleaned):
        return False

    if re.match(r'^(?:local|declare|typeset)\s+\w+', cleaned):
        return False

    if re.search(r'(?<!<)<[a-zA-Z0-9_\.\-\s\u00C0-\u00FF]+>(?!>)', cleaned):
        return False

    if re.match(r'^[=\-_*#]{3,}.*[=\-_*#]{3,}$', cleaned):
        return False

    if re.match(r'^\d+\.\s+.*$', cleaned) and not re.match(r'^\d+\.\s+(?:sudo|apt|chmod|cd|ls|curl|git|docker)\b', cleaned):
        return False

    return True


def repair_heredoc_script(heredoc_text: str, full_scripts: list[str]) -> Optional[str]:
    """
    If a heredoc command contains placeholder lines (e.g. '...', '... (inserir...)'),
    replaces the placeholder with the actual full script found in the response.
    """
    match = re.match(
        r'^(.*?(?:cat|tee)\s+<<\s*[\'"]?(\w+)[\'"]?\s*(?:>|>>)\s*[^\n]+\n)([\s\S]*?)(?:\n\s*\2\s*)$',
        heredoc_text.strip(),
        re.DOTALL
    )
    if not match:
        return heredoc_text

    header, delimiter, body = match.groups()
    has_placeholder = any(
        re.match(r'^\s*(?:\.\.\.|\.\.\.\s*\(|\<inserir|\<insert|\/\/ code here|\# insert|\# \.\.\.).*$', line, re.IGNORECASE)
        for line in body.splitlines()
    )

    if has_placeholder:
        if full_scripts:
            return f"{header}{full_scripts[0]}\n{delimiter}"
        return None

    return heredoc_text


def extract_commands_from_code_body(code_body: str, full_scripts: list[str]) -> list[str]:
    """Extracts CLI commands and properly handles heredoc blocks without splitting them line-by-line."""
    extracted: list[str] = []
    lines = code_body.splitlines()
    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()
        if not stripped:
            i += 1
            continue

        heredoc_start = re.match(r'^(?:sudo\s+)?(?:cat|tee)\s+<<\s*[\'"]?(\w+)[\'"]?', stripped)
        if heredoc_start:
            delim = heredoc_start.group(1)
            heredoc_lines = [line]
            i += 1
            while i < len(lines):
                curr = lines[i]
                heredoc_lines.append(curr)
                if curr.strip() == delim:
                    i += 1
                    break
                i += 1
            full_heredoc = "\n".join(heredoc_lines)
            repaired = repair_heredoc_script(full_heredoc, full_scripts)
            if repaired:
                extracted.append(repaired)
            continue

        if stripped.endswith("\\"):
            cont_lines = [stripped]
            i += 1
            while i < len(lines):
                next_line = lines[i].strip()
                cont_lines.append(next_line)
                i += 1
                if not next_line.endswith("\\"):
                    break
            joined = " ".join(l.rstrip("\\").strip() for l in cont_lines)
            if is_valid_cli_command(joined):
                extracted.append(joined)
            continue

        if is_valid_cli_command(stripped):
            extracted.append(stripped)
        i += 1

    return extracted


class PlanParser:
    """Parses and validates LLM output into structured ActionPlan objects."""

    @staticmethod
    def parse(
        raw_text: str,
        provider_name: str = "",
    ) -> Union[ActionPlan, str]:
        """
        Parse raw model response into an ActionPlan or return pure text fallback.

        Args:
            raw_text: Model output text.
            provider_name: Identifier of the provider.

        Returns:
            ActionPlan if a valid plan structure was decoded, or fallback string.
        """
        json_data = extract_json_object(raw_text)
        if json_data is None:
            # Fallback for models outputting markdown code blocks with bash commands
            code_block_matches = list(_CODE_BLOCK_PATTERN.finditer(raw_text))

            # First collect full multi-line scripts
            full_scripts: list[str] = []
            for m in code_block_matches:
                lang = m.group(1).lower() if m.group(1) else ""
                code_body = m.group(2).strip()
                if lang in ("bash", "sh", "shell", "zsh", "") and code_body:
                    if is_multi_line_script(code_body) and not re.match(r'^(?:sudo\s+)?(?:cat|tee)\s+<<', code_body):
                        full_scripts.append(code_body)

            detected_commands: list[str] = []
            for m in code_block_matches:
                lang = m.group(1).lower() if m.group(1) else ""
                code_body = m.group(2).strip()
                if lang in ("bash", "sh", "shell", "zsh", "") and code_body:
                    if is_multi_line_script(code_body) and not re.search(r'(?:cat|tee)\s+<<', code_body):
                        continue

                    cmds = extract_commands_from_code_body(code_body, full_scripts)
                    detected_commands.extend(cmds)

            # If full script was provided and commands reference chmod/execution without a creation step, synthesize creation
            if full_scripts:
                has_creation_cmd = any("<<" in c or ">" in c for c in detected_commands)
                if not has_creation_cmd:
                    target_script_name = None
                    for cmd_str in detected_commands:
                        match_script = re.search(r'(?:chmod\s+\+x\s+|(?:\./|\. |bash\s+))([~/\w\.\-]+\.sh)', cmd_str)
                        if match_script:
                            target_script_name = match_script.group(1)
                            break
                    if target_script_name:
                        synth_cmd = f"cat << 'EOF' > {target_script_name}\n{full_scripts[0]}\nEOF"
                        detected_commands.insert(0, synth_cmd)

            if len(detected_commands) >= 1:
                steps = []
                for i, cmd_str in enumerate(detected_commands):
                    argv = [tok for tok in cmd_str.split() if tok]
                    steps.append(
                        ActionStep(
                            step_id=f"step_{i+1}",
                            tool="shell.run",
                            argv=argv,
                            description=cmd_str,
                            risk=RiskLevel.USER_WRITE,
                        )
                    )
                return ActionPlan(
                    plan_id=f"plan_{uuid.uuid4().hex[:8]}",
                    intent="Plano de comandos sugeridos",
                    summary=raw_text.strip(),
                    steps=steps,
                    provider=provider_name,
                )

            return raw_text.strip()

        # Handle legacy OnyxSH format: {"reply": "...", "commands": ["..."]}
        if "reply" in json_data and "steps" not in json_data:
            summary = json_data.get("reply", "")
            raw_commands = json_data.get("commands", [])
            steps = []
            for i, cmd in enumerate(raw_commands):
                cmd_str = cmd if isinstance(cmd, str) else cmd.get("command", "")
                if cmd_str:
                    argv = [tok for tok in cmd_str.split() if tok]
                    steps.append(
                        ActionStep(
                            step_id=f"step_{i+1}",
                            tool="shell.run",
                            argv=argv,
                            description=f"Executar comando: {cmd_str}",
                            risk=RiskLevel.USER_WRITE,
                        )
                    )
            return ActionPlan(
                plan_id=f"plan_{uuid.uuid4().hex[:8]}",
                intent="Execução sugerida",
                summary=summary,
                steps=steps,
                provider=provider_name,
            )

        # Standard ActionPlan schema format
        try:
            if "plan_id" not in json_data:
                json_data["plan_id"] = f"plan_{uuid.uuid4().hex[:8]}"
            if "provider" not in json_data:
                json_data["provider"] = provider_name

            plan = ActionPlan.from_dict(json_data)
            return plan
        except Exception:
            # If ActionPlan schema validation fails, fallback to clean text
            return raw_text.strip()
