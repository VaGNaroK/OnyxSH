"""Filesystem tools for safe inspection, staging, diffing, and editing."""

from __future__ import annotations

import difflib
import json
import os
import re
import shutil
import stat
import time
from datetime import datetime
from pathlib import Path
from typing import Any, Optional

from .models import ToolResult
from .path_guard import PathGuard
from .redactor import redact_secrets


DEFAULT_MAX_READ_BYTES = 20000
STAGING_BASE_DIR = Path.home() / ".cache" / "zashterminal" / "ai-staging"
BACKUP_BASE_DIR = Path.home() / ".local" / "share" / "zashterminal" / "backups"


def _get_staging_path(target_path: Path, plan_id: str = "default") -> Path:
    """Generate a unique staging path for a target file within a plan."""
    sanitized_name = target_path.name or "file"
    staging_dir = STAGING_BASE_DIR / plan_id
    staging_dir.mkdir(parents=True, exist_ok=True)
    return staging_dir / f"{sanitized_name}.staged"


def create_file_backup(target_path: Path) -> Optional[Path]:
    """Create a timestamped backup of a file before overwriting."""
    if not target_path.exists() or not target_path.is_file():
        return None

    try:
        from datetime import timezone
        ts = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
        backup_dir = BACKUP_BASE_DIR / ts
        backup_dir.mkdir(parents=True, exist_ok=True)
        backup_file = backup_dir / target_path.name
        shutil.copy2(target_path, backup_file)
        return backup_file
    except Exception:
        return None


