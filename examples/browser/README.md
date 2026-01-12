# Browser WASM Example with WebLLM (Local-First)

This is a **minimal browser host** that demonstrates **agent-core portability**.

## What This Example Proves

- **Same agent logic** runs in native and browser contexts
- **Same correctness guarantees** apply (guardrails, explicit failures)
- **Local-first inference** works via WebLLM (no API keys, no server)
- **Real browser tools** integrate with the agent

**This is intentionally minimal.**
It proves portability, not feature parity with the native runtime.

Aligned with [Mozilla AI's local-first philosophy](https://github.com/mlc-ai/web-llm).

## What This Example Does

1. Loads a small LLM model locally via WebLLM (WebGPU)
2. Executes agent decision logic via WASM
3. Runs browser tools (read_dom, fetch_url)
4. Validates tool outputs with semantic guardrails
5. **Fails explicitly** when validation fails

## What This Example Does NOT Do

- **No retry loops** (see native runtime for retry logic)
- **No advanced lifecycle hooks** (minimal host adapter only)
- **No production features** (this is a proof-of-concept)

This example demonstrates portability, not feature completeness.

## Architecture

```
┌─────────────────────────────────────┐
│         Browser Host (TS)           │
│  ┌─────────────┐  ┌──────────────┐ │
│  │   WebLLM    │  │  Real Tools  │ │
│  │  (WebGPU)   │  │  (Browser)   │ │
│  └─────────────┘  └──────────────┘ │
│         │                │          │
│         └────────┬───────┘          │
│                  │                  │
│         ┌────────▼───────┐          │
│         │  agent-core    │          │
│         │    (WASM)      │          │
│         │  + guardrails  │          │
│         └────────────────┘          │
└─────────────────────────────────────┘
```

**Key principle:**
- Agent logic (agent-core) has ZERO knowledge of WebLLM or browsers
- Host provides capabilities, not intelligence
- Guardrails behave identically to native

## Prerequisites

- **WebGPU-capable browser**: Chrome 113+, Edge 113+
  - (Safari/Firefox: experimental WebGPU support)
- **Node.js 20.19+ or 22.12+** (required by Vite 7.x)
  - `.nvmrc` file provided for automatic version selection with nvm
- **~2GB GPU memory** for Qwen2.5-3B model

## Setup

### Quick Start (Recommended)

From the repository root:

```bash
make demo-browser
```

This single command:
- Builds the WASM module via `wasm-pack`
- Copies artifacts into `public/agent-wasm/`
- Automatically uses correct Node.js version (via `.nvmrc` if nvm is available)
- Installs npm dependencies
- Starts the dev server at `http://localhost:8080`

**Note:** If you have `nvm` installed, the correct Node.js version (22) is used automatically. No manual version switching needed!

### Alternative: Manual Setup

```bash
# 1. Build and copy WASM artifacts
cd examples/browser
./build-wasm.sh

# 2. Install dependencies
npm install

# 3. Start dev server
npm run dev
```

Open `http://localhost:8080` in your browser.

**First run:** Model downloads automatically (~1.8GB Qwen2.5-3B, then cached).

### How It Works

WASM artifacts are copied into `public/agent-wasm/` to avoid Vite filesystem whitelisting. This mirrors how real applications ship WASM bundles - the browser example owns its artifacts and serves them directly.

## Example Behavior

### Success Case with fetch_url

```
Query: "Fetch data from https://httpbin.org/json"

Model: {"tool":"fetch_url","url":"https://httpbin.org/json"}
→ Tool: fetch_url
Output: {"slideshow": {"author": "Yours Truly", ...}}
✓ Guardrail: Accept

Result: [JSON data displayed]
```

### Success Case with read_dom

```
Query: "Extract the page title"

Model: {"tool":"read_dom","selector":"title"}
→ Tool: read_dom
Output: "agent.rs Browser Example"
✓ Guardrail: Accept

Result: agent.rs Browser Example
```

### Failure Case (By Design)

```
Query: "Get file sizes of all images"

Model: {"tool":"fetch_url","url":"..."}
→ Tool: fetch_url
Output: "total 12345"
✗ Guardrail: Reject - Metadata-only output (total line)

❌ TASK FAILED

Tool executed successfully, but output was semantically invalid.
The system refused to return incorrect results (by design).
```

**Failures are intentional when models cannot satisfy correctness constraints.**

## Available Tools

### `fetch_url`
Fetch content from a URL.

```json
{"tool": "fetch_url", "url": "https://httpbin.org/json"}
{"tool": "fetch_url", "url": "https://api.github.com/users/github"}
```

**CORS Handling:**
- First attempts direct fetch (works for CORS-enabled APIs)
- If CORS fails, automatically falls back to CORS proxy (allorigins.win)
- Proxy allows fetching from some non-CORS URLs

**Recommended URLs (CORS-enabled, no proxy needed):**
- `https://httpbin.org/json` - Test API with sample JSON
- `https://api.github.com/users/github` - GitHub user API
- `https://jsonplaceholder.typicode.com/posts/1` - Fake REST API

**Known Limitations:**
- 2000 character limit on response
- Large sites (Amazon, Google, etc.) may block proxy services → Empty output → Guardrail rejection ✅
- Sites requiring JavaScript execution won't work (browser fetch doesn't execute JS)
- This is by design - the guardrail prevents returning empty/invalid results

