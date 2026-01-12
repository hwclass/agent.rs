#!/usr/bin/env bash
set -euo pipefail

# Build script for browser example WASM module
# This script builds agent-wasm and copies artifacts into public/agent-wasm/

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
WASM_CRATE="$REPO_ROOT/crates/agent-wasm"
PUBLIC_DIR="$SCRIPT_DIR/public/agent-wasm"

echo "🔧 Building WASM module..."

# Build WASM with wasm-pack
cd "$WASM_CRATE"
wasm-pack build --target web

echo "📦 Copying WASM artifacts to public/agent-wasm/..."

# Create public directory if it doesn't exist
mkdir -p "$PUBLIC_DIR"

# Copy generated artifacts
cp "$WASM_CRATE/pkg/agent_wasm.js" "$PUBLIC_DIR/"
cp "$WASM_CRATE/pkg/agent_wasm_bg.wasm" "$PUBLIC_DIR/"

echo "✅ WASM build complete"
echo "   Artifacts available at: public/agent-wasm/"
