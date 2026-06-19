#!/usr/bin/env bash
set -e
ROOT="$(cd "$(dirname "$0")" && pwd)"

echo "[wisp] Building Rust binary..."
cd "$ROOT/wisp"
cargo build --release

echo "[wisp] Deploying to npm/dist/..."
DEST="$ROOT/npm/dist/wisp.exe"
SRC="target/release/wisp.exe"

# On Windows, the running exe is locked — remove first, then copy.
rm -f "$DEST"
cp "$SRC" "$DEST"

echo "[wisp] Done — npm/dist/wisp.exe updated."