### `read_dom`
Extract text from DOM elements.

```json
{"tool": "read_dom", "selector": "h1, p.intro"}
```

## Semantic Guardrails

All tool outputs pass through the **same semantic validation** as native:

### PlausibilityGuard

Rejects:
- Empty outputs
- Metadata-only outputs (`"total 123"`)
- Outputs lacking substantive content

### Behavior

1. Tool executes successfully
2. Guardrail validates output
   - **Accept** → display result
   - **Reject** → **explicit failure**

**No silent success.** (Note: This minimal browser host does not implement corrective retry - see native runtime for full retry logic)

## Known Failure Modes (By Design)

### Tool Behavior Issues

Even capable models like Qwen2.5-3B may exhibit:

1. **Tool invocation avoidance** - Explaining instead of acting (for environmental queries)
2. **Unnecessary tool invocation** - Using tools when direct answers would work
   - Fixed via improved system prompt with examples of when NOT to use tools
   - Example: "What is 2 + 2?" should answer "4" directly
3. **Complex multi-step reasoning** - Struggles with tasks requiring multiple inference steps
4. **Tool selection errors** - Choosing wrong tool for the task

### Why This Happens

Small models (< 7B parameters) have limited reasoning capability. The Qwen2.5-3B model is:
- ✅ Good enough for reliable tool invocation (better than 1-2B models)
- ✅ Follows JSON format correctly
- ⚠️ May not always know when NOT to use tools
- ⚠️ May struggle with nuanced task understanding

**This is intentional.**

The system prioritizes correctness:
- ✅ Attempts the task
- ✅ Enforces tool obligations (queries requiring environmental data MUST invoke tools)
- ✅ Validates results
- ✅ Fails explicitly when validation fails or tools are not invoked
- ❌ Never returns explanatory prose when environmental data is required
- ❌ Never returns plausible-looking but incorrect output

### Observed Test Results

Based on testing:
- ✅ `Fetch data from https://httpbin.org/json` → Works perfectly
- ❌ `Fetch data from https://www.amazon.com` → Guardrail rejects (empty output from proxy)
- ✅ `Extract the page title` → Works perfectly
- ✅ `What is 2 + 2?` → Should answer directly (4) without invoking tools

**Example Failure:**
If a query like "Extract the page title" causes the model to explain how to extract it instead of invoking `read_dom`, the system will reject the response and fail explicitly.

**A correct system that fails honestly is better than one that returns plausible-looking but incorrect results.**

