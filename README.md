# agent.rs

**A proof-of-concept local LLM agent inspired by Mozilla's agent.cpp**

This project demonstrates core agent loop semantics: a local GGUF model that invokes tools in a loop until it reaches a final answer. Built with architectural clarity as the primary goal.

## Architecture

### Design Principles

This is **agent.cpp-inspired, not a clone**. Key differences:

- **Uses Rust llama.cpp bindings** (not vendored C++ submodule)
- **Agent logic is WASM-portable** (LLM inference stays native)
- **Library-first design** (pure core + platform-specific implementations)
- **Single tool demo** (shell with human approval)

### Crate Structure

```
agent-rs/
├── crates/
│   ├── agent-core/       # Pure Rust, WASM-compatible agent logic
│   ├── agent-native/     # CLI demo with llama.cpp
│   └── agent-wasm/       # WASM compilation target
```

#### agent-core

Pure Rust agent logic with **zero** platform dependencies:

- **agent.rs** - Agent state management and decision loop
- **protocol.rs** - Parse model output (JSON tool call vs plain text answer)
- **tool.rs** - Tool request/result abstractions

Compiles to `wasm32-unknown-unknown` without feature flags.

#### agent-native

Native CLI demo:

- Loads GGUF models via [llama-cpp-2](https://crates.io/crates/llama-cpp-2)
- Implements shell tool with human-in-the-loop approval
- Runs agent loop until final answer or max iterations

#### agent-wasm

WASM compilation proof:

- Exports `run_agent_step()` - process one model output → decision
- Proves agent logic is **sandboxable and embeddable**
- Does NOT run LLM inference in WASM (by design)

## Tool Invocation Protocol

The model invokes tools via JSON:

```json
{
  "tool": "shell",
  "command": "ls -la"
}
```

**Rules:**

- Presence of `"tool"` field → tool invocation
- Any other output → final answer
- No schema negotiation, no OpenAI-style function calling

## Quick Start

### Prerequisites

- **Rust 1.75+**
- **C/C++ compiler** (clang or gcc)
- **CMake** (required by llama-cpp-2 bindings)
- **macOS**: Xcode Command Line Tools (`xcode-select --install`)
- **Linux**: `build-essential` package

### Step-by-Step Setup

**1. Install CMake**

```bash
# macOS
brew install cmake

# Ubuntu/Debian
sudo apt install cmake

# Arch
sudo pacman -S cmake
```

**2. Download a GGUF Model**

```bash
# Recommended: Granite 4.0 Micro (compact, fast)
wget https://huggingface.co/ibm-granite/granite-4.0-micro-GGUF/resolve/main/granite-4.0-micro-Q8_0.gguf

# Alternative: Granite 3.1 2B (larger, more capable)
# wget https://huggingface.co/ibm-granite/granite-3.1-2b-instruct-GGUF/resolve/main/granite-3.1-2b-instruct-Q4_K_M.gguf
```

**3. Build the Project**

```bash
make setup
```

The setup script will:
- Verify Rust toolchain and CMake
- Check for C/C++ compiler
- Install WASM target
- Build all crates
- Run tests

**4. Run the Demo**

```bash
./target/release/agent-native \
  --model ./granite-4.0-micro-Q8_0.gguf \
  --query "List files and show disk usage"
```

### Example Session

```
=== agent.rs ===
Query: List files and show disk usage

→ shell: ls -la
  Execute? (y/n): y

total 48
drwxr-xr-x  11 user  staff   352 Jan  6 16:42 .
drwxr-xr-x  27 user  staff   864 Jan  6 15:30 ..
-rw-r--r--   1 user  staff  6148 Jan  6 15:30 .DS_Store
-rw-r--r--   1 user  staff 18520 Jan  6 15:31 Cargo.lock
-rw-r--r--   1 user  staff   254 Jan  6 15:30 Cargo.toml
-rw-r--r--   1 user  staff  9705 Jan  6 16:42 README.md
drwxr-xr-x   5 user  staff   160 Jan  6 15:30 crates
drwxr-xr-x   8 user  staff   256 Jan  6 16:52 target

OBSERVATIONS
- Directory contains 11 items
- Key files: Cargo.toml, README.md, Makefile
- Includes crates/ and target/ directories

FINAL ANSWER
The directory contains 11 items including project files and build artifacts, totaling approximately 6.7 MB.
```

## Building for WASM

The `agent-wasm` crate compiles the **pure agent decision logic** to WebAssembly. This proves the agent is portable, sandboxable, and embeddable.

**What WASM does:**
- Parses model output (JSON tool call vs plain text)
- Updates agent state
- Returns decisions (invoke tool / done)

**What WASM does NOT do:**
- Run LLM inference
- Execute tools
- Perform I/O

The host (JavaScript, native, edge worker) provides model output and executes tools. WASM is a **pure state transition engine**.

### Build

```bash
# Install wasm32 target
rustup target add wasm32-unknown-unknown

# Build agent-wasm
cargo build --target wasm32-unknown-unknown --package agent-wasm

# Or use wasm-pack for JavaScript bindings
cd crates/agent-wasm
wasm-pack build --target web
```

### Using in JavaScript

```javascript
import init, { create_agent_state, run_agent_step } from './agent_wasm.js';

await init();

// 1. Create initial agent state
let stateJson = create_agent_state("List files");

// 2. Host provides model output (from your LLM)
const modelOutput = '{"tool":"shell","command":"ls"}';

// 3. WASM processes observation and returns decision
const input = {
  state_json: stateJson,
  model_output: modelOutput
};

const output = JSON.parse(run_agent_step(JSON.stringify(input)));

// 4. Host handles the decision
if (output.decision.type === "invoke_tool") {
  console.log("Tool requested:", output.decision.tool);
  console.log("Parameters:", output.decision.params);

  // Host executes tool (not shown)
  // Host provides tool output to next iteration
  stateJson = output.state_json;  // Update state
}
```

**Execution Contract:**
- Host runs LLM → produces text
- WASM receives text → produces decision
- Host executes tool → produces output
- Repeat until `decision.type === "done"`

## Agent Loop Semantics

The core agent loop is deterministic and pure:

1. **Receive** current state + model output
2. **Parse** output to detect tool call vs final answer
3. **Decide** next action (invoke tool OR done)
4. **Update** state with decision
5. **Repeat** until final answer or max iterations

This logic has:

- ✅ No side effects
- ✅ No IO
- ✅ No FFI
- ✅ Compiles to WASM

## Human-in-the-Loop Safety

The shell tool requires explicit approval:

```
Execute this command? (y/n):
```

Rejected commands return an error to the agent, allowing it to:

- Try a different approach
- Ask for clarification
- Provide a final answer without tool use

## Architectural Decisions

### Why CMake IS required

**The `llama-cpp-2` crate uses CMake internally to build llama.cpp.**

Even though this is a Rust project, the `llama-cpp-2` bindings:
- Vendor the C++ llama.cpp library
- Use CMake (via the `cmake` crate) to configure and build it
- Link the compiled library into the Rust binary

**Users need**:
- ✅ Rust toolchain (cargo, rustc)
- ✅ C/C++ compiler (clang or gcc)
- ✅ **CMake** (required by llama-cpp-sys-2)

The `setup.sh` script checks for CMake and provides installation instructions if missing.

### Why not vendor llama.cpp?

**agent.cpp vendors llama.cpp because it's C++.** Rust already has stable bindings via `llama-cpp-2`. Vendoring would add:

- Git submodule complexity
- Custom build scripts
- Manual FFI bindings

Using existing crates is architecturally cleaner than vendoring.

### Why WASM if LLM doesn't run in it?

**WASM proves the agent is portable, sandboxable, and embeddable.**

Running GGML inference in WASM is possible but orthogonal. This implementation shows:

- Agent logic is platform-independent
- Can be embedded in browsers, edge workers, or plugins
- Decision-making is isolated from inference backend

### Why only one tool?

**Correct architecture > feature count.**

One tool demonstrates:

- Tool protocol design
- Human approval flow
- Agent loop with feedback

More tools would dilute the core concepts.

## Non-Goals

This is a **proof-of-concept**, not a production framework:

- ❌ Multiple tools
- ❌ Memory/embeddings
- ❌ Streaming tokens
- ❌ Web UI
- ❌ Full WASM inference
- ❌ Feature parity with agent.cpp

## Testing

```bash
# Test agent-core (pure Rust logic)
cargo test --package agent-core

# Test agent-wasm
cargo test --package agent-wasm

# Test everything
cargo test --all
```

## Model Recommendations

For best results with the demo:

- **Granite-3.1-2B-Instruct** - Small, fast, instruction-tuned
- **Llama-3.2-1B-Instruct** - Tiny but capable
- **Qwen2.5-1.5B-Instruct** - Good tool-calling behavior

Larger models work better but are slower. For a quick demo, use the smallest instruct-tuned model you can find.

## Extending the Demo

### Adding a New Tool

1. **Define the tool interface** in your tool implementation
2. **Update the system prompt** to describe the tool
3. **Add a handler** in `execute_tool()` in [agent-native/src/main.rs](crates/agent-native/src/main.rs)

Example:

```rust
fn execute_tool(request: &ToolRequest) -> Result<ToolResult> {
    match request.tool.as_str() {
        "shell" => execute_shell_tool(request),
        "calculator" => execute_calculator_tool(request),  // New!
        _ => Ok(ToolResult::failure(format!("Unknown tool: {}", request.tool))),
    }
}
```

### Using a Different LLM Backend

The agent-core logic is backend-agnostic. To use a different inference backend:

1. Keep agent-core unchanged
2. Create a new crate (e.g., `agent-llamacpp-rs`, `agent-candle`, etc.)
3. Implement your own `generate()` function
4. Use the same agent loop semantics

## License

MIT OR Apache-2.0

## Acknowledgments

Inspired by [agent.cpp](https://github.com/Mozilla-Ocho/llamafile/tree/main/agent.cpp) from Mozilla Ocho.

## FAQ

### Is this production-ready?

No. This is a **proof-of-concept** demonstrating core concepts. It lacks:

- Error recovery
- Proper token management
- Streaming
- State persistence
- Security hardening

### Why Rust?

- Memory safety without GC
- WASM-first ecosystem
- Excellent FFI story
- Strong type system for protocol design

### Can I use this with OpenAI/Claude/etc?

Yes! The agent-core logic is backend-agnostic. Replace the llama.cpp inference in agent-native with API calls to any LLM provider. The tool protocol and agent loop remain the same.

### How does this compare to LangChain/AutoGPT/etc?

This is **architecturally minimal**:

- No framework abstractions
- No memory systems
- No prompt templates
- Just: loop → tool → loop

It's meant to demonstrate **the core pattern**, not provide a full agent framework.
