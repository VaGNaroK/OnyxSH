# onyxsh/utils/backup.py

import json
import shutil
import tempfile
import threading
from pathlib import Path
from typing import List, Optional

from gi.repository import Gio

from .exceptions import StorageReadError, StorageWriteError
from .logger import get_logger
from .platform import get_config_directory


def _get_py7zr():
    """Lazy import for py7zr module. Only called when backup/restore is used."""
    try:
        import py7zr

        return py7zr
    except ImportError:
        return None


class BackupManager:
    """Manages encrypted backup and recovery operations."""

    def __init__(self, backup_dir: Optional[Path] = None):
        self.logger = get_logger("onyxsh.backup")
        if backup_dir is None:
            backup_dir = get_config_directory() / "backups"
        self.backup_dir = Path(backup_dir)
        self.backup_dir.mkdir(parents=True, exist_ok=True)
        self._lock = threading.Lock()
        self.logger.info(
            f"Backup manager initialized with directory: {self.backup_dir}"
        )

    def create_encrypted_backup(
        self,
        target_file_path: str,
        password: str,
        sessions_store: Gio.ListStore,
        source_files: List[Path],
        layouts_dir: Path,
    ) -> None:
        """
        Creates a single, password-protected .7z backup file.

        Args:
            target_file_path: The full path where the backup file will be saved.
            password: The password for encrypting the backup.
            sessions_store: The session store to export passwords from.
            source_files: List of primary config files to include (e.g., sessions.json).
            layouts_dir: The directory containing layout files to be backed up.

        Raises:
            StorageWriteError: If the backup process fails.
        """
        py7zr = _get_py7zr()
        if not py7zr:
            raise StorageWriteError(
                target_file_path,
                "The 'py7zr' library is required for encrypted backups. Please install it.",
            )

        with self._lock:
            with tempfile.TemporaryDirectory(prefix="onyxsh_backup_") as tmpdir:
                temp_path = Path(tmpdir)
                self.logger.debug(f"Using temporary directory for backup: {temp_path}")

                try:
                    # 1. Copy primary source files
                    for src_file in source_files:
                        if src_file.exists():
                            shutil.copy(src_file, temp_path / src_file.name)

                    # 2. Copy layouts directory
                    if layouts_dir.exists() and layouts_dir.is_dir():
                        shutil.copytree(
                            layouts_dir, temp_path / "layouts", dirs_exist_ok=True
                        )

                    # 3. Export and save passwords (lazy import crypto)
                    from .crypto import export_all_passwords
                    passwords = export_all_passwords(sessions_store)
                    if passwords:
                        with open(temp_path / "passwords.json", "w") as f:
                            json.dump(passwords, f, indent=2)

                    # 4. Create the encrypted 7z archive
                    self.logger.info(f"Creating encrypted backup at {target_file_path}")
                    with py7zr.SevenZipFile(
                        target_file_path, "w", password=password
                    ) as archive:
                        archive.writeall(temp_path, arcname="")

                    self.logger.info("Encrypted backup created successfully.")

                except Exception as e:
                    self.logger.error(f"Failed to create encrypted backup: {e}")
                    raise StorageWriteError(target_file_path, str(e)) from e

    def restore_from_encrypted_backup(
        self, source_file_path: str, password: str, config_dir: Path
    ) -> None:
        """
        Restores configuration from a password-protected .7z backup file.

        Args:
            source_file_path: The path to the .7z backup file.
            password: The password to decrypt the backup.
            config_dir: The root configuration directory to restore files to.

        Raises:
            StorageReadError: If the restore process fails.
        """
        py7zr = _get_py7zr()
        if not py7zr:
            raise StorageReadError(
                source_file_path,
                "The 'py7zr' library is required for encrypted backups. Please install it.",
            )

        with self._lock:
            with tempfile.TemporaryDirectory(prefix="onyxsh_restore_") as tmpdir:
                temp_path = Path(tmpdir)
                self.logger.debug(f"Using temporary directory for restore: {temp_path}")

                try:
                    # 1. Extract the archive
                    self.logger.info(f"Extracting backup from {source_file_path}")
                    with py7zr.SevenZipFile(
                        source_file_path, "r", password=password
                    ) as archive:
                        archive.extractall(path=temp_path)

                    # 2. Restore files
                    for item in temp_path.iterdir():
                        target_path = config_dir / item.name
                        if item.is_dir():
                            shutil.rmtree(target_path, ignore_errors=True)
                            shutil.copytree(item, target_path, dirs_exist_ok=True)
                        elif item.is_file() and item.name != "passwords.json":
                            shutil.copy(item, target_path)

                    # 3. Import passwords
                    passwords_file = temp_path / "passwords.json"
                    if passwords_file.exists():
                        with open(passwords_file, "r") as f:
                            passwords = json.load(f)

                        from .crypto import store_password
                        imported_count = 0
                        for session_name, pwd in passwords.items():
                            try:
                                store_password(session_name, pwd)
                                imported_count += 1
                            except Exception as e:
                                self.logger.error(
                                    f"Failed to import password for '{session_name}': {e}"
                                )
                        self.logger.info(f"Imported {imported_count} passwords.")

                    self.logger.info(
                        "Restore from encrypted backup completed successfully."
                    )

                except py7zr.exceptions.PasswordRequired:
                    self.logger.error("Password required for backup file.")
                    raise StorageReadError(source_file_path, "Password required.")
                except py7zr.exceptions.Bad7zFile:
                    self.logger.error("Bad 7z file or incorrect password.")
                    raise StorageReadError(
                        source_file_path, "Incorrect password or corrupted file."
                    )
                except Exception as e:
                    self.logger.error(f"Failed to restore from encrypted backup: {e}")
                    raise StorageReadError(source_file_path, str(e)) from e


