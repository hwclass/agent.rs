.PHONY: \
	all setup \
	check check-deps \
	build build-core build-wasm build-native \
	test test-core test-wasm test-native \
	wasm demo demo-shell demo-browser demo-edge serve-docs clean help

# Load .env file if it exists (for environment variables)
-include .env
export

# -----------------------------
# Default target
# -----------------------------
all: setup

# -----------------------------
# Dependency checks (lightweight)
# -----------------------------
check-deps:
	@echo "Checking base dependencies..."
	@command -v cargo >/dev/null 2>&1 || { echo "❌ Rust (cargo) not found. Install from https://rustup.rs/"; exit 1; }
	@command -v rustc >/dev/null 2>&1 || { echo "❌ Rust (rustc) not found. Install from https://rustup.rs/"; exit 1; }
	@command -v clang >/dev/null 2>&1 || command -v gcc >/dev/null 2>&1 || { echo "❌ C/C++ compiler (clang or gcc) required"; exit 1; }
	@echo "✅ Base dependencies OK"

check-native-deps:
	@echo "Checking native (agent-native) dependencies..."
	@command -v cmake >/dev/null 2>&1 || { \
		echo "❌ CMake is required for agent-native (llama-cpp-sys-2)"; \
		echo "   See setup.sh for installation instructions."; \
		exit 1; \
	}
	@echo "✅ Native dependencies OK"

# -----------------------------
# Setup
# -----------------------------
setup: check-deps
	@echo "Running setup.sh (full project setup)..."
	@./setup.sh

# -----------------------------
# Build targets
# -----------------------------
build: build-core build-wasm build-native

build-core:
	@echo "Building agent-core (pure Rust)..."
	cargo build --package agent-core --release

build-wasm:
	@echo "Building agent-wasm (wasm32-unknown-unknown)..."
	rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
	cargo build \
		--package agent-wasm \
		--target wasm32-unknown-unknown \
		--release

build-native: check-native-deps
	@echo "Building agent-native (LLM + tools, llama.cpp via CMake)..."
	cargo build --package agent-native --release

# -----------------------------
# Test targets
# -----------------------------
test: test-core test-wasm

test-core:
	@echo "Testing agent-core..."
	cargo test --package agent-core

test-wasm:
	@echo "Testing agent-wasm..."
	cargo test --package agent-wasm

test-native:
	@echo "Testing agent-native (may require model)..."
	cargo test --package agent-native || \
		echo "⚠️  agent-native tests failed or skipped (model-dependent)"

# -----------------------------
# WASM convenience
# -----------------------------
wasm: build-wasm
	@echo "WASM build complete:"
	@echo "  target/wasm32-unknown-unknown/release/agent_wasm.wasm"

# -----------------------------
# Examples / Demos
# -----------------------------

# Default demo (backwards compatibility)
demo:
	@echo "Available demos:"
	@echo "  make demo-shell     Native CLI with llama.cpp (requires MODEL_PATH)"
	@echo "  make demo-browser   Browser with WebLLM (local-first)"
	@echo "  make demo-edge      Edge runtime with Deno (requires LLM_ENDPOINT)"
	@echo ""

# Shell/Native demo (original demo target)
demo-shell:
ifndef MODEL_PATH
	@echo "❌ Error: MODEL_PATH not set"
	@echo ""
	@echo "Usage:"
	@echo "  make demo-shell MODEL_PATH=/path/to/model.gguf"
	@echo ""
	@echo "Or configure in .env file:"
	@echo "  cp .env.example .env"
	@echo "  # Edit .env and set MODEL_PATH=/path/to/model.gguf"
	@echo "  make demo-shell"
	@echo ""
	@exit 1
endif
	@echo "Running agent-native demo (shell)..."
	./target/release/agent-native \
		--model $(MODEL_PATH) \
		--query "List the files in this directory and show disk usage"

# Browser demo
demo-browser:
	@echo "🔧 Preparing browser example..."
	@cd examples/browser && ./build-wasm.sh
	@echo ""
	@echo "🚀 Starting dev server..."
	@echo "   Open http://localhost:8080 in your browser"
	@echo ""
	@cd examples/browser && ./run-dev.sh

# Edge demo (Deno)
demo-edge:
	@echo "🔧 Preparing edge example..."
	@cd examples/edge && ./build-wasm.sh
	@echo ""
ifndef LLM_ENDPOINT
	@echo "⚠️  Warning: LLM_ENDPOINT not set"
	@echo ""
	@echo "Configure in .env file (recommended):"
	@echo "  cp .env.example .env"
	@echo "  # Edit .env and set LLM_ENDPOINT, LLM_API_KEY, LLM_MODEL"
	@echo "  make demo-edge"
	@echo ""
	@echo "Or export environment variables:"
	@echo "  export LLM_ENDPOINT='https://api.openai.com/v1/chat/completions'"
	@echo "  export LLM_API_KEY='sk-...'"
	@echo "  export LLM_MODEL='gpt-3.5-turbo'"
	@echo ""
	@echo "Starting server anyway (will fail on first request without config)..."
	@echo ""
endif
	@echo "🚀 Starting Deno edge server..."
	@echo "   Server will listen on http://localhost:8000"
	@echo "   Send POST requests with: {\"query\": \"your query\"}"
	@echo ""
	@cd examples/edge && deno task start

# Documentation
serve-docs:
	@echo "📚 Starting local server for GitHub Pages..."
	@echo "   Open http://localhost:3000 in your browser"
	@echo ""
	@command -v python3 >/dev/null 2>&1 && \
		cd docs && python3 -m http.server 3000 || \
		{ echo "❌ Python 3 not found. Install Python or use another HTTP server."; exit 1; }

# -----------------------------
# Quick check (no build)
# -----------------------------
check:
	@echo "Running cargo check (no build)..."
	cargo check --all

# -----------------------------
# Cleanup
# -----------------------------
clean:
	@echo "Cleaning build artifacts..."
	cargo clean

# -----------------------------
# Help
# -----------------------------
help:
	@echo "agent.rs - Local LLM Agent (Makefile)"
	@echo ""
	@echo "Core targets:"
	@echo "  make setup         Full setup (delegates to setup.sh)"
	@echo "  make build         Build all crates"
	@echo "  make test          Test agent-core + agent-wasm"
	@echo ""
	@echo "Granular builds:"
	@echo "  make build-core    Build agent-core only (no native deps)"
	@echo "  make build-wasm    Build agent-wasm only"
	@echo "  make build-native  Build agent-native (requires CMake)"
	@echo ""
	@echo "Testing:"
	@echo "  make test-core"
	@echo "  make test-wasm"
	@echo "  make test-native"
	@echo ""
	@echo "Examples/Demos:"
	@echo "  make demo          Show available demos"
	@echo "  make demo-shell    Native CLI (requires MODEL_PATH)"
	@echo "  make demo-browser  Browser with WebLLM"
	@echo "  make demo-edge     Edge runtime (requires LLM_ENDPOINT)"
	@echo ""
	@echo "Documentation:"
	@echo "  make serve-docs    Run GitHub Pages site locally (port 3000)"
	@echo ""
	@echo "Other:"
	@echo "  make wasm          Build WASM artifact"
	@echo "  make check         Cargo check only"
	@echo "  make clean"
	@echo "  make help"
	@echo ""
