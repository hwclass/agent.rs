#!/usr/bin/env bash
set -euo pipefail

# Build script for edge example WASM module
# This script builds agent-wasm and copies artifacts into agent-wasm/

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
WASM_CRATE="$REPO_ROOT/crates/agent-wasm"
TARGET_DIR="$SCRIPT_DIR/agent-wasm"

echo "🔧 Building WASM module..."

# Build WASM with wasm-pack
cd "$WASM_CRATE"
wasm-pack build --target web

echo "📦 Copying WASM artifacts to agent-wasm/..."

# Create target directory if it doesn't exist
mkdir -p "$TARGET_DIR"

# Copy generated artifacts
cp "$WASM_CRATE/pkg/agent_wasm.js" "$TARGET_DIR/"
cp "$WASM_CRATE/pkg/agent_wasm_bg.wasm" "$TARGET_DIR/"

echo "✅ WASM build complete"
echo "   Artifacts available at: agent-wasm/"
