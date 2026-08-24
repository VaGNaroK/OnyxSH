# onyxsh/agent/verifier.py
"""Post-execution verification loop and sanity checks for OnyxSH Agent."""

from __future__ import annotations

import os
import re
import shlex
import subprocess
from dataclasses import dataclass, field
from typing import Any, Callable, Dict, List, Optional

from ..utils.logger import get_logger

logger = get_logger("onyxsh.agent.verifier")


@dataclass
class VerificationCheck:
    """Represents an inferred or explicit post-execution sanity check."""

    target_command: str
    check_command: str
    check_type: str  # service_active, syntax_test, path_exists, path_absent, package_installed, firewall_status, docker_status, generic
    description: str
    expected_exit_code: int = 0
    failure_diagnostic_command: Optional[str] = None
    expected_output_pattern: Optional[str] = None
    status: str = "pending"  # pending, running, success, failed, skipped

    def to_dict(self) -> Dict[str, Any]:
        return {
            "target_command": self.target_command,
            "check_command": self.check_command,
            "check_type": self.check_type,
            "description": self.description,
            "expected_exit_code": self.expected_exit_code,
            "failure_diagnostic_command": self.failure_diagnostic_command,
            "expected_output_pattern": self.expected_output_pattern,
            "status": self.status,
        }


@dataclass
class VerificationResult:
    """Outcome of running a verification check."""

    check: VerificationCheck
    success: bool
    exit_code: int
    output: str
    diagnostic_output: Optional[str] = None
    error_message: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "check": self.check.to_dict(),
            "success": self.success,
            "exit_code": self.exit_code,
            "output": self.output,
            "diagnostic_output": self.diagnostic_output,
            "error_message": self.error_message,
        }


def safe_quote_path(path: str) -> str:
    """Quotes a path safely for shell execution while preserving ~ or $HOME expansion."""
    p = path.strip("\"'")
    if p.startswith("~/"):
        subpath = p[2:]
        return f'"$HOME/{subpath}"' if '"' not in subpath else f'"$HOME"/{shlex.quote(subpath)}'
    elif p == "~":
        return '"$HOME"'
    elif p.startswith("$HOME/"):
        subpath = p[6:]
        return f'"$HOME/{subpath}"' if '"' not in subpath else f'"$HOME"/{shlex.quote(subpath)}'
    elif p == "$HOME":
        return '"$HOME"'
    else:
        return shlex.quote(p)


