/**
 * Minimal Deno Edge Example for agent.rs
 *
 * This demonstrates agent-core running in a stateless edge environment:
 * - WASM for agent decision logic
 * - HTTP-based LLM inference
 * - Edge-safe tools (fetch_url only)
 * - Same guardrails as native and browser
 *
 * This is a REFERENCE HOST, not a production runtime.
 */

import { load as loadEnv } from '@std/dotenv';

// WASM types (matching agent-wasm exports)
interface AgentWasm {
  create_agent_state(query: string): string;
  run_agent_step(inputJson: string): string;
}

type AgentDecision =
  | { type: 'invoke_tool'; tool: string; params: Record<string, unknown> }
  | { type: 'done'; answer: string }
  | { type: 'inconclusive'; output: string };

// Config from environment
interface Config {
  llmEndpoint: string;
  llmApiKey?: string;
  llmModel: string;
}

// Globals
let wasm: AgentWasm | null = null;

/**
 * Load WASM module
 */
async function loadWASM(): Promise<void> {
  const wasmPath = new URL('./agent-wasm/agent_wasm_bg.wasm', import.meta.url);
  const jsPath = new URL('./agent-wasm/agent_wasm.js', import.meta.url);

  try {
    // Import the JS bindings
    const mod = await import(jsPath.href);

    // Initialize with WASM binary
    await mod.default(wasmPath.href);

    wasm = {
      create_agent_state: mod.create_agent_state,
      run_agent_step: mod.run_agent_step,
    };

    console.log('✅ WASM loaded successfully');
  } catch (error) {
    console.error('❌ Failed to load WASM:', error);
    console.error('\nTo build WASM:');
    console.error('  cd ../../crates/agent-wasm');
    console.error('  wasm-pack build --target web');
    console.error('  cp pkg/agent_wasm* ../../examples/edge/agent-wasm/');
    throw new Error('WASM initialization failed');
  }
}

/**
 * Get config from environment
 */
function getConfig(): Config {
  const llmEndpoint = Deno.env.get('LLM_ENDPOINT');
  const llmModel = Deno.env.get('LLM_MODEL') || 'gpt-3.5-turbo';

  if (!llmEndpoint) {
    throw new Error('LLM_ENDPOINT environment variable is required');
  }

  return {
    llmEndpoint,
    llmApiKey: Deno.env.get('LLM_API_KEY'),
    llmModel,
  };
}

/**
 * Call LLM via HTTP (OpenAI-compatible)
 */
async function callLLM(config: Config, prompt: string): Promise<string> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
  };

  if (config.llmApiKey) {
    headers['Authorization'] = `Bearer ${config.llmApiKey}`;
  }

  const response = await fetch(config.llmEndpoint, {
    method: 'POST',
    headers,
    body: JSON.stringify({
      model: config.llmModel,
      messages: [{ role: 'user', content: prompt }],
      temperature: 0.7,
      max_tokens: 256,
    }),
  });

  if (!response.ok) {
    throw new Error(`LLM request failed: ${response.status} ${response.statusText}`);
  }

  const data = await response.json();
  return data.choices[0].message.content;
}

/**
 * Execute edge-safe tool
 */
