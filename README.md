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

**3. Configure Environment (Optional)**

You can configure environment variables via a `.env` file for convenience:

```bash
# Copy the example configuration
cp .env.example .env

# Edit .env to set your paths and API keys
# MODEL_PATH=/path/to/your/model.gguf          # For native demo
# LLM_ENDPOINT=https://api.openai.com/...      # For edge demo
# LLM_API_KEY=sk-...                            # For edge demo
```

**4. Build the Project**

```bash
make setup
```

The setup script will:
- Verify Rust toolchain and CMake
- Check for C/C++ compiler
- Install WASM target
- Build all crates
- Run tests

**5. Run the Demo**

```bash
# Option 1: Direct command
./target/release/agent-native \
  --model ./granite-4.0-micro-Q8_0.gguf \
  --query "List files and show disk usage"

# Option 2: Using make (reads MODEL_PATH from .env)
make demo-shell

# Run other demos
make demo           # Show all available demos
make demo-browser   # Browser demo with WebLLM
make demo-edge      # Edge demo with Deno
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
let stateJson = create_agent_state("What is 2 + 2?");

// 2. Host provides model output (from your LLM API)
const modelOutput = '{"tool":"calculator","expression":"2+2"}';

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

  // Host executes tool (browser-safe example)
  const result = eval(output.decision.params.expression); // "4"

  // Feed result back to agent (next iteration)
  stateJson = output.state_json;
} else if (output.decision.type === "done") {
  console.log("Final answer:", output.decision.answer);
}
```

**Execution Contract:**
- Host runs LLM → produces text
- WASM receives text → produces decision
- Host executes tool → produces output
- Repeat until `decision.type === "done"`

**Note:** The `agent-native` demo uses a `shell` tool for local CLI usage. In browser/edge contexts, you'd define tools appropriate to that environment (API calls, calculations, DOM operations, etc.).

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

## Known Failure Modes (By Design)

**agent.rs prioritizes correctness over convenience.** The system includes semantic guardrails that validate tool outputs to prevent false-positive success.

### What This Means

When the agent executes a tool, it validates that the output is semantically meaningful for the requested task. If validation fails:

1. The system attempts one corrective retry with stricter instructions
2. If the retry also fails validation, **the agent fails explicitly**
3. The system will NOT return plausible-looking but incorrect results

### Why Some Tasks Fail

Some models (particularly smaller ones under 7B parameters) lack sufficient tool-reasoning capability. They may:

- Generate syntactically correct tool calls
- Execute tools successfully
- But produce outputs that don't actually satisfy the task

**Example:**
```
Query: "List the biggest file in the directory by size"
Tool call: {"tool":"shell","command":"ls -lS | head -n 1"}
Tool output: "total 7079928"
Result: ❌ REJECTED - output contains only metadata, not actual file data
```

### This is Intentional

The guardrail system (inspired by [Mozilla.ai's any-guardrail pattern](https://github.com/mozilla-ai/any-guardrail)) prevents the agent from:

- Hallucinating success based on metadata
- Accepting empty or malformed outputs
- Claiming task completion when the result is semantically invalid

**A correct system that fails honestly is better than one that returns plausible-looking but incorrect results.**

### What You Can Do

If the agent fails with guardrail rejection:

- Use a **larger model** (7B+ parameters recommended)
- Use a model **specifically fine-tuned for tool use**
- **Simplify the query** to reduce reasoning complexity
- Verify the task is **achievable with available tools**

### Future Direction

Current guardrails use heuristic validation (e.g., rejecting `"total <number>"` as metadata-only output). Future enhancements may include:

- **Tool postconditions** - explicit semantic contracts declared by tools
- **Executable validation** - tests as postconditions that verify correctness
- **Model capability negotiation** - adapting task complexity to model capabilities

See the [Roadmap](#roadmap) section below for details.

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

## Examples

Complete working examples are available in the [examples/](examples/) directory:

- **[examples/shell/](examples/shell/)** - CLI shell tool demo with human-in-the-loop approval
- **[examples/browser/](examples/browser/)** - Minimal browser host with WebLLM local inference and real browser tools
- **[examples/edge/](examples/edge/)** - Stateless edge runtime (Deno) with HTTP-based LLM and fetch_url tool

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

## Roadmap

### Semantic Validation Enhancements

Current guardrails use heuristic validation to detect invalid tool outputs. Future work will formalize correctness as a first-class architectural concept.

#### Tool Postconditions

**Problem:** Current guardrails reject obvious failures (empty output, metadata-only) but cannot verify semantic correctness.

**Solution:** Tools declare explicit contracts that outputs must satisfy.

```rust
pub struct ToolPostcondition {
    name: String,
    validate: Box<dyn Fn(&ToolOutput) -> ValidationResult>,
}

// Example: shell tool listing files
fn file_list_postcondition(output: &ToolOutput) -> ValidationResult {
    if output.lines().all(|line| line.starts_with("total")) {
        return ValidationResult::Reject("Output contains only metadata");
    }
    // More sophisticated checks...
}
```

**Impact:** Catches semantic errors that smaller models consistently make, enabling graceful degradation or task decomposition.

#### Executable Validation

**Problem:** Some correctness criteria cannot be expressed as simple predicates.

**Solution:** Tests as postconditions - executable specifications that verify outputs.

```rust
// Postcondition: "output should contain at least one file entry"
fn validate_file_list(output: &str) -> bool {
    output.lines()
        .any(|line| line.contains(".txt") || line.contains(".rs"))
}
```

**Benefit:** Aligns with agent.cpp's extensibility model and any-guardrail's pluggable validation pattern.

#### Model Capability Negotiation

**Problem:** Small models fail on complex tasks; large models are slow for simple ones.

**Solution:** Runtime capability detection and task adaptation.

- Measure model success rate on validation checks
- Decompose tasks when model struggles
- Route simple queries to fast models, complex ones to capable models

**This turns validation failures into architectural feedback.**

### Relationship to Current Failures

When the agent fails with guardrail rejection today, it's exposing a capability gap. The roadmap addresses this by:

1. **Formalizing contracts** (postconditions) - makes requirements explicit
2. **Automating verification** (executable validation) - removes heuristic guessing
3. **Adapting to models** (capability negotiation) - matches task complexity to model strength

**These enhancements don't eliminate failure - they make failure productive.**

A system that:
- Attempts the task
- Validates the result
- Fails explicitly when validation fails

...is fundamentally more trustworthy than one that always returns plausible-looking output.

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