See main [README: Known Failure Modes](../../README.md#known-failure-modes-by-design) for details.

## Model Choice

This example uses **Qwen2.5-3B-Instruct-q4f16_1-MLC**, a capable instruction-tuned model supported by WebLLM.

**Why This Model:**
- Supported by WebLLM's precompiled model registry
- Good balance of capability and size (3B parameters)
- Instruction-tuned for tool use and structured outputs
- Reliable tool invocation behavior

**Characteristics:**
- Download size: ~1.8GB (cached after first load)
- GPU memory: ~2GB required
- Stronger reasoning than 1-2B models
- Better at following tool invocation instructions

**Expected Behavior:**
- Reliably invokes tools when needed
- Follows JSON tool format correctly
- Handles both DOM and fetch operations
- Still subject to guardrail validation (explicit failures)

**Note:** Smaller models (1-2B) may be faster but often fail to invoke tools correctly. This 3B model provides a good balance for demonstrating the architecture with reliable tool use.

See [WebLLM model list](https://github.com/mlc-ai/web-llm#available-models) for other supported models.

## Development

### Project Structure

```
examples/browser/
├── README.md          # This file
├── index.html         # Minimal UI
├── main.ts            # Browser host (WebLLM + tools)
├── package.json       # Dependencies
└── tsconfig.json      # TypeScript config
```

### Adding New Tools

1. Define tool in `main.ts`:

```typescript
async function executeTool(tool: string, params: any): Promise<string> {
  switch (tool) {
    case "your_tool":
      // Real browser API call
      const result = await fetch(params.url);
      return await result.text();
    default:
      throw new Error(`Unknown tool: ${tool}`);
  }
}
```

2. Update system prompt to describe the tool

3. Guardrails automatically validate output

### Testing Guardrails

Trigger failures intentionally:

```
Query: "Return the color of the third pixel in the favicon"
```

Expected: Guardrail rejection (task is not feasible with browser tools).

## Comparison to Native

| Feature                  | Native (CLI)      | Browser (WASM) |
|--------------------------|-------------------|----------------|
| Agent logic              | agent-core        | agent-core     |
| LLM backend              | llama.cpp         | WebLLM         |
| Tools                    | shell commands    | Browser APIs   |
| Guardrails               | ✓ PlausibilityGuard | ✓ PlausibilityGuard |
| Corrective retry         | ✓                 | ✓              |
| Explicit failure         | ✓                 | ✓              |
| API keys required        | ✗                 | ✗              |
| Local-first              | ✓                 | ✓              |

**Same correctness guarantees, different host.**

## Performance

- WASM init: ~10ms
- WebLLM model load: ~30s (first run, then cached)
- Inference: ~50-200ms per token (depends on model size and GPU)
- Guardrail validation: <1ms

Bottleneck is inference speed, not agent logic.

## Security

- **No eval()** - Tools are explicitly defined
- **CORS-aware** - fetch_url respects same-origin policy
- **Sandboxed** - Agent cannot escape browser security model
- **Local-only** - No data sent to servers

## Limitations

### Technical

- Requires WebGPU-capable browser
- GPU memory limits model size
- Slower than native llama.cpp
- Tool execution is synchronous

### Correctness Limitations

- Small models fail on complex tasks (by design)
- Guardrails are heuristic-based (see Roadmap for postconditions)
- Retry is limited to one attempt

**These are not bugs - they are design constraints.**

The system refuses to guess or hallucinate when it cannot verify correctness.

## Roadmap Alignment

This example demonstrates current capabilities and exposes future needs:

1. **Tool postconditions** - Browser tools could declare explicit contracts
2. **Executable validation** - Test-based verification of tool outputs
3. **Model capability negotiation** - Adapt task complexity to model size

See main [README: Roadmap](../../README.md#roadmap) for details.

## Troubleshooting

### Node.js version warning

If you see: `Vite requires Node.js version 20.19+ or 22.12+`

**This should not happen when using `make demo-browser`**, which automatically handles Node.js versions via `run-dev.sh`.

**If you're running npm directly:**

With nvm (automatic):
```bash
cd examples/browser
nvm use  # Reads .nvmrc and switches to Node.js 22
```

Without nvm:
```bash
# Download and install Node.js 22 LTS from https://nodejs.org/
# Or upgrade using your package manager
```

**For manual control:**
```bash
# Install specific version with nvm
nvm install 22
nvm use 22
```

### Model fails to load

- Check WebGPU support: `chrome://gpu`
- Ensure sufficient GPU memory (~2GB for Qwen2.5-3B)
- Try smaller model variant (e.g., `Phi-2-q4f16_1` or `TinyLlama-1.1B-q4f16_1`)

### Guardrail rejection loops

- This is expected with insufficient models
- Try simpler queries
- Check that query is achievable with available tools

### CORS errors in fetch_url

**Note:** As of this version, CORS is handled automatically via proxy fallback.

If fetch still fails:
- URL may be blocking proxy services
- Try alternative URL
- Check browser console for specific error

## License

MIT OR Apache-2.0
