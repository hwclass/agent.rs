# Edge Example (Deno) - Stateless HTTP Agent

This is a **minimal edge example** that demonstrates **agent-core running in a stateless, distributed environment**.

## What This Example Demonstrates

- **Same agent logic** (agent-core via WASM) runs in edge/serverless contexts
- **HTTP-based LLM inference** (OpenAI-compatible API)
- **Edge-safe tools** (fetch_url only - no filesystem, no shell)
- **Same correctness guarantees** (guardrails, explicit failures)
- **Stateless execution** (single-pass, no retries)

**This is a reference host, not a production runtime.**

It proves that agent-core can run in distributed edge environments with the same architectural principles as native and browser hosts.

## Why Deno (Edge Runtime)

Deno is chosen for this example because:

- **Edge-ready** - Runs in Deno Deploy, Cloudflare Workers (with adapter), edge functions
- **WASM support** - First-class WebAssembly integration
- **TypeScript native** - No build step for TS
- **Secure by default** - Explicit permissions for network, env, files
- **Standards-compliant** - Uses Web Platform APIs

This example works locally and can be deployed to edge platforms with minimal changes.

## Architecture

```
HTTP Request (POST /query)
    ↓
LLM (HTTP inference - OpenAI-compatible)
    ↓
agent-core (WASM)
    ↓
Decision
    ├─ invoke_tool → fetch_url (HTTP only)
    └─ done → final answer
    ↓
Guardrail validation
    ├─ accept → HTTP 200
    └─ reject → HTTP 422
    ↓
HTTP Response (JSON)
```

**Key principle:**
- Agent logic (agent-core) has ZERO knowledge of edge runtime or HTTP
- Edge host provides LLM and tools via HTTP
- WASM is a pure state transition engine
- Guardrails enforce correctness regardless of environment

## Prerequisites