async function executeTool(
  tool: string,
  params: Record<string, unknown>
): Promise<{ success: boolean; output: string; error?: string }> {
  try {
    if (tool === 'fetch_url') {
      const url = params.url as string;
      if (!url) throw new Error('Missing url parameter');

      const response = await fetch(url);
      if (!response.ok) throw new Error(`HTTP ${response.status}`);

      const text = await response.text();
      return { success: true, output: text.substring(0, 2000) };
    }

    throw new Error(`Unknown tool: ${tool}`);
  } catch (error) {
    return {
      success: false,
      output: '',
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Validate tool output with guardrails (same logic as native/browser)
 */
function validateOutput(output: string): { accept: boolean; reason?: string } {
  if (output.trim().length === 0) {
    return { accept: false, reason: 'Empty output' };
  }

  // Check for metadata-only (e.g., "total 123")
  const trimmed = output.trim();
  if (trimmed.split('\n').length === 1) {
    const parts = trimmed.split(/\s+/);
    if (
      parts.length === 2 &&
      parts[0].toLowerCase() === 'total' &&
      /^\d+$/.test(parts[1])
    ) {
      return { accept: false, reason: 'Metadata-only output (total line)' };
    }
  }

  if (output.length < 3 || !/[a-zA-Z0-9]/.test(output)) {
    return { accept: false, reason: 'Lacks substantive content' };
  }

  return { accept: true };
}

/**
 * Process agent query
 */
async function runAgent(query: string, config: Config): Promise<Response> {
  if (!wasm) throw new Error('WASM not initialized');

  // Create agent state
  const stateJson = wasm.create_agent_state(query);

  // System prompt with tool obligations
  const systemPrompt = `You are an agent with tools:
- fetch_url: {"tool":"fetch_url","url":"https://httpbin.org/json"}

CRITICAL RULES:
1. If a task requires fetching data from EXTERNAL URLs, you MUST invoke fetch_url.
2. If a task can be answered with your own knowledge (math, general facts), answer DIRECTLY without tools.
3. Do NOT explain what tool should be used.
4. Do NOT describe how to solve the task.

Examples:
User: Fetch data from https://api.example.com
Assistant: {"tool":"fetch_url","url":"https://api.example.com"}

User: What is 2 + 2?
Assistant: 4

User: What is the capital of France?
Assistant: Paris

Respond with JSON to use a tool, or plain text to answer directly.`;

  const userPrompt = `${systemPrompt}\n\nUser: ${query}\n\nAssistant:`;

  // Call LLM
  const modelOutput = await callLLM(config, userPrompt);

  // Process with WASM
  const stepInput = { state_json: stateJson, model_output: modelOutput };
  const stepOutput = JSON.parse(wasm.run_agent_step(JSON.stringify(stepInput)));
  const decision: AgentDecision = stepOutput.decision;

  // Handle decision
  if (decision.type === 'invoke_tool') {
    // Execute tool
    const result = await executeTool(decision.tool, decision.params);

    if (!result.success) {
      return new Response(
        JSON.stringify({
          error: 'Tool execution failed',
          details: result.error,
          host: 'edge',
        }),
        {
          status: 500,
          headers: { 'Content-Type': 'application/json' },
        }
      );
    }

    // Validate with guardrail
    const validation = validateOutput(result.output);

    if (validation.accept) {
      return new Response(
        JSON.stringify({
          result: result.output,
          host: 'edge',
          model: config.llmModel,
          guardrail: 'accept',
        }),
        {
          status: 200,
          headers: { 'Content-Type': 'application/json' },
        }
      );
    } else {
      return new Response(
        JSON.stringify({
          error: 'Guardrail rejection',
          reason: validation.reason,
          details:
            'Tool executed successfully, but output was semantically invalid. The system refused to return incorrect results (by design).',
          host: 'edge',
          guardrail: 'reject',
        }),
        {
          status: 422,
          headers: { 'Content-Type': 'application/json' },
        }
      );
    }
  } else if (decision.type === 'done') {
    return new Response(
      JSON.stringify({
        result: decision.answer,
        host: 'edge',
        model: config.llmModel,
        guardrail: 'n/a',
      }),
      {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }
    );
  } else {
    return new Response(
      JSON.stringify({
        error: 'Model produced inconclusive output',
        details: 'Task cannot be completed',
        host: 'edge',
      }),
      {
        status: 500,
        headers: { 'Content-Type': 'application/json' },
      }
    );
  }
}

/**
 * HTTP request handler
 */
async function handler(req: Request): Promise<Response> {
  // Only accept POST
  if (req.method !== 'POST') {
    return new Response(
      JSON.stringify({ error: 'Method not allowed. Use POST.' }),
      {
        status: 405,
        headers: { 'Content-Type': 'application/json' },
      }
    );
  }

  try {
    const body = await req.json();
    const query = body.query as string;

    if (!query) {
      return new Response(
        JSON.stringify({ error: 'Missing "query" field in request body' }),
        {
          status: 400,
          headers: { 'Content-Type': 'application/json' },
        }
      );
    }

    const config = getConfig();
    return await runAgent(query, config);
  } catch (error) {
    console.error('Request error:', error);
    return new Response(
      JSON.stringify({
        error: 'Internal server error',
        details: error instanceof Error ? error.message : String(error),
      }),
      {
        status: 500,
        headers: { 'Content-Type': 'application/json' },
      }
    );
  }
}

/**
 * Initialize and start server
 */
async function main() {
  console.log('🚀 agent.rs Edge Example (Deno)');
  console.log('================================\n');

  // Load .env from repository root (two levels up from examples/edge/)
  try {
    await loadEnv({ envPath: '../../.env', export: true });
    console.log('✅ Loaded configuration from .env file\n');
  } catch {
    // .env file doesn't exist - use environment variables directly
  }

  // Load WASM
  await loadWASM();

  // Verify config
  try {
    const config = getConfig();
    console.log(`LLM Endpoint: ${config.llmEndpoint}`);
    console.log(`LLM Model: ${config.llmModel}`);
    console.log(`API Key: ${config.llmApiKey ? 'configured' : 'not set'}\n`);
  } catch (error) {
    console.error('❌ Configuration error:', error);
    console.error('\nRequired environment variables:');
    console.error('  LLM_ENDPOINT - OpenAI-compatible endpoint');
    console.error('  LLM_MODEL    - Model name (default: gpt-3.5-turbo)');
    console.error('  LLM_API_KEY  - API key (optional)\n');
    Deno.exit(1);
  }

  // Start server
  const port = 8000;
  console.log(`Server listening on http://localhost:${port}`);
  console.log('Send POST requests with: {"query": "your query here"}\n');

  await Deno.serve({ port }, handler);
}

// Run
main();
