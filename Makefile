.PHONY: \
	all setup \
	check check-deps \
	build build-core build-wasm build-native \
	test test-core test-wasm test-native \
	wasm demo clean help

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
# Demo
# -----------------------------
demo:
ifndef MODEL_PATH
	@echo "❌ Error: MODEL_PATH not set"
	@echo ""
	@echo "Usage:"
	@echo "  make demo MODEL_PATH=/path/to/model.gguf"
	@echo ""
	@exit 1
endif
	@echo "Running agent-native demo..."
	./target/release/agent-native \
		--model $(MODEL_PATH) \
		--query "List the files in this directory and show disk usage"

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
	@echo "Other:"
	@echo "  make wasm          Build WASM artifact"
	@echo "  make demo MODEL_PATH=..."
	@echo "  make check         Cargo check only"
	@echo "  make clean"
	@echo "  make help"
	@echo ""