_backup_manager: Optional[BackupManager] = None
_backup_manager_lock = threading.Lock()


def get_backup_manager() -> BackupManager:
    """Get the global backup manager instance."""
    global _backup_manager
    if _backup_manager is None:
        with _backup_manager_lock:
            if _backup_manager is None:
                _backup_manager = BackupManager()
    return _backup_manager


# -------------------------------------------------------------------
# Agent File Backup & Rollback Helpers
# -------------------------------------------------------------------

def _compute_sha256(path: Path) -> str:
    """Compute sha256 checksum of a file."""
    import hashlib
    if not path.exists() or not path.is_file():
        return ""
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def create_file_backup(
    target_file: str | Path,
    staged_file: Optional[str | Path] = None,
    plan_id: str = "",
    step_id: str = "",
) -> dict:
    """
    Create a timestamped backup and manifest before applying a staged edit.

    Returns:
        Manifest dictionary with backup details.
    """
    import uuid
    from datetime import datetime, timezone

    target = Path(target_file).resolve()
    base_backup_dir = Path.home() / ".local" / "share" / "onyxsh" / "backups"
    ts = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
    backup_id = f"backup_{ts}_{uuid.uuid4().hex[:8]}"
    backup_folder = base_backup_dir / backup_id
    backup_folder.mkdir(parents=True, exist_ok=True)

    backup_copy = backup_folder / "original"
    sha_orig = ""
    if target.exists() and target.is_file():
        shutil.copy2(target, backup_copy)
        sha_orig = _compute_sha256(backup_copy)

    sha_new = ""
    if staged_file:
        staged_p = Path(staged_file).resolve()
        if staged_p.exists():
            sha_new = _compute_sha256(staged_p)

    manifest = {
        "backup_id": backup_id,
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "target_path": str(target),
        "backup_path": str(backup_copy),
        "sha256_original": sha_orig,
        "sha256_new": sha_new,
        "plan_id": plan_id,
        "step_id": step_id,
    }

    manifest_file = backup_folder / "manifest.json"
    with open(manifest_file, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2)

    return manifest


def rollback_file_backup(backup_id: str) -> bool:
    """
    Restore a file from its backup manifest.

    Returns:
        True if successfully restored and verified byte-identical.
    """
    base_backup_dir = Path.home() / ".local" / "share" / "onyxsh" / "backups"
    backup_folder = base_backup_dir / backup_id
    manifest_file = backup_folder / "manifest.json"

    if not manifest_file.exists():
        raise FileNotFoundError(f"Manifesto não encontrado para backup '{backup_id}'.")

    with open(manifest_file, "r", encoding="utf-8") as f:
        manifest = json.load(f)

    target_path = Path(manifest["target_path"])
    backup_path = Path(manifest["backup_path"])
    expected_sha = manifest.get("sha256_original", "")

    if not backup_path.exists():
        raise FileNotFoundError(f"Arquivo de backup original não existe: {backup_path}")

    # Verify backup copy hash
    actual_backup_sha = _compute_sha256(backup_path)
    if expected_sha and actual_backup_sha != expected_sha:
        raise ValueError(
            f"Integridade do arquivo de backup corrompida (esperado {expected_sha}, obtido {actual_backup_sha})"
        )

    # Ensure parent exists
    target_path.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(backup_path, target_path)

    # Verify restored target
    restored_sha = _compute_sha256(target_path)
    if expected_sha and restored_sha != expected_sha:
        raise ValueError("Falha ao verificar arquivo restaurado pós-rollback.")

    return True


def list_file_backups() -> list[dict]:
    """List all available file backup manifests ordered newest first."""
    base_backup_dir = Path.home() / ".local" / "share" / "onyxsh" / "backups"
    if not base_backup_dir.exists():
        return []

    manifests = []
    for folder in base_backup_dir.iterdir():
        if folder.is_dir():
            mf = folder / "manifest.json"
            if mf.exists():
                try:
                    with open(mf, "r", encoding="utf-8") as f:
                        manifests.append(json.load(f))
                except Exception:
                    continue

    return sorted(manifests, key=lambda m: m.get("timestamp", ""), reverse=True)

