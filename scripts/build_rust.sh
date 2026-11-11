#!/usr/bin/env bash
# OnyxSH Native Rust Workspace Build & Verification Script
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

CARGO_BIN="${CARGO_BIN:-$(command -v cargo || echo "$HOME/.cargo/bin/cargo")}"

if [ -f "scripts/sync_translations.py" ] && command -v msgfmt >/dev/null 2>&1; then
  echo "==> [0/3] Compiling i18n .mo catalogs across all 28 languages..."
  python3 scripts/sync_translations.py --compile-only
fi

echo "==> [1/3] Running Rust Workspace Unit Tests..."
"$CARGO_BIN" test --workspace

echo "==> [2/3] Building Optimized Release Binaries (onyxsh & onyxsh-admin-helper)..."
"$CARGO_BIN" build --release --workspace

echo "==> [3/3] Verifying Compiled Artifacts..."
ls -lh target/release/onyxsh target/release/onyxsh-admin-helper
echo "OnyxSH Rust build completed successfully."
