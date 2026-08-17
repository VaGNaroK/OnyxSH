#!/usr/bin/env python3
"""
Automated Rebranding Script: OnyxSH -> OnyxSH
Updates codebase, manifests, build scripts, desktop files, translations, and configs.
"""

import os
import re
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def replace_in_file(file_path: Path, replacements: list[tuple[str, str]]) -> bool:
    if not file_path.is_file():
        return False
    try:
        content = file_path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return False

    orig = content
    for old, new in replacements:
        content = content.replace(old, new)

    if content != orig:
        file_path.write_text(content, encoding="utf-8")
        return True
    return False


def main():
    print("🚀 Iniciando Rebranding para OnyxSH...")

    # 1. Replacements list (order matters: specific to generic)
    replacements = [
        ("io.github.vagnarok.OnyxSH", "io.github.vagnarok.OnyxSH"),
        ("io.github.vagnarok.OnyxSH", "io.github.vagnarok.OnyxSH"),
        ("OnyxSH", "OnyxSH"),
        ("onyxsh_admin_helper", "onyxsh_admin_helper"),
        ("onyxsh-admin-helper", "onyxsh-admin-helper"),
        ("onyxsh_session", "onyxsh_session"),
        ("onyxsh_handler_ids", "onyxsh_handler_ids"),
        ("onyxsh_pane_id", "onyxsh_pane_id"),
        ("onyxsh_last_exit_code", "onyxsh_last_exit_code"),
        ("onyxsh_bashrc", "onyxsh_bashrc"),
        ("onyxsh_backup_", "onyxsh_backup_"),
        ("onyxsh_restore_", "onyxsh_restore_"),
        ("__onyxsh_", "__onyxsh_"),
        ("_onyxsh_", "_onyxsh_"),
        ("ONYXSH_", "ONYXSH_"),
        ("ONYXSH", "ONYXSH"),
        ("OnyxSH", "OnyxSH"),
        ("onyxsh", "onyxsh"),
    ]

    # Process all text files in src/onyxsh, tests, scripts
    target_dirs = [ROOT / "src", ROOT / "tests", ROOT / "scripts", ROOT / "manifests"]
    for d in target_dirs:
        for p in d.rglob("*"):
            if p.is_file() and not p.name.endswith((".pyc", ".mo", ".png", ".jpg", ".flatpak")):
                if replace_in_file(p, replacements):
                    print(f"  ✓ Atualizado: {p.relative_to(ROOT)}")

    # Process root config files
    root_files = [
        ROOT / "pyproject.toml",
        ROOT / "install.sh",
        ROOT / "PKGBUILD",
        ROOT / "default.nix",
        ROOT / "flake.nix",
        ROOT / "README.md",
        ROOT / "README.en.md",
    ]
    for rf in root_files:
        if replace_in_file(rf, replacements):
            print(f"  ✓ Atualizado: {rf.relative_to(ROOT)}")

    # Process usr directory
    usr_dir = ROOT / "usr"
    if usr_dir.exists():
        for p in usr_dir.rglob("*"):
            if p.is_file() and not p.name.endswith((".pyc", ".mo", ".png", ".jpg")):
                replace_in_file(p, replacements)

    print("✨ Substituições textuais concluídas!")


if __name__ == "__main__":
    main()
