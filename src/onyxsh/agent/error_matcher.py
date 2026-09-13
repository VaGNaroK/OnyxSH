# onyxsh/agent/error_matcher.py
"""
Heuristic Error Pattern Matcher and Proactive Suggestion Engine for OnyxSH Terminal.
Detects common Linux terminal errors, extracts dynamic parameters (e.g. ports, missing packages),
and proposes instant zero-latency quick fixes and AI-assisted diagnoses.
"""

from __future__ import annotations

import enum
import re
import shlex
from dataclasses import dataclass
from typing import Any, Dict, List, Optional

from ..utils.logger import get_logger
from ..utils.translation_utils import _

logger = get_logger("onyxsh.agent.error_matcher")


class ErrorCategory(str, enum.Enum):
    PERMISSION_DENIED = "permission_denied"
    COMMAND_NOT_FOUND = "command_not_found"
    PORT_IN_USE = "port_in_use"
    NO_SPACE_LEFT = "no_space_left"
    FILE_NOT_FOUND = "file_not_found"
    CONNECTION_REFUSED = "connection_refused"
    PACKAGE_MISSING = "package_missing"
    GIT_ERROR = "git_error"
    GENERIC_ERROR = "generic_error"


@dataclass
class ErrorMatch:
    """Represents a matched terminal error with suggested quick fixes and AI hints."""

    category: ErrorCategory
    title: str
    description: str
    quick_fix_command: Optional[str] = None
    quick_fix_label: Optional[str] = None
    extracted_target: Optional[str] = None
    ai_prompt_hint: str = ""
    confidence: float = 1.0
    exit_code: int = 1
    command: str = ""
    output_snippet: str = ""

    @property
    def has_quick_fix(self) -> bool:
        return bool(self.quick_fix_command and self.quick_fix_command.strip())

    def to_dict(self) -> Dict[str, Any]:
        return {
            "category": self.category.value,
            "title": self.title,
            "description": self.description,
            "quick_fix_command": self.quick_fix_command,
            "quick_fix_label": self.quick_fix_label,
            "extracted_target": self.extracted_target,
            "ai_prompt_hint": self.ai_prompt_hint,
            "confidence": self.confidence,
            "exit_code": self.exit_code,
            "command": self.command,
            "output_snippet": self.output_snippet,
        }


