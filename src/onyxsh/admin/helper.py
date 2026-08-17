#!/usr/bin/env python3
"""
OnyxSH Admin Helper (Polkit Backend).

Executes ONLY pre-approved administrative actions defined in admin_actions.json.
Validates all parameters strictly using regular expressions.
Never accepts arbitrary commands or shell strings.
"""

from __future__ import annotations

import argparse
import json
import logging
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

# Configure system logging
logging.basicConfig(
    level=logging.INFO,
    format="[%(asctime)s] [onyxsh-admin-helper] [%(levelname)s] %(message)s",
    handlers=[
        logging.StreamHandler(sys.stderr),
    ],
)
logger = logging.getLogger("onyxsh-admin-helper")


def _find_admin_actions_file() -> Path:
    """Locate the admin_actions.json policy definition file."""
    candidates = [
        Path(__file__).parent.parent / "data" / "policies" / "admin_actions.json",
        Path("/usr/share/onyxsh/data/policies/admin_actions.json"),
        Path("/usr/local/share/onyxsh/data/policies/admin_actions.json"),
        Path("/etc/onyxsh/policies/admin_actions.json"),
    ]
    for c in candidates:
        if c.exists():
            return c
    raise FileNotFoundError("admin_actions.json não encontrado nas localizações padrão.")


def load_admin_actions() -> dict[str, Any]:
    """Load and parse the admin actions policy file."""
    path = _find_admin_actions_file()
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def _detect_package_manager() -> str:
    """Detect system package manager executable."""
    for cmd in ["pacman", "apt-get", "apt", "dnf", "zypper"]:
        if subprocess.run(["which", cmd], capture_output=True).returncode == 0:
            return cmd
    return "apt"


def validate_and_build_command(
    action_id: str,
    raw_params: dict[str, Any],
    actions_data: dict[str, Any],
) -> tuple[list[str], str]:
    """
    Validate action_id and parameters against schema and regex patterns.

    Returns:
        (argv, description)
    """
    actions = actions_data.get("actions", actions_data)
    if action_id not in actions:
        raise PermissionError(f"Ação administrativa '{action_id}' não autorizada.")

    action_def = actions[action_id]
    param_specs = action_def.get("params", action_def.get("parameters", {}))

    # Check for forbidden unexpected parameters
    for p_name in raw_params:
        if p_name not in param_specs:
            raise ValueError(f"Parâmetro inesperado '{p_name}' para a ação '{action_id}'.")

    # Validate each parameter against regex
    validated_params: dict[str, str] = {}
    for p_name, p_pattern in param_specs.items():
        if isinstance(p_pattern, dict):
            pattern_str = p_pattern.get("pattern", "")
            default_val = p_pattern.get("default")
        else:
            pattern_str = str(p_pattern)
            default_val = None

        val = raw_params.get(p_name, default_val)
        if val is None:
            raise ValueError(f"Parâmetro obrigatório ausente: '{p_name}'")

        val_str = str(val)
        if pattern_str:
            regex = re.compile(pattern_str)
            if not regex.fullmatch(val_str):
                raise ValueError(
                    f"Valor inválido para o parâmetro '{p_name}': '{val_str}' "
                    f"(deve coincidir com {pattern_str})"
                )
        validated_params[p_name] = val_str

    # Inject default system pkg manager if {pkg} placeholder exists
    if "pkg" not in validated_params:
        validated_params["pkg"] = _detect_package_manager()

    # Build argv template
    argv_template = action_def.get("argv", action_def.get("argv_template", []))
    built_argv: list[str] = []
    for token in argv_template:
        formatted_token = token
        for k, v in validated_params.items():
            formatted_token = formatted_token.replace(f"{{{k}}}", v)
        built_argv.append(formatted_token)

    return built_argv, action_def.get("description", action_id)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="OnyxSH Admin Helper — Executa ações administrativas autorizadas via Polkit."
    )
    parser.add_argument("--action", required=True, help="Identificador da ação autorizada")
    parser.add_argument("--args", default="{}", help="Argumentos da ação em formato JSON")
    parser.add_argument("--dry-run", action="store_true", help="Simular execução sem aplicar alterações")

    args = parser.parse_args()

    try:
        raw_params = json.loads(args.args)
        if not isinstance(raw_params, dict):
            raise ValueError("--args deve ser um objeto JSON.")
    except Exception as e:
        logger.error("JSON de argumentos inválido: %s", e)
        print(json.dumps({"success": False, "error": f"JSON inválido: {e}"}))
        return 1

    try:
        actions_data = load_admin_actions()
        argv, description = validate_and_build_command(args.action, raw_params, actions_data)
    except Exception as e:
        logger.error("Validação de segurança falhou para ação '%s': %s", args.action, e)
        print(json.dumps({"success": False, "error": str(e)}))
        return 2

    logger.info(
        "Ação aprovada: '%s' | Descrição: '%s' | Comando: %s | Dry-run: %s",
        args.action,
        description,
        argv,
        args.dry_run,
    )

    if args.dry_run:
        result = {
            "success": True,
            "dry_run": True,
            "action": args.action,
            "argv": argv,
            "stdout": f"[DRY-RUN] Comando que seria executado: {' '.join(argv)}",
            "stderr": "",
            "returncode": 0,
        }
        print(json.dumps(result))
        return 0

    try:
        proc = subprocess.run(
            argv,
            shell=False,
            capture_output=True,
            text=True,
            timeout=120,
            check=False,
        )
        result = {
            "success": proc.returncode == 0,
            "action": args.action,
            "argv": argv,
            "stdout": proc.stdout[:10000],
            "stderr": proc.stderr[:10000],
            "returncode": proc.returncode,
        }
        print(json.dumps(result))
        return 0 if proc.returncode == 0 else proc.returncode
    except subprocess.TimeoutExpired:
        logger.error("Timeout ao executar ação '%s'", args.action)
        print(json.dumps({"success": False, "error": "Tempo limite de execução excedido (120s)."}))
        return 124
    except Exception as e:
        logger.error("Falha na execução do subprocesso: %s", e)
        print(json.dumps({"success": False, "error": str(e)}))
        return 3


if __name__ == "__main__":
    sys.exit(main())
