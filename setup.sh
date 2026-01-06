#!/usr/bin/env bash
set -e

echo "=== agent.rs Setup Script ==="
echo ""

# -----------------------------
# Helpers
# -----------------------------
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

abort() {
    echo ""
    echo "❌ $1"
    echo ""
    exit 1
}

warn() {
    echo "⚠️  $1"
}

ok() {
    echo "✅ $1"
}

info() {
    echo "ℹ️  $1"
}

# -----------------------------
# Rust toolchain (GLOBAL)
# -----------------------------
if ! command_exists cargo || ! command_exists rustc; then
    abort "Rust is not installed. Install from https://rustup.rs/"
fi

ok "Rust is installed: $(rustc --version)"

# -----------------------------
# Native compiler (REQUIRED FOR agent-native)
# -----------------------------
if command_exists clang; then
    ok "Found C/C++ compiler: $(clang --version | head -n1)"
elif command_exists gcc; then
    ok "Found C/C++ compiler: $(gcc --version | head -n1)"
else
    abort "No C/C++ compiler found. Install clang or gcc."
fi

# -----------------------------
# Platform-specific checks
# -----------------------------
OS="$(uname -s)"

if [[ "$OS" == "Darwin" ]]; then
    if ! xcode-select -p >/dev/null 2>&1; then
        abort "Xcode Command Line Tools not installed. Run: xcode-select --install"
    fi
    ok "Xcode Command Line Tools are installed"
elif [[ "$OS" == "Linux" ]]; then
    ok "Linux detected"
else
    warn "Unknown OS ($OS). Build may fail."
fi

# -----------------------------
# Optional native helpers
# -----------------------------
if ! command_exists make; then
    warn "'make' not found (llama.cpp build may require it)"
fi

if ! command_exists pkg-config; then
    warn "'pkg-config' not found (may be required on some systems)"
fi

# -----------------------------
# WASM target (agent-wasm only)
# -----------------------------
if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
    info "Installing wasm32-unknown-unknown target..."
    rustup target add wasm32-unknown-unknown
    ok "WASM target installed"
else
    ok "WASM target already installed"
fi

# -----------------------------
# Build: agent-core
# -----------------------------
echo ""
echo "=== Building agent-core ==="
echo "Pure Rust, no native dependencies, WASM-compatible"
echo ""

cargo build --package agent-core --release

# -----------------------------
# Build: agent-wasm
# -----------------------------
echo ""
echo "=== Building agent-wasm ==="
echo "Compiling agent-core logic to wasm32-unknown-unknown"
echo ""

cargo build \
  --package agent-wasm \
  --release \
  --target wasm32-unknown-unknown

# -----------------------------
# Build: agent-native
# -----------------------------
echo ""
echo "=== Building agent-native ==="
echo "This builds llama.cpp via Rust bindings (llama-cpp-sys-2)"
echo ""

# CMake is REQUIRED here due to llama-cpp-sys-2
if ! command_exists cmake; then
    echo ""
    echo "❌ CMake is required to build agent-native."
    echo ""
    echo "Reason:"
    echo "  agent-native depends on llama-cpp-sys-2,"
    echo "  which builds the vendored C++ llama.cpp via CMake."
    echo ""

    if [[ "$OS" == "Darwin" ]]; then
        echo "Install via Homebrew:"
        echo "  brew install cmake"
    elif [[ "$OS" == "Linux" ]]; then
        echo "Install via your package manager:"
        echo "  Ubuntu/Debian: sudo apt install cmake"
        echo "  Fedora:        sudo dnf install cmake"
        echo "  Arch:          sudo pacman -S cmake"
    else
        echo "Install from: https://cmake.org/download/"
    fi

    exit 1
fi

ok "CMake found: $(cmake --version | head -n1)"

echo ""
echo "Building agent-native (this may take several minutes)..."
cargo build --package agent-native --release

# -----------------------------
# Tests
# -----------------------------
echo ""
echo "=== Running Tests ==="
echo ""

cargo test --package agent-core
cargo test --package agent-wasm

# agent-native tests are optional; they may require a model
info "Skipping agent-native tests (model-dependent)"

# -----------------------------
# Done
# -----------------------------
echo ""
echo "=== Setup Complete ==="
echo ""

echo "Native binary:"
echo "  ./target/release/agent-native"
echo ""

echo "WASM module:"
echo "  ./target/wasm32-unknown-unknown/release/agent_wasm.wasm"
echo ""

echo "To run the demo, download a GGUF model:"
echo "  https://huggingface.co/models?library=gguf"
echo ""

echo "Recommended small models:"
echo "  - ibm-granite/granite-3.1-2b-instruct-GGUF"
echo "  - Qwen/Qwen2.5-1.5B-Instruct-GGUF"
echo ""

echo "Example:"
echo "  ./target/release/agent-native \\"
echo "    --model /path/to/model.gguf \\"
echo '    --query "List files and show disk usage"'
echo ""