class TerminalErrorMatcher:
    """Evaluates executed commands, exit codes, and output text to detect known error patterns."""

    def __init__(self) -> None:
        self.logger = logger

    def match(
        self,
        command: str,
        exit_code: int,
        output: str,
        cwd: str = "",
    ) -> Optional[ErrorMatch]:
        """
        Analyzes a finished command with exit_code != 0 and returns an ErrorMatch.
        Returns None if exit_code == 0.
        """
        if exit_code == 0:
            return None

        cmd_clean = (command or "").strip()
        out_text = (output or "").strip()
        combined_text = f"{cmd_clean}\n{out_text}".lower()

        # 1. PERMISSION DENIED
        match_perm = self._check_permission_denied(cmd_clean, out_text, exit_code, combined_text)
        if match_perm:
            return match_perm

        # 2. COMMAND NOT FOUND
        match_cnf = self._check_command_not_found(cmd_clean, out_text, exit_code, combined_text)
        if match_cnf:
            return match_cnf

        # 3. PORT / ADDRESS ALREADY IN USE
        match_port = self._check_port_in_use(cmd_clean, out_text, exit_code, combined_text)
        if match_port:
            return match_port

        # 4. NO SPACE LEFT ON DEVICE
        match_space = self._check_no_space_left(cmd_clean, out_text, exit_code, combined_text)
        if match_space:
            return match_space

        # 5. MISSING PACKAGE / MODULE (Python, Node)
        match_pkg = self._check_package_missing(cmd_clean, out_text, exit_code, out_text)
        if match_pkg:
            return match_pkg

        # 6. FILE OR DIRECTORY NOT FOUND
        match_fnf = self._check_file_not_found(cmd_clean, out_text, exit_code, combined_text)
        if match_fnf:
            return match_fnf

        # 7. CONNECTION REFUSED / UNREACHABLE
        match_conn = self._check_connection_refused(cmd_clean, out_text, exit_code, combined_text)
        if match_conn:
            return match_conn

        # 8. GIT REPOSITORY / MERGE CONFLICT ERRORS
        match_git = self._check_git_error(cmd_clean, out_text, exit_code, combined_text)
        if match_git:
            return match_git

        # 9. GENERIC ERROR FALLBACK
        return self._build_generic_error(cmd_clean, out_text, exit_code)

    # -------------------------------------------------------------------------
    # Pattern Checkers
    # -------------------------------------------------------------------------

    def _check_permission_denied(
        self, cmd: str, out: str, exit_code: int, combined: str
    ) -> Optional[ErrorMatch]:
        patterns = [
            r"\bpermission\s+denied\b",
            r"\bpermiss[ãa]o\s+negada\b",
            r"\boperation\s+not\s+permitted\b",
            r"\bopera[çc][ãa]o\s+n[ãa]o\s+permitida\b",
            r"\beacces\b",
            r"\beperm\b",
            r"\bare\s+you\s+root\b",
            r"\bmust\s+be\s+run\s+as\s+root\b",
            r"\broot\s+privileges\s+required\b",
            r"\baccess\s+denied\b",
        ]
        has_sudo = bool(re.match(r"^(sudo|pkexec|doas)\b", cmd.strip()))

        # Direct root file target or package manager check without sudo
        is_root_target = bool(
            re.search(r"(?:^|\s)/(?:etc/(?:shadow|gshadow|sudoers)|root/)", cmd)
        )
        is_root_pkg_mgr = bool(
            re.search(
                r"^(?:apt|apt-get|dpkg|dnf|pacman|systemctl\s+(?:start|stop|restart|reload|enable|disable))\b",
                cmd.strip(),
            )
        )

        matched = any(re.search(p, combined) for p in patterns) or (
            not has_sudo
            and exit_code in (1, 13, 126)
            and (is_root_target or is_root_pkg_mgr)
        )

        if matched:
            quick_fix = None
            quick_fix_label = None

            if not has_sudo and cmd:
                quick_fix = f"sudo {cmd}"
                quick_fix_label = _("⚡ Executar com sudo")
            else:
                quick_fix_label = _("🤖 Diagnosticar com IA")

            snippet = out[:200] if out else ""
            return ErrorMatch(
                category=ErrorCategory.PERMISSION_DENIED,
                title=_("Permissão Negada"),
                description=_(
                    "O comando falhou por falta de privilégios de superusuário (root) ou permissão de escrita."
                ),
                quick_fix_command=quick_fix,
                quick_fix_label=quick_fix_label,
                ai_prompt_hint=_(
                    "O comando falhou com Permissão Negada (Permission Denied). Verifique se elevar com sudo resolve, "
                    "ou se é necessário alterar permissões de arquivo/diretório com chmod ou chown."
                ),
                exit_code=exit_code,
                command=cmd,
                output_snippet=snippet,
            )
        return None

    def _check_command_not_found(
        self, cmd: str, out: str, exit_code: int, combined: str
    ) -> Optional[ErrorMatch]:
        cnf_patterns = [
            r"\bcommand\s+not\s+found\b",
            r"\bcomando\s+n[ãa]o\s+encontrado\b",
            r"bash:\s+([^:]+):\s+(?:command\s+not\s+found|n[ãa]o\s+encontrado)",
            r"zsh:\s+command\s+not\s+found:\s+([^\n\r]+)",
            r"no\s+command\s+'([^']+)'\s+found",
        ]
        is_cnf = exit_code == 127 or any(re.search(p, combined) for p in cnf_patterns)
        if not is_cnf:
            return None

        missing_cmd = ""
        # Try extracting missing command name
        for p in (
            r"zsh:\s+command\s+not\s+found:\s+([^\s\n\r]+)",
            r"bash:\s+([^\s:]+):\s+(?:command\s+not\s+found|n[ãa]o\s+encontrado)",
            r"no\s+command\s+'([^']+)'\s+found",
        ):
            m = re.search(p, out, re.IGNORECASE)
            if m:
                missing_cmd = m.group(1).strip()
                break

        if not missing_cmd and cmd:
            # Fallback: extract primary executable from command line
            try:
                tokens = shlex.split(cmd)
                if tokens:
                    # Skip env vars prefix (e.g. VAR=val cmd)
                    for tok in tokens:
                        if "=" not in tok or tok.startswith("./") or tok.startswith("/"):
                            missing_cmd = tok
                            break
            except Exception:
                parts = cmd.split()
                if parts:
                    missing_cmd = parts[0]

        quick_fix = None
        quick_fix_label = None
        if missing_cmd:
            quick_fix = f"apt search {missing_cmd}"
            quick_fix_label = _("🔍 Buscar Pacote")
        else:
            quick_fix_label = _("🤖 Diagnosticar com IA")

        snippet = out[:200] if out else ""
        desc = (
            _("O comando '{cmd_name}' não está instalado ou não foi encontrado no PATH.").format(
                cmd_name=missing_cmd or cmd
            )
            if missing_cmd
            else _("O comando executado não foi encontrado no sistema.")
        )

        return ErrorMatch(
            category=ErrorCategory.COMMAND_NOT_FOUND,
            title=_("Comando Não Encontrado"),
            description=desc,
            quick_fix_command=quick_fix,
            quick_fix_label=quick_fix_label,
            extracted_target=missing_cmd,
            ai_prompt_hint=_(
                "O comando '{cmd_name}' não foi encontrado. Identifique qual pacote Linux (apt, dnf, pacman, snap, pip, cargo) "
                "fornece este binário e sugira o comando seguro de instalação."
            ).format(cmd_name=missing_cmd or cmd),
            exit_code=exit_code,
            command=cmd,
            output_snippet=snippet,
        )

    def _check_port_in_use(
        self, cmd: str, out: str, exit_code: int, combined: str
    ) -> Optional[ErrorMatch]:
        port_patterns = [
            r"\baddress\s+already\s+in\s+use\b",
            r"\bendere[çc]o\s+j[áa]\s+em\s+uso\b",
            r"\bbind:\s+address\s+already\s+in\s+use\b",
            r"\blisten\s+tcp\s+.*:(\d+):\s+bind:\s+address\s+already\s+in\s+use\b",
            r"\bport\s+(\d+)\s+(?:is\s+)?already\s+in\s+use\b",
            r"\bporta\s+(\d+)\s+j[áa]\s+est[áa]\s+em\s+uso\b",
            r"\beaddrinuse\b",
        ]
        matched = False
        port = ""
        for p in port_patterns:
            m = re.search(p, combined)
            if m:
                matched = True
                if m.groups():
                    port = m.group(1)
                break

        if not matched:
            return None

        # If port wasn't in the error message regex, try to extract port number from command
        if not port:
            port_m = re.search(r"(?:-p\s+|--port[=\s]+|:\s*|http\.server\s+)(\d{2,5})\b", cmd)
            if port_m:
                port = port_m.group(1)
            else:
                candidates = re.findall(r"\b(\d{2,5})\b", cmd)
                if candidates:
                    port = candidates[-1]

        quick_fix = f"lsof -i :{port}" if port else "ss -tulpn"
        quick_fix_label = _("🔍 Inspecionar Porta {port}").format(port=f":{port}" if port else "")

        desc = (
            _("A porta de rede {port} já está sendo utilizada por outro processo.").format(port=f":{port}")
            if port
            else _("O endereço ou porta de rede solicitado já está sendo utilizado por outro processo.")
        )

        return ErrorMatch(
            category=ErrorCategory.PORT_IN_USE,
            title=_("Porta Já em Uso"),
            description=desc,
            quick_fix_command=quick_fix,
            quick_fix_label=quick_fix_label,
            extracted_target=port,
            ai_prompt_hint=_(
                "A porta de rede {port} está ocupada (Address already in use). "
                "Ajude a identificar qual processo está rodando nela e mostre como liberar a porta ou usar outra."
            ).format(port=f":{port}" if port else ""),
            exit_code=exit_code,
            command=cmd,
            output_snippet=out[:200],
        )

    def _check_no_space_left(
        self, cmd: str, out: str, exit_code: int, combined: str
    ) -> Optional[ErrorMatch]:
        space_patterns = [
            r"\bno\s+space\s+left\s+on\s+device\b",
            r"\bn[ãa]o\s+h[áa]\s+espa[çc]o\s+dispon[íi]vel\b",
            r"\bespa[çc]o\s+em\s+disco\s+esgotado\b",
            r"\benospc\b",
            r"\bdisk\s+quota\s+exceeded\b",
        ]
        if any(re.search(p, combined) for p in space_patterns):
            return ErrorMatch(
                category=ErrorCategory.NO_SPACE_LEFT,
                title=_("Espaço em Disco Esgotado"),
                description=_("O sistema de arquivos ou a partição de disco não possui espaço livre para esta operação."),
                quick_fix_command="df -h",
                quick_fix_label=_("💾 Verificar Disco (df -h)"),
                ai_prompt_hint=_(
                    "O disco está sem espaço livre (No space left on device). "
                    "Sugira comandos para verificar as partições mais cheias (df -h) e encontrar arquivos pesados para limpeza segura."
                ),
                exit_code=exit_code,
                command=cmd,
                output_snippet=out[:200],
            )
        return None

    def _check_package_missing(
        self, cmd: str, out: str, exit_code: int, out_raw: str
    ) -> Optional[ErrorMatch]:
        # Python ModuleNotFoundError / ImportError
        py_m = re.search(
            r"\b(?:ModuleNotFoundError|ImportError):\s+No\s+module\s+named\s+['\"]?([^'\"\s\n\r]+)['\"]?",
            out_raw,
            re.IGNORECASE,
        )
        if not py_m and (
            "modulenotfounderror" in out_raw.lower()
            or "no module named" in out_raw.lower()
        ):
            py_m = re.search(
                r"no\s+module\s+named\s+['\"]?([^'\"\s\n\r]+)['\"]?",
                out_raw,
                re.IGNORECASE,
            )

        # Fallback for python -c "import X" when command failed with exit_code != 0
        if not py_m and exit_code != 0:
            cmd_import_m = re.search(
                r"python[3]?\s+(?:-[c|m]\s+)?['\"]?(?:from\s+([^\s;'\"]+)\s+import|import\s+([^\s;'\"]+))",
                cmd,
            )
            if cmd_import_m:
                extracted = cmd_import_m.group(1) or cmd_import_m.group(2)
                if extracted and (
                    not out_raw
                    or "modulenotfounderror" in out_raw.lower()
                    or "no module" in out_raw.lower()
                    or "error" in out_raw.lower()
                    or not out_raw.strip()
                ):
                    pkg = extracted.split(".")[0].strip()
                    return ErrorMatch(
                        category=ErrorCategory.PACKAGE_MISSING,
                        title=_("Módulo Python Ausente"),
                        description=_(
                            "O módulo Python '{pkg}' não está instalado no ambiente."
                        ).format(pkg=pkg),
                        quick_fix_command=f"pip install {pkg}",
                        quick_fix_label=_("📦 pip install {pkg}").format(pkg=pkg),
                        extracted_target=pkg,
                        ai_prompt_hint=_(
                            "O script Python falhou por falta do módulo '{pkg}'. "
                            "Sugira como instalá-lo via pip ou no ambiente virtual apropriado."
                        ).format(pkg=pkg),
                        exit_code=exit_code,
                        command=cmd,
                        output_snippet=out[:200],
                    )

        if py_m:
            pkg = py_m.group(1).split(".")[0].strip()
            return ErrorMatch(
                category=ErrorCategory.PACKAGE_MISSING,
                title=_("Módulo Python Ausente"),
                description=_(
                    "O módulo Python '{pkg}' não está instalado no ambiente."
                ).format(pkg=pkg),
                quick_fix_command=f"pip install {pkg}",
                quick_fix_label=_("📦 pip install {pkg}").format(pkg=pkg),
                extracted_target=pkg,
                ai_prompt_hint=_(
                    "O script Python falhou por falta do módulo '{pkg}'. "
                    "Sugira como instalá-lo via pip ou no ambiente virtual apropriado."
                ).format(pkg=pkg),
                exit_code=exit_code,
                command=cmd,
                output_snippet=out[:200],
            )

        # Node / NPM missing module
        node_m = re.search(
            r"\bCannot\s+find\s+module\s+['\"]([^'\"]+)['\"]",
            out_raw,
            re.IGNORECASE,
        )
        if node_m:
            pkg = node_m.group(1)
            return ErrorMatch(
                category=ErrorCategory.PACKAGE_MISSING,
                title=_("Módulo Node.js Ausente"),
                description=_("O módulo Node.js '{pkg}' não foi encontrado.").format(pkg=pkg),
                quick_fix_command=f"npm install {pkg}",
                quick_fix_label=_("📦 npm install {pkg}").format(pkg=pkg),
                extracted_target=pkg,
                ai_prompt_hint=_(
                    "O script Node.js falhou por falta do módulo '{pkg}'. "
                    "Sugira como instalá-lo via npm ou yarn."
                ).format(pkg=pkg),
                exit_code=exit_code,
                command=cmd,
                output_snippet=out[:200],
            )

        return None

    def _check_file_not_found(
        self, cmd: str, out: str, exit_code: int, combined: str
    ) -> Optional[ErrorMatch]:
        fnf_patterns = [
            r"\bno\s+such\s+file\s+or\s+directory\b",
            r"\barquivo\s+ou\s+diret[óo]rio\s+n[ãa]o\s+encontrado\b",
            r"\barquivo\s+ou\s+diret[óo]rio\s+inexistente\b",
            r"\benoent\b",
        ]
        if any(re.search(p, combined) for p in fnf_patterns):
            return ErrorMatch(
                category=ErrorCategory.FILE_NOT_FOUND,
                title=_("Arquivo Não Encontrado"),
                description=_("O arquivo ou caminho de diretório especificado não existe ou o caminho relativo está incorreto."),
                quick_fix_command="ls -la",
                quick_fix_label=_("📂 Listar Diretório (ls -la)"),
                ai_prompt_hint=_(
                    "O comando falhou porque o arquivo ou diretório não existe (No such file or directory). "
                    "Verifique caminhos relativos, erros de digitação e sugira validações com ls."
                ),
                exit_code=exit_code,
                command=cmd,
                output_snippet=out[:200],
            )
        return None

    def _check_connection_refused(
        self, cmd: str, out: str, exit_code: int, combined: str
    ) -> Optional[ErrorMatch]:
        conn_patterns = [
            r"\bconnection\s+refused\b",
            r"\bconex[ãa]o\s+recusada\b",
            r"\beconnrefused\b",
            r"\bnetwork\s+is\s+unreachable\b",
            r"\brede\s+inalcan[çc][áa]vel\b",
            r"\bcould\s+not\s+resolve\s+host\b",
            r"\bn[ãa]o\s+foi\s+poss[íi]vel\s+resolver\s+o\s+host\b",
            r"\bconnection\s+timed\s+out\b",
            r"\bfailed\s+to\s+connect(?:\s+to)?\b",
            r"\bcouldn't\s+connect\s+to\s+server\b",
            r"\bn[ãa]o\s+foi\s+poss[íi]vel\s+conectar\b",
            r"\bcurl:\s*\(\s*7\s*\)",
            r"\bcurl:\s*\(\s*28\s*\)",
        ]
        if any(re.search(p, combined) for p in conn_patterns):
            port = ""
            port_m = re.search(r"(?:port\s+|:)(\d{2,5})\b", combined)
            if port_m:
                port = port_m.group(1)

            if port and ("localhost" in combined or "127.0.0.1" in combined):
                quick_fix = f"ss -tulpn | grep {port}"
                quick_fix_label = _("🔍 Verificar Porta :{port}").format(port=port)
                title = _("Conexão Recusada (:{port})").format(port=port)
                desc = _(
                    "Não foi possível conectar à porta :{port} no localhost (serviço inativo ou conexão recusada)."
                ).format(port=port)
            else:
                quick_fix = None
                quick_fix_label = _("🤖 Diagnosticar com IA")
                title = _("Falha de Conexão de Rede")
                desc = _(
                    "A conexão de rede falhou ou o servidor de destino não está aceitando conexões no momento."
                )

            return ErrorMatch(
                category=ErrorCategory.CONNECTION_REFUSED,
                title=title,
                description=desc,
                quick_fix_command=quick_fix,
                quick_fix_label=quick_fix_label,
                extracted_target=port,
                ai_prompt_hint=_(
                    "O comando falhou por recusa de conexão ou rede inalcançável (Connection refused). "
                    "Diagnostique se o serviço remoto está ativo, se há bloqueio de firewall ou erro de porta/DNS."
                ),
                exit_code=exit_code,
                command=cmd,
                output_snippet=out[:200],
            )
        return None

    def _check_git_error(
        self, cmd: str, out: str, exit_code: int, combined: str
    ) -> Optional[ErrorMatch]:
        if not cmd.startswith("git ") and "git" not in combined:
            return None

        git_patterns = [
            r"\bfatal:\s+not\s+a\s+git\s+repository\b",
            r"\bfatal:\s+'[^']+'\s+does\s+not\s+appear\s+to\s+be\s+a\s+git\s+repository\b",
            r"\bmerge\s+conflict\b",
            r"\bconflict\s+\(content\):\s+merge\s+conflict\b",
            r"\byour\s+local\s+changes\s+to\s+the\s+following\s+files\s+would\s+be\s+overwritten\s+by\s+merge\b",
            r"\berror:\s+failed\s+to\s+push\s+some\s+refs\b",
        ]
        if any(re.search(p, combined) for p in git_patterns):
            return ErrorMatch(
                category=ErrorCategory.GIT_ERROR,
                title=_("Erro de Operação Git"),
                description=_("A operação Git falhou devido ao estado do repositório, conflitos de merge ou ramificação remota."),
                quick_fix_command="git status",
                quick_fix_label=_("🌿 Ver git status"),
                ai_prompt_hint=_(
                    "A operação Git falhou. Analise a árvore de trabalho, conflitos de merge ou branches divergentes "
                    "e sugira passos seguros para sincronização sem perda de dados."
                ),
                exit_code=exit_code,
                command=cmd,
                output_snippet=out[:200],
            )
        return None

    def _build_generic_error(
        self, cmd: str, out: str, exit_code: int
    ) -> ErrorMatch:
        snippet = out[:200] if out else ""
        return ErrorMatch(
            category=ErrorCategory.GENERIC_ERROR,
            title=_("Comando Falhou ({code})").format(code=f"exit {exit_code}"),
            description=_("O comando falhou com código de saída {code}.").format(code=exit_code),
            quick_fix_command=None,
            quick_fix_label=_("🤖 Diagnosticar com IA"),
            ai_prompt_hint=_(
                "O comando '{cmd}' falhou com código de saída {code}. "
                "Analise as saídas do terminal, identifique a causa do erro e proponha a solução recomendada."
            ).format(cmd=cmd, code=exit_code),
            exit_code=exit_code,
            command=cmd,
            output_snippet=snippet,
        )


# Global singleton
_error_matcher_instance: Optional[TerminalErrorMatcher] = None


def get_terminal_error_matcher() -> TerminalErrorMatcher:
    """Returns the singleton instance of TerminalErrorMatcher."""
    global _error_matcher_instance
    if _error_matcher_instance is None:
        _error_matcher_instance = TerminalErrorMatcher()
    return _error_matcher_instance
