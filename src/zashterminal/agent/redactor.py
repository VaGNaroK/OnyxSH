"""Secret redactor to prevent leaking sensitive tokens, keys, and passwords."""

from __future__ import annotations

import re


SECRET_PATTERNS: list[tuple[re.Pattern, str]] = [
    # AWS Access Key IDs
    (re.compile(r"\b(AKIA[0-9A-Z]{16})\b"), "[REDACTED_AWS_KEY]"),
    # AWS Secret Access Keys / generic secret assignment
    (
        re.compile(
            r"(?i)\b(aws_secret_access_key|aws_session_token)\s*[:=]\s*['\"]?([A-Za-z0-9/+=]{40})['\"]?"
        ),
        r"\1=[REDACTED_AWS_SECRET]",
    ),
    # Private Key blocks (RSA, EC, OPENSSH, PGP, etc.)
    (
        re.compile(
            r"-----BEGIN [A-Z0-9_\-\s]+PRIVATE KEY-----[\s\S]*?-----END [A-Z0-9_\-\s]+PRIVATE KEY-----"
        ),
        "[REDACTED_PRIVATE_KEY]",
    ),
    # PGP Private Key blocks
    (
        re.compile(
            r"-----BEGIN PGP PRIVATE KEY BLOCK-----[\s\S]*?-----END PGP PRIVATE KEY BLOCK-----"
        ),
        "[REDACTED_PGP_PRIVATE_KEY]",
    ),
    # Bearer tokens
    (
        re.compile(r"(?i)\bBearer\s+([a-zA-Z0-9_\-\.]{20,})\b"),
        "Bearer [REDACTED_BEARER_TOKEN]",
    ),
    # Common API Keys (OpenAI, Groq, Gemini, GitHub tokens)
    (
        re.compile(r"\b(sk-[a-zA-Z0-9]{20,}|gsk_[a-zA-Z0-9]{20,}|ghp_[a-zA-Z0-9]{30,}|github_pat_[a-zA-Z0-9_]{40,}|AIzaSy[a-zA-Z0-9_\-]{33})\b"),
        "[REDACTED_API_KEY]",
    ),
    # Generic password, secret, token, api_key in assignments
    (
        re.compile(
            r"(?i)\b(password|passwd|secret|token|apikey|api_key|access_token|auth_token|client_secret)\s*[:=]\s*['\"]?([^\s'\";,]{6,})['\"]?"
        ),
        r'\1="[REDACTED]"',
    ),
    # Database connection strings with credentials (postgres, mysql, mongodb, redis)
    (
        re.compile(
            r"(?i)\b((?:postgres|postgresql|mysql|mongodb|redis)://[^:]+:)([^@]+)(@)"
        ),
        r"\1[REDACTED]\3",
    ),
]


def redact_secrets(text: str) -> tuple[str, int]:
    """
    Scan text and replace sensitive credentials with safe placeholders.

    Returns:
        tuple[str, int]: (redacted_text, count_of_redactions)
    """
    if not text:
        return text, 0

    redacted_text = text
    total_redactions = 0

    for pattern, replacement in SECRET_PATTERNS:
        matches = pattern.findall(redacted_text)
        if matches:
            total_redactions += len(matches)
            redacted_text = pattern.sub(replacement, redacted_text)

    return redacted_text, total_redactions
