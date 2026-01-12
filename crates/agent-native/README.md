# agent-native

Reference native host runtime for agent.rs.

## Architecture

This crate demonstrates how to build a **host runtime** that:
1. Selects and initializes an LLM backend
2. Executes the agent loop
3. Handles tool execution
4. Manages I/O

## LLM Backend Abstraction

The runtime uses an **LLM backend abstraction** to decouple the agent loop from inference engines.

### Interface

```rust
pub trait LLMBackend {
    fn infer(&mut self, input: LLMInput) -> Result<LLMOutput>;
}
```

Where:
- `LLMInput` contains: prompt, max_tokens, KV cache position, flags
- `LLMOutput` contains: generated text, tokens processed

### Current Backend

**`LlamaCppBackend`** - Reference implementation using llama.cpp via Rust bindings.

Located in: `src/llama_cpp_backend.rs`

### Adding a New Backend

To add a new LLM backend (e.g., Candle, llama-cpp-rs, ONNX, API-based):

1. **Implement the `LLMBackend` trait**:
   ```rust
   pub struct MyBackend {
       // Your backend state
   }

   impl LLMBackend for MyBackend {
       fn infer(&mut self, input: LLMInput) -> Result<LLMOutput> {
           // Your inference logic
       }
   }
   ```

2. **Update `main.rs` to use your backend**:
   ```rust
   let mut llm_backend = MyBackend::new(...)?;
   ```

3. **The agent loop remains unchanged** - it only calls `llm_backend.infer()`.

### Key Properties

- **Agent core never depends on LLM backends** - it only sees text input/output
- **Backends are host implementation details** - chosen at runtime initialization
- **No dynamic loading** - backends are compiled in, selected via code
- **No configuration DSLs** - explicit composition in `main()`

## Responsibility Boundaries

| Component | Responsibilities |
|-----------|------------------|
| **agent-core** | State machine, decision logic, protocol parsing |
| **LLM Backend** | Text generation (prompt → text) |
| **Native Runtime** | Agent loop, tool execution, I/O, backend selection |

## Future Backends

The abstraction supports (but does not yet implement):

- **API-based** (OpenAI, Anthropic, etc.) - HTTP calls instead of local inference
- **Candle** - Pure Rust ML framework
- **llama-cpp-rs** - Alternative Rust bindings
- **ONNX Runtime** - Cross-platform inference
- **Custom** - Any inference engine that can produce text from prompts

## Building

```bash
cargo build --release
```

## Running

```bash
./target/release/agent-native \
  --model path/to/model.gguf \
  --query "Your query here"
```

## Testing

```bash
cargo test
```

All tests pass without requiring a model file - they test the abstraction boundaries, not inference quality.