- **Deno 1.40+** ([install](https://deno.land/#installation))
- **WASM artifacts** built from agent-core
- **OpenAI-compatible LLM endpoint** (OpenAI API, local server, etc.)

## Setup

### 1. Build WASM Module

From the repository root:

```bash
cd crates/agent-wasm
wasm-pack build --target web
```

### 2. Copy WASM Artifacts

```bash
mkdir -p examples/edge/agent-wasm
cp crates/agent-wasm/pkg/agent_wasm_bg.wasm examples/edge/agent-wasm/
cp crates/agent-wasm/pkg/agent_wasm.js examples/edge/agent-wasm/
```

### 3. Configure Environment

**Option 1: Use .env file (recommended)**

From the repository root:

```bash
cp .env.example .env
# Edit .env and configure:
# LLM_ENDPOINT=https://api.openai.com/v1/chat/completions
# LLM_API_KEY=sk-...
# LLM_MODEL=gpt-3.5-turbo
```

**Option 2: Export variables directly**

```bash
export LLM_ENDPOINT="https://api.openai.com/v1/chat/completions"
export LLM_API_KEY="sk-..."
export LLM_MODEL="gpt-3.5-turbo"
```

**For local LLM servers:**

```bash
# In .env or exported
LLM_ENDPOINT=http://localhost:8080/v1/chat/completions
LLM_MODEL=granite-3.1-2b-instruct
# LLM_API_KEY not required for local servers
```

### 4. Run Locally

```bash
cd examples/edge
deno task start
```

Server starts on `http://localhost:8000`

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `LLM_ENDPOINT` | ✅ Yes | - | OpenAI-compatible chat/completions endpoint |
| `LLM_MODEL` | No | `gpt-3.5-turbo` | Model name to use |
| `LLM_API_KEY` | No | - | API key for authentication (if required) |

## Usage

### Example Request

```bash
curl -X POST http://localhost:8000 \
  -H "Content-Type: application/json" \
  -d '{"query": "Fetch data from https://httpbin.org/json"}'
```

### Success Response (HTTP 200)

```json
{
  "result": "{\"slideshow\": {...}}",
  "host": "edge",
  "model": "gpt-3.5-turbo",
  "guardrail": "accept"
}
```

### Guardrail Rejection (HTTP 422)

```json
{
  "error": "Guardrail rejection",
  "reason": "Empty output",
  "details": "Tool executed successfully, but output was semantically invalid. The system refused to return incorrect results (by design).",
  "host": "edge",
  "guardrail": "reject"
}
```

### Tool Execution Error (HTTP 500)

```json
{
  "error": "Tool execution failed",
  "details": "HTTP 404",
  "host": "edge"
}
```

## Available Tools

### `fetch_url`

Fetch content from a URL (edge-safe, HTTP only).

**LLM invocation:**
```json
{"tool": "fetch_url", "url": "https://api.example.com/data"}
```

**Limitations:**
- HTTP/HTTPS only
- 2000 character limit on response
- No authentication headers (tool is minimal by design)
- Subject to edge runtime timeouts

**Note:** This is a reference implementation. Production tools would include timeout handling, streaming, etc.

## Guardrails

All tool outputs pass through **the same semantic validation** as native and browser hosts:

### PlausibilityGuard

Rejects:
- Empty outputs
- Metadata-only outputs (`"total 123"`)
- Outputs lacking substantive content

### Behavior

1. Tool executes successfully
2. Guardrail validates output
   - **Accept** → HTTP 200 with result
   - **Reject** → HTTP 422 with reason

**No silent success.**

## Known Limitations (By Design)

### Intentional Constraints

- **No retries** - Single-pass execution only (see native runtime for retry logic)
- **No state** - Stateless HTTP handler, no persistence
- **Minimal tools** - fetch_url only (no filesystem, no shell)
- **No streaming** - Response returned when complete
- **Single decision** - One LLM call per request

**Why these constraints exist:**

This example demonstrates **portability, not production features**. It proves that:
- Agent logic is environment-agnostic
- Guardrails work identically across hosts
- Failures are explicit even in edge environments

### Edge Runtime Constraints

- **Cold starts** - WASM initialization on first request
- **Timeout limits** - Edge platforms typically limit execution time (10-30s)
- **Memory limits** - WASM module size matters
- **No long-running state** - Each request is independent

**These are edge platform realities, not agent.rs limitations.**

## Relationship to Other Hosts

This edge example shares the same architectural principles as:

### Native (CLI)
- Uses agent-core for decision logic
- Enforces guardrails with PlausibilityGuard
- Fails explicitly on validation rejection
- **Difference:** Native uses llama.cpp (local inference), shell tools, retry loops

### Browser (WebLLM)
- Uses agent-core via WASM
- Same guardrail validation logic
- Same tool invocation protocol
- **Difference:** Browser uses WebLLM (local), browser-specific tools (DOM)

### Edge (This Example)
- Uses agent-core via WASM
- Same guardrail validation logic
- Same tool invocation protocol
- **Difference:** Edge uses HTTP LLM, stateless execution, edge-safe tools only

**Shared guarantees across all hosts:**
- ✅ Same agent decision logic
- ✅ Same guardrail validation
- ✅ Same tool protocol
- ✅ Same failure semantics
- ✅ No silent success

## Deployment

### Deno Deploy

1. Push to GitHub
2. Connect to Deno Deploy
3. Set environment variables in dashboard
4. Deploy

### Cloudflare Workers

Requires adapter for Deno → Workers compatibility. See [deno2node](https://deno.land/x/deno2node) or rewrite using Workers API.

### Vercel Edge Functions

Export as Vercel-compatible handler:

```typescript
export default async (req: Request) => {
  return await handler(req);
}
```

## Development

### Project Structure

```
examples/edge/
├── README.md          # This file
├── deno.json          # Deno config and tasks
├── main.ts            # Edge HTTP handler
└── agent-wasm/        # WASM artifacts (not in git)
    ├── agent_wasm.js
    └── agent_wasm_bg.wasm
```

### Adding Tools

Edge tools MUST be stateless and fast:

```typescript
async function executeTool(tool: string, params: any) {
  if (tool === 'my_tool') {
    // Execute via HTTP, KV, or other edge-safe API
    const result = await fetch(...);
    return { success: true, output: await result.text() };
  }
  // ...
}
```

**Forbidden in edge:**
- Filesystem access
- Shell commands
- Long-running operations
- Persistent state

### Testing Locally

```bash
# Terminal 1: Start edge server
deno task start

# Terminal 2: Send test request
curl -X POST http://localhost:8000 \
  -H "Content-Type: application/json" \
  -d '{"query": "Fetch https://httpbin.org/json"}'
```

## Comparison to Production Edge Runtimes

This is a **minimal reference implementation**. Production edge agents would add:

- Request validation and rate limiting
- Retry logic with exponential backoff
- Streaming responses
- Tool authentication and headers
- Request tracing and observability
- Multi-step conversation state (via KV)
- Token counting and limits
- Model fallbacks

**This example proves the architecture works. Production features are separate concerns.**

## License

MIT OR Apache-2.0

## Acknowledgments

Demonstrates agent-core portability inspired by the same architectural principles as:
- Native runtime (llama.cpp)
- Browser runtime (WebLLM)
- Edge runtime (HTTP LLM)

All hosts share the same correctness guarantees.