class FSTools:
    """Filesystem inspection and safe modification tools."""

    def __init__(self, path_guard: Optional[PathGuard] = None) -> None:
        self.path_guard = path_guard or PathGuard()

    async def list_directory(
        self,
        path: str = ".",
        recursive: bool = False,
        max_entries: int = 100,
    ) -> ToolResult:
        """List files and folders in directory."""
        p = Path(os.path.expanduser(path)).resolve()
        if not self.path_guard.can_read(p):
            return ToolResult(
                status="denied",
                stderr=f"Permissão negada pelo PathGuard para leitura de: {path}",
            )

        if not p.exists() or not p.is_dir():
            return ToolResult(
                status="error",
                stderr=f"O diretório não existe: {path}",
                exit_code=1,
            )

        entries: list[dict[str, Any]] = []
        try:
            if recursive:
                for root, dirs, files in os.walk(p):
                    root_path = Path(root)
                    if not self.path_guard.can_read(root_path):
                        continue
                    for d in dirs:
                        if len(entries) >= max_entries:
                            break
                        entries.append({
                            "name": str((root_path / d).relative_to(p)),
                            "type": "directory",
                        })
                    for f in files:
                        if len(entries) >= max_entries:
                            break
                        file_path = root_path / f
                        size = file_path.stat().st_size if file_path.exists() else 0
                        entries.append({
                            "name": str(file_path.relative_to(p)),
                            "type": "file",
                            "size_bytes": size,
                        })
                    if len(entries) >= max_entries:
                        break
            else:
                for item in sorted(p.iterdir(), key=lambda x: (not x.is_dir(), x.name)):
                    if len(entries) >= max_entries:
                        break
                    if not self.path_guard.can_read(item):
                        continue
                    is_dir = item.is_dir()
                    size = item.stat().st_size if not is_dir and item.exists() else None
                    entries.append({
                        "name": item.name,
                        "type": "directory" if is_dir else "file",
                        "size_bytes": size,
                    })

            return ToolResult(
                status="ok",
                stdout=json.dumps(entries, indent=2, ensure_ascii=False),
            )
        except Exception as e:
            return ToolResult(status="error", stderr=str(e), exit_code=1)

    async def metadata(self, path: str) -> ToolResult:
        """Inspect file metadata (size, permissions, timestamps)."""
        p = Path(os.path.expanduser(path)).resolve()
        if not self.path_guard.can_read(p):
            return ToolResult(
                status="denied",
                stderr=f"Permissão negada pelo PathGuard para: {path}",
            )

        if not p.exists():
            return ToolResult(
                status="error",
                stderr=f"Arquivo ou pasta não encontrado: {path}",
                exit_code=1,
            )

        try:
            st = p.stat()
            info = {
                "path": str(p),
                "is_dir": p.is_dir(),
                "is_file": p.is_file(),
                "is_symlink": p.is_symlink(),
                "size_bytes": st.st_size,
                "mode_octal": oct(stat.S_IMODE(st.st_mode)),
                "modified": datetime.fromtimestamp(st.st_mtime).isoformat(),
                "created": datetime.fromtimestamp(st.st_ctime).isoformat(),
            }
            return ToolResult(
                status="ok",
                stdout=json.dumps(info, indent=2, ensure_ascii=False),
            )
        except Exception as e:
            return ToolResult(status="error", stderr=str(e), exit_code=1)

    async def read_file(
        self,
        path: str,
        max_bytes: int = DEFAULT_MAX_READ_BYTES,
    ) -> ToolResult:
        """Read file contents with size limits and secret redaction."""
        p = Path(os.path.expanduser(path)).resolve()
        if not self.path_guard.can_read(p):
            return ToolResult(
                status="denied",
                stderr=f"Permissão negada pelo PathGuard para ler: {path}",
            )

        if not p.exists() or not p.is_file():
            return ToolResult(
                status="error",
                stderr=f"Arquivo não encontrado: {path}",
                exit_code=1,
            )

        try:
            with open(p, "rb") as f:
                data = f.read(max_bytes + 1)

            truncated = False
            if len(data) > max_bytes:
                data = data[:max_bytes]
                truncated = True

            text = data.decode("utf-8", errors="replace")
            redacted_text, count_redacted = redact_secrets(text)

            if truncated:
                redacted_text += f"\n\n[... Arquivo truncado no limite de {max_bytes} bytes ...]"

            return ToolResult(
                status="ok",
                stdout=redacted_text,
                truncated=truncated,
                secrets_redacted=count_redacted,
            )
        except Exception as e:
            return ToolResult(status="error", stderr=str(e), exit_code=1)

    async def search_text(
        self,
        path: str,
        pattern: str,
        max_results: int = 50,
    ) -> ToolResult:
        """Search text patterns in files under directory."""
        p = Path(os.path.expanduser(path)).resolve()
        if not self.path_guard.can_read(p):
            return ToolResult(
                status="denied",
                stderr=f"Permissão negada pelo PathGuard para busca em: {path}",
            )

        try:
            regex = re.compile(pattern, re.IGNORECASE)
        except re.error as e:
            return ToolResult(status="error", stderr=f"Regex inválido: {e}", exit_code=1)

        matches: list[dict[str, Any]] = []
        try:
            for root, _, files in os.walk(p):
                root_path = Path(root)
                if not self.path_guard.can_read(root_path):
                    continue
                for f in files:
                    if len(matches) >= max_results:
                        break
                    file_path = root_path / f
                    if not self.path_guard.can_read(file_path):
                        continue
                    try:
                        with open(file_path, "r", encoding="utf-8", errors="ignore") as handle:
                            for line_idx, line in enumerate(handle, start=1):
                                if regex.search(line):
                                    redacted_line, _ = redact_secrets(line.strip())
                                    matches.append({
                                        "file": str(file_path.relative_to(p)),
                                        "line": line_idx,
                                        "content": redacted_line,
                                    })
                                    if len(matches) >= max_results:
                                        break
                    except Exception:
                        continue
                if len(matches) >= max_results:
                    break

            return ToolResult(
                status="ok",
                stdout=json.dumps(matches, indent=2, ensure_ascii=False),
            )
        except Exception as e:
            return ToolResult(status="error", stderr=str(e), exit_code=1)

    async def disk_usage(self, path: str = "~") -> ToolResult:
        """Get disk usage for a specific path."""
        p = Path(os.path.expanduser(path)).resolve()
        if not self.path_guard.can_read(p):
            return ToolResult(
                status="denied",
                stderr=f"Permissão negada pelo PathGuard para: {path}",
            )

        try:
            total, used, free = shutil.disk_usage(p)
            gb = 1024 ** 3
            res = {
                "path": str(p),
                "total_gb": round(total / gb, 2),
                "used_gb": round(used / gb, 2),
                "free_gb": round(free / gb, 2),
                "percent_used": round((used / total) * 100, 1),
            }
            return ToolResult(
                status="ok",
                stdout=json.dumps(res, indent=2, ensure_ascii=False),
            )
        except Exception as e:
            return ToolResult(status="error", stderr=str(e), exit_code=1)

    async def write_staged_file(
        self,
        path: str,
        content: str,
        plan_id: str = "default",
    ) -> ToolResult:
        """Write content into a staging directory for human review and diff generation."""
        p = Path(os.path.expanduser(path)).resolve()
        if not self.path_guard.can_write(p):
            return ToolResult(
                status="denied",
                stderr=f"Permissão negada pelo PathGuard para escrita em: {path}",
            )

        try:
            staging_file = _get_staging_path(p, plan_id)
            with open(staging_file, "w", encoding="utf-8") as f:
                f.write(content)

            # Generate diff if original exists
            original_lines: list[str] = []
            if p.exists() and p.is_file():
                with open(p, "r", encoding="utf-8", errors="replace") as f:
                    original_lines = f.read().splitlines(keepends=True)

            new_lines = content.splitlines(keepends=True)
            diff = list(
                difflib.unified_diff(
                    original_lines,
                    new_lines,
                    fromfile=f"a/{p.name}",
                    tofile=f"b/{p.name}",
                )
            )
            diff_str = "".join(diff) if diff else "(Nenhuma diferença de conteúdo detectada)"

            return ToolResult(
                status="ok",
                stdout=f"Arquivo colocado em staging: {staging_file}\n\n{diff_str}",
            )
        except Exception as e:
            return ToolResult(status="error", stderr=str(e), exit_code=1)

    async def propose_edit(
        self,
        path: str,
        new_content: str,
        plan_id: str = "default",
    ) -> ToolResult:
        """Propose an edit, creating a staged copy and producing a diff for user approval."""
        return await self.write_staged_file(path, new_content, plan_id)

    async def create_directory(self, path: str) -> ToolResult:
        """Safely create a new directory."""
        p = Path(os.path.expanduser(path)).resolve()
        if not self.path_guard.can_write(p):
            return ToolResult(
                status="denied",
                stderr=f"Permissão negada pelo PathGuard para criar pasta em: {path}",
            )

        try:
            p.mkdir(parents=True, exist_ok=True)
            return ToolResult(
                status="ok",
                stdout=f"Diretório criado com sucesso: {p}",
            )
        except Exception as e:
            return ToolResult(status="error", stderr=str(e), exit_code=1)

    async def move_to_trash(self, path: str) -> ToolResult:
        """Safely move a file or folder to desktop Trash."""
        p = Path(os.path.expanduser(path)).resolve()
        if not self.path_guard.can_write(p):
            return ToolResult(
                status="denied",
                stderr=f"Permissão negada pelo PathGuard para mover para a lixeira: {path}",
            )

        if not p.exists():
            return ToolResult(
                status="error",
                stderr=f"Arquivo ou diretório não encontrado: {path}",
                exit_code=1,
            )

        try:
            trash_files_dir = Path.home() / ".local" / "share" / "Trash" / "files"
            trash_files_dir.mkdir(parents=True, exist_ok=True)
            dest = trash_files_dir / f"{p.name}_{int(time.time())}"
            shutil.move(str(p), str(dest))
            return ToolResult(
                status="ok",
                stdout=f"Item movido para a lixeira com sucesso: {dest}",
            )
        except Exception as e:
            return ToolResult(status="error", stderr=str(e), exit_code=1)

    async def apply_staged(
        self,
        target_path: str,
        staged_path: str,
        backup: bool = True,
    ) -> ToolResult:
        """Apply a staged file to its destination after user approval with optional backup."""
        target = Path(os.path.expanduser(target_path)).resolve()
        staged = Path(os.path.expanduser(staged_path)).resolve()

        if not self.path_guard.can_write(target):
            return ToolResult(
                status="denied",
                stderr=f"Permissão negada pelo PathGuard para aplicar alterações em: {target}",
            )

        if not staged.exists() or not staged.is_file():
            return ToolResult(
                status="error",
                stderr=f"Arquivo staged não encontrado: {staged}",
                exit_code=1,
            )

        try:
            backup_file = None
            if backup and target.exists():
                backup_file = create_file_backup(target)

            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(staged, target)

            msg = f"Arquivo {target} atualizado com sucesso a partir de {staged}."
            if backup_file:
                msg += f" Backup criado em {backup_file}."

            return ToolResult(status="ok", stdout=msg)
        except Exception as e:
            return ToolResult(status="error", stderr=str(e), exit_code=1)