class PostVerifier:
    """Infers and executes sanity verification checks for mutating commands."""

    def __init__(self, is_flatpak: Optional[bool] = None) -> None:
        self.is_flatpak = (
            is_flatpak if is_flatpak is not None else os.path.exists("/.flatpak-info")
        )

    def infer_verifications(
        self, commands: List[Any]
    ) -> List[VerificationCheck]:
        """Analyzes executed commands and infers appropriate sanity checks.

        Args:
            commands: List of command strings or ActionStep dictionaries.

        Returns:
            List of VerificationCheck instances.
        """
        checks: List[VerificationCheck] = []
        seen_checks: set[str] = set()

        for item in commands:
            cmd_str = ""
            if isinstance(item, str):
                cmd_str = item.strip()
            elif isinstance(item, dict):
                cmd_str = item.get("command", "") or item.get("cmd", "") or item.get("command_str", "")
                if not cmd_str and "argv" in item:
                    argv = item["argv"]
                    if isinstance(argv, list):
                        cmd_str = " ".join(str(tok) for tok in argv)
                    elif isinstance(argv, str):
                        cmd_str = argv

            if not cmd_str:
                continue

            inferred = self._infer_single_command(cmd_str)
            for chk in inferred:
                key = f"{chk.check_type}:{chk.check_command}"
                if key not in seen_checks:
                    seen_checks.add(key)
                    checks.append(chk)

        return checks

    def _infer_single_command(self, cmd_str: str) -> List[VerificationCheck]:
        """Infers checks for a single command line."""
        checks: List[VerificationCheck] = []
        raw_cmd = cmd_str.strip()

        # Strip sudo / pkexec prefix for analysis
        clean_cmd = re.sub(r"^(sudo|pkexec|doas)\s+(-[a-zA-Z]+\s+)*", "", raw_cmd).strip()

        # 1. Systemd / Systemctl Service Actions
        systemctl_match = re.match(
            r"^systemctl\s+(start|restart|reload|try-restart|enable|disable|stop)\s+([a-zA-Z0-9@_\.\-]+)",
            clean_cmd,
        )
        if systemctl_match:
            action, service = systemctl_match.group(1), systemctl_match.group(2)
            if not service.endswith(".service") and not service.endswith(".socket") and not service.endswith(".timer"):
                full_service = f"{service}.service"
            else:
                full_service = service

            if action in {"start", "restart", "reload", "try-restart"}:
                checks.append(
                    VerificationCheck(
                        target_command=raw_cmd,
                        check_command=f"systemctl is-active {full_service}",
                        check_type="service_active",
                        description=f"Verificar se o serviço '{full_service}' está ativo",
                        expected_exit_code=0,
                        failure_diagnostic_command=f"journalctl -u {full_service} -n 25 --no-pager",
                    )
                )
            elif action == "enable":
                checks.append(
                    VerificationCheck(
                        target_command=raw_cmd,
                        check_command=f"systemctl is-enabled {full_service}",
                        check_type="service_enabled",
                        description=f"Verificar se o serviço '{full_service}' foi habilitado no boot",
                        expected_exit_code=0,
                        failure_diagnostic_command=f"systemctl status {full_service} --no-pager",
                    )
                )
            elif action in {"disable", "stop"}:
                checks.append(
                    VerificationCheck(
                        target_command=raw_cmd,
                        check_command=f"systemctl is-active {full_service}",
                        check_type="service_stopped",
                        description=f"Verificar se o serviço '{full_service}' foi desativado/parado",
                        expected_exit_code=3,  # Inactive status exit code in systemd
                        failure_diagnostic_command=f"systemctl status {full_service} --no-pager",
                    )
                )
            return checks

        # 2. SysV Service Actions (e.g. `service nginx restart`)
        service_match = re.match(
            r"^service\s+([a-zA-Z0-9@_\.\-]+)\s+(start|restart|reload|status)",
            clean_cmd,
        )
        if service_match:
            service, action = service_match.group(1), service_match.group(2)
            checks.append(
                VerificationCheck(
                    target_command=raw_cmd,
                    check_command=f"service {service} status",
                    check_type="service_active",
                    description=f"Verificar status do serviço '{service}'",
                    expected_exit_code=0,
                    failure_diagnostic_command=f"journalctl -u {service} -n 25 --no-pager",
                )
            )
            return checks

        # 3. Web Servers & Proxy Configurations (Nginx, Apache, Caddy, Lighttpd)
        if "nginx" in clean_cmd:
            if re.search(r"nginx\s+(-s\s+(reload|reopen)|-t)", clean_cmd) or "/etc/nginx" in clean_cmd:
                checks.append(
                    VerificationCheck(
                        target_command=raw_cmd,
                        check_command="nginx -t",
                        check_type="syntax_test",
                        description="Testar integridade da sintaxe de configuração do Nginx",
                        expected_exit_code=0,
                        failure_diagnostic_command="nginx -t",
                    )
                )
                return checks

        if "apache2" in clean_cmd or "httpd" in clean_cmd:
            if "apache2ctl" in clean_cmd or "httpd" in clean_cmd or "/etc/apache2" in clean_cmd or "/etc/httpd" in clean_cmd:
                check_cmd = "apache2ctl configtest" if "apache2" in clean_cmd else "httpd -t"
                checks.append(
                    VerificationCheck(
                        target_command=raw_cmd,
                        check_command=check_cmd,
                        check_type="syntax_test",
                        description="Testar integridade de sintaxe do servidor Apache/HTTPD",
                        expected_exit_code=0,
                        failure_diagnostic_command=check_cmd,
                    )
                )
                return checks

        # 4. SSH Daemon Configuration
        if "sshd" in clean_cmd or "/etc/ssh/sshd_config" in clean_cmd:
            checks.append(
                VerificationCheck(
                    target_command=raw_cmd,
                    check_command="sshd -t",
                    check_type="syntax_test",
                    description="Validar sintaxe do arquivo de configuração do SSH (/etc/ssh/sshd_config)",
                    expected_exit_code=0,
                    failure_diagnostic_command="sshd -t",
                )
            )
            return checks

        # 5. Firewall Rules (UFW, iptables, nftables)
        if clean_cmd.startswith("ufw "):
            checks.append(
                VerificationCheck(
                    target_command=raw_cmd,
                    check_command="ufw status verbose",
                    check_type="firewall_status",
                    description="Verificar status e regras ativas do firewall UFW",
                    expected_exit_code=0,
                    failure_diagnostic_command="ufw status verbose",
                )
            )
            return checks

        if clean_cmd.startswith("iptables ") or clean_cmd.startswith("ip6tables "):
            checks.append(
                VerificationCheck(
                    target_command=raw_cmd,
                    check_command="iptables -L -n -v",
                    check_type="firewall_status",
                    description="Verificar tabela de regras ativas do iptables",
                    expected_exit_code=0,
                )
            )
            return checks

        # 6. File Permissions and Ownership (chmod, chown, chgrp)
        perm_match = re.match(
            r"^(chmod|chown|chgrp)\s+([^\s]+(?:\s+[^\s]+)*?)\s+([^\s]+)$",
            clean_cmd,
        )
        if perm_match:
            op, _, target_path = perm_match.group(1), perm_match.group(2), perm_match.group(3)
            target_path = target_path.strip("\"'")
            checks.append(
                VerificationCheck(
                    target_command=raw_cmd,
                    check_command=f"ls -ld {safe_quote_path(target_path)}",
                    check_type="path_permissions",
                    description=f"Verificar novas permissões/proprietário de '{target_path}'",
                    expected_exit_code=0,
                    failure_diagnostic_command=f"ls -ld {safe_quote_path(target_path)}",
                )
            )
            return checks

        # 7. Directory & File Creation / Deletion (mkdir, touch, rm, cp, mv)
        mkdir_match = re.match(r"^mkdir\s+(-[a-zA-Z]+\s+)*(.+)$", clean_cmd)
        if mkdir_match:
            dirs = mkdir_match.group(2).split()
            for d in dirs:
                d_clean = d.strip("\"'")
                checks.append(
                    VerificationCheck(
                        target_command=raw_cmd,
                        check_command=f"test -d {safe_quote_path(d_clean)}",
                        check_type="path_exists",
                        description=f"Verificar criação do diretório '{d_clean}'",
                        expected_exit_code=0,
                        failure_diagnostic_command=f"ls -ld {safe_quote_path(d_clean)}",
                    )
                )
            return checks

        touch_match = re.match(r"^touch\s+(.+)$", clean_cmd)
        if touch_match:
            files = touch_match.group(1).split()
            for f in files:
                f_clean = f.strip("\"'")
                checks.append(
                    VerificationCheck(
                        target_command=raw_cmd,
                        check_command=f"test -e {safe_quote_path(f_clean)}",
                        check_type="path_exists",
                        description=f"Verificar existência do arquivo '{f_clean}'",
                        expected_exit_code=0,
                        failure_diagnostic_command=f"ls -la {safe_quote_path(f_clean)}",
                    )
                )
            return checks

        rm_match = re.match(r"^rm\s+(-[a-zA-Z]+\s+)*(.+)$", clean_cmd)
        if rm_match:
            targets = rm_match.group(2).split()
            for t in targets:
                t_clean = t.strip("\"'")
                if not t_clean.startswith("-"):
                    checks.append(
                        VerificationCheck(
                            target_command=raw_cmd,
                            check_command=f"test ! -e {safe_quote_path(t_clean)}",
                            check_type="path_absent",
                            description=f"Verificar remoção completa de '{t_clean}'",
                            expected_exit_code=0,
                            failure_diagnostic_command=f"ls -la {safe_quote_path(t_clean)}",
                        )
                    )
            return checks

        # 8. Package Managers (apt, apt-get, dpkg, dnf, pacman, pip, npm)
        pkg_install_match = re.match(
            r"^(apt|apt-get|dnf|pacman|yum)\s+(install|-S)\s+(-[a-zA-Z0-9-]+\s+)*(.+)$",
            clean_cmd,
        )
        if pkg_install_match:
            mgr, _, _, pkgs = pkg_install_match.group(1), pkg_install_match.group(2), pkg_install_match.group(3), pkg_install_match.group(4)
            valid_pkg_name_re = re.compile(r"^[a-zA-Z0-9][a-zA-Z0-9.+_-]*$")
            for pkg in pkgs.split():
                pkg_clean = pkg.strip("\"'()$;`")
                if not pkg_clean.startswith("-") and valid_pkg_name_re.match(pkg_clean):
                    if mgr in {"apt", "apt-get"}:
                        chk_cmd = f"dpkg -s {shlex.quote(pkg_clean)}"
                    elif mgr in {"dnf", "yum"}:
                        chk_cmd = f"rpm -q {shlex.quote(pkg_clean)}"
                    else:
                        chk_cmd = f"pacman -Q {shlex.quote(pkg_clean)}"
                    checks.append(
                        VerificationCheck(
                            target_command=raw_cmd,
                            check_command=chk_cmd,
                            check_type="package_installed",
                            description=f"Verificar se o pacote '{pkg_clean}' foi instalado com sucesso",
                            expected_exit_code=0,
                            failure_diagnostic_command=chk_cmd,
                        )
                    )
            return checks

        pkg_upgrade_match = re.match(r"^(apt|apt-get|dnf|pacman|yum)\s+(upgrade|dist-upgrade|-Syu)\b", clean_cmd)
        if pkg_upgrade_match:
            mgr = pkg_upgrade_match.group(1)
            if mgr in {"apt", "apt-get"}:
                chk_cmd = "apt list --upgradable"
            elif mgr in {"dnf", "yum"}:
                chk_cmd = "dnf check-update"
            else:
                chk_cmd = "pacman -Qu"
            checks.append(
                VerificationCheck(
                    target_command=raw_cmd,
                    check_command=chk_cmd,
                    check_type="package_installed",
                    description="Verificar integridade e pacotes pendentes após atualização do sistema",
                    expected_exit_code=0,
                )
            )
            return checks

        pip_match = re.match(r"^(?:python[0-9.]*\s+-m\s+)?pip\s+install\s+(?:-[a-zA-Z0-9-]+\s+)*(.+)$", clean_cmd)
        if pip_match:
            pkgs = pip_match.group(1).split()
            for pkg in pkgs:
                pkg_clean = pkg.strip("\"'")
                if not pkg_clean.startswith("-"):
                    clean_name = re.split(r"[=<>!~]", pkg_clean)[0]
                    checks.append(
                        VerificationCheck(
                            target_command=raw_cmd,
                            check_command=f"pip show {shlex.quote(clean_name)}",
                            check_type="package_installed",
                            description=f"Verificar se o pacote Python '{clean_name}' está disponível",
                            expected_exit_code=0,
                            failure_diagnostic_command=f"pip show {shlex.quote(clean_name)}",
                        )
                    )
            return checks

        # 9. Docker & Podman Container Actions
        docker_match = re.match(
            r"^(docker|podman)\s+(run|start|restart)\s+(?:-[a-zA-Z0-9\-]+\s+)*(?:--name\s+([^\s]+)\s+)?([^\s]+)",
            clean_cmd,
        )
        if docker_match:
            cli, action, name_flag, image_or_name = (
                docker_match.group(1),
                docker_match.group(2),
                docker_match.group(3),
                docker_match.group(4),
            )
            container_name = name_flag if name_flag else image_or_name
            container_name = container_name.strip("\"'")
            checks.append(
                VerificationCheck(
                    target_command=raw_cmd,
                    check_command=f"{cli} ps -f name={shlex.quote(container_name)}",
                    check_type="docker_status",
                    description=f"Verificar se o contêiner '{container_name}' está em execução",
                    expected_exit_code=0,
                    failure_diagnostic_command=f"{cli} logs --tail 25 {shlex.quote(container_name)}",
                )
            )
            return checks

        if clean_cmd.startswith("docker compose up") or clean_cmd.startswith("docker-compose up"):
            checks.append(
                VerificationCheck(
                    target_command=raw_cmd,
                    check_command="docker compose ps",
                    check_type="docker_status",
                    description="Verificar status dos serviços do Docker Compose",
                    expected_exit_code=0,
                    failure_diagnostic_command="docker compose logs --tail 25",
                )
            )
            return checks

        # 10. Cron Jobs
        if "crontab" in clean_cmd:
            checks.append(
                VerificationCheck(
                    target_command=raw_cmd,
                    check_command="crontab -l",
                    check_type="cron_status",
                    description="Verificar agendamentos ativos no crontab",
                    expected_exit_code=0,
                    failure_diagnostic_command="crontab -l",
                )
            )
            return checks

        return checks

    def run_verification(
        self,
        check: VerificationCheck,
        custom_runner: Optional[Callable[[str], tuple[int, str, str]]] = None,
    ) -> VerificationResult:
        """Executes a verification check and captures diagnostics on failure.

        Args:
            check: The VerificationCheck to run.
            custom_runner: Optional custom execution function (cmd -> (exit_code, stdout, stderr)).

        Returns:
            VerificationResult with outcome and diagnostic logs.
        """
        check.status = "running"

        def _exec_command(cmd: str) -> tuple[int, str, str]:
            if custom_runner:
                return custom_runner(cmd)

            if self.is_flatpak:
                full_cmd = ["flatpak-spawn", "--host", "bash", "-c", cmd]
            else:
                full_cmd = ["bash", "-c", cmd]

            try:
                proc = subprocess.run(
                    full_cmd,
                    capture_output=True,
                    text=True,
                    timeout=15,
                )
                return proc.returncode, proc.stdout.strip(), proc.stderr.strip()
            except subprocess.TimeoutExpired:
                return -1, "", "Timeout de execução excedido (15s)"
            except Exception as e:
                return -1, "", str(e)

        exit_code, stdout, stderr = _exec_command(check.check_command)
        combined_output = f"{stdout}\n{stderr}".strip() if stderr else stdout

        success = (exit_code == check.expected_exit_code)
        if check.expected_output_pattern and success:
            if not re.search(check.expected_output_pattern, combined_output):
                success = False

        diagnostic_output = None
        error_msg = None

        if success:
            check.status = "success"
        else:
            check.status = "failed"
            error_msg = (
                f"Código de saída inesperado ({exit_code}, esperado {check.expected_exit_code})"
                if exit_code != check.expected_exit_code
                else "Padrão de saída esperado não encontrado"
            )

            # Run failure diagnostic command if configured
            if check.failure_diagnostic_command:
                diag_code, diag_out, diag_err = _exec_command(check.failure_diagnostic_command)
                diag_combined = f"{diag_out}\n{diag_err}".strip() if diag_err else diag_out
                if diag_combined:
                    diagnostic_output = diag_combined

        return VerificationResult(
            check=check,
            success=success,
            exit_code=exit_code,
            output=combined_output,
            diagnostic_output=diagnostic_output,
            error_message=error_msg,
        )
