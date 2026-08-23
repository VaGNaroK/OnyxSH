# onyxsh/utils/checksum_utils.py
"""
Utilities for calculating, verifying, and formatting cryptographic file hashes.
Supports SHA-256, SHA-512, MD5, and SHA-1 in single-pass chunked streaming.
"""

import hashlib
import os
import re
import threading
from datetime import datetime
from pathlib import Path
from typing import Callable, Dict, Optional, Tuple

SUPPORTED_ALGORITHMS = ("sha256", "sha512", "md5", "sha1")

# Regex for detecting hash hex strings
_HEX_RE = re.compile(r"^[0-9a-fA-F]+$")

HASH_LENGTHS: Dict[int, str] = {
    32: "md5",
    40: "sha1",
    64: "sha256",
    128: "sha512",
}


def calculate_file_hashes(
    file_path: str,
    algorithms: Tuple[str, ...] = SUPPORTED_ALGORITHMS,
    chunk_size: int = 65536,
    progress_callback: Optional[Callable[[int, int, float], None]] = None,
    cancel_event: Optional[threading.Event] = None,
) -> Dict[str, str]:
    """Calculates cryptographic hashes of a file in a single stream pass.

    Args:
        file_path: Path to the target file.
        algorithms: Tuple of algorithm names (e.g. 'sha256', 'sha512', 'md5', 'sha1').
        chunk_size: Byte size to read per chunk (default 64 KB).
        progress_callback: Optional callback(bytes_read, total_bytes, percentage).
        cancel_event: Optional threading.Event to abort calculation early.

    Returns:
        Dictionary mapping algorithm name to hex digest string.

    Raises:
        FileNotFoundError: If file_path does not exist.
        ValueError: If unsupported algorithm requested.
        InterruptedError: If calculation was cancelled via cancel_event.
    """
    p = Path(file_path)
    if not p.is_file():
        raise FileNotFoundError(f"File not found: {file_path}")

    total_size = p.stat().st_size
    bytes_read = 0

    hashers: Dict[str, hashlib._Hash] = {}
    for algo in algorithms:
        normalized = algo.lower().replace("-", "")
        if normalized == "sha256":
            hashers[algo] = hashlib.sha256()
        elif normalized == "sha512":
            hashers[algo] = hashlib.sha512()
        elif normalized == "md5":
            hashers[algo] = hashlib.md5()
        elif normalized == "sha1":
            hashers[algo] = hashlib.sha1()
        else:
            raise ValueError(f"Unsupported algorithm: {algo}")

    with open(p, "rb") as f:
        while True:
            if cancel_event and cancel_event.is_set():
                raise InterruptedError("Checksum calculation cancelled.")

            chunk = f.read(chunk_size)
            if not chunk:
                break

            for hasher in hashers.values():
                hasher.update(chunk)

            bytes_read += len(chunk)
            if progress_callback and total_size > 0:
                pct = min(1.0, bytes_read / total_size)
                progress_callback(bytes_read, total_size, pct)

    if progress_callback and total_size == 0:
        progress_callback(0, 0, 1.0)

    return {algo: hasher.hexdigest().lower() for algo, hasher in hashers.items()}


def detect_hash_type(hash_string: str) -> Optional[str]:
    """Detects likely hash algorithm based on hex length.

    Args:
        hash_string: Candidate hash string (spaces and quotes stripped).

    Returns:
        Algorithm name ('md5', 'sha1', 'sha256', 'sha512') or None.
    """
    cleaned = hash_string.strip().strip('"').strip("'")
    if not cleaned or not _HEX_RE.match(cleaned):
        return None

    return HASH_LENGTHS.get(len(cleaned))


def compare_hash(
    computed_hashes: Dict[str, str],
    input_hash: str,
) -> Tuple[bool, Optional[str]]:
    """Compares an input hash against calculated dictionary.

    Args:
        computed_hashes: Dict of calculated {algo: hex_digest}.
        input_hash: String pasted or provided by the user.

    Returns:
        (is_match, matched_algorithm_name)
    """
    cleaned = input_hash.strip().strip('"').strip("'").lower()
    if not cleaned or not _HEX_RE.match(cleaned):
        return False, None

    for algo, val in computed_hashes.items():
        if val.lower() == cleaned:
            return True, algo

    return False, None


def format_checksum_report(
    file_name: str,
    file_path: str,
    file_size_str: str,
    hashes: Dict[str, str],
) -> str:
    """Formats calculated hashes into a clean, markdown-formatted report.

    Args:
        file_name: File basename.
        file_path: Full absolute file path.
        file_size_str: Human-readable file size string.
        hashes: Dict of {algo: hex_digest}.

    Returns:
        Formatted markdown string.
    """
    now_str = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    lines = [
        f"### 🔐 OnyxSH Checksum Report",
        f"- **File:** `{file_name}`",
        f"- **Path:** `{file_path}`",
        f"- **Size:** {file_size_str}",
        f"- **Generated:** {now_str}",
        f"",
        f"| Algorithm | Hash Value |",
        f"| :--- | :--- |",
    ]

    for algo in ("sha256", "sha512", "md5", "sha1"):
        if algo in hashes:
            lines.append(f"| **{algo.upper()}** | `{hashes[algo]}` |")

    return "\n".join(lines)
