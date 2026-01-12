/**
 * Minimal Browser Host for agent.rs
 *
 * This demonstrates agent-core portability: the same WASM module runs in
 * both native and browser contexts with the same correctness guarantees.
 *
 * This is a DEMONSTRATION HOST, not a second runtime.
 * - Uses real WebLLM for local inference
 * - Uses real agent-core WASM for decisions
 * - Uses real browser tools
 * - Enforces semantic guardrails
 * - Fails explicitly on invalid output
 *
 * Intentional limitations (see native runtime for full capabilities):
 * - Single agent execution flow
 * - No corrective retry loop
 * - Minimal prompt engineering
 */

import { CreateMLCEngine } from '@mlc-ai/web-llm';

// WASM types
interface AgentWasm {
  create_agent_state(query: string): string;
  run_agent_step(inputJson: string): string;
}

type AgentDecision =
  | { type: 'invoke_tool'; tool: string; params: any }
  | { type: 'done'; answer: string }
  | { type: 'inconclusive'; output: string };

// Globals
let engine: any = null;
let wasm: AgentWasm | null = null;

/**
 * Initialize WebLLM (local inference via WebGPU)
 */
async function initWebLLM(): Promise<void> {
  const statusEl = document.getElementById('model-status')!;
  const progressEl = document.getElementById('model-progress')!;
  const fill = document.getElementById('progress-bar-fill')!;

  statusEl.textContent = 'Loading WebLLM model...';
  progressEl.classList.remove('hidden');

  engine = await CreateMLCEngine('Qwen2.5-3B-Instruct-q4f16_1-MLC', {
    initProgressCallback: (p) => {
      fill.style.width = `${Math.round(p.progress * 100)}%`;
      statusEl.textContent = `Loading: ${Math.round(p.progress * 100)}%`;
    }
  });

  statusEl.textContent = 'Ready!';
  statusEl.className = 'status success';
  progressEl.classList.add('hidden');
}

/**
 * Load WASM module
 */
async function loadWASM(): Promise<void> {
  const mod = await import('/public/agent-wasm/agent_wasm.js');
  await mod.default();
  wasm = {
    create_agent_state: mod.create_agent_state,
    run_agent_step: mod.run_agent_step
  };
}

/**
 * Execute browser tool
 */
async function executeTool(tool: string, params: any): Promise<{ success: boolean; output: string; error?: string }> {
  try {
    let output: string;

    if (tool === 'read_dom') {
      if (!params.selector) throw new Error('Missing selector');
      const els = document.querySelectorAll(params.selector);
      if (els.length === 0) throw new Error(`No elements: ${params.selector}`);
      output = Array.from(els).map(el => el.textContent?.trim()).join(' | ');
    } else if (tool === 'fetch_url') {
      if (!params.url) throw new Error('Missing url');

      // Try direct fetch first (for CORS-enabled URLs)
      let res: Response;
      try {
        res = await fetch(params.url);
      } catch (corsError) {
        // If CORS fails, try with CORS proxy (allorigins.win is a public CORS proxy)
        const proxyUrl = `https://api.allorigins.win/raw?url=${encodeURIComponent(params.url)}`;
        res = await fetch(proxyUrl);
      }

      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      output = (await res.text()).substring(0, 2000);
    } else {
      throw new Error(`Unknown tool: ${tool}`);
    }

    return { success: true, output };
  } catch (error: any) {
    return { success: false, output: '', error: error.message };
  }
}

/**
 * Validate with guardrails (same logic as native PlausibilityGuard)
 */
function validateOutput(output: string): { accept: boolean; reason?: string } {
  if (output.trim().length === 0) {
    return { accept: false, reason: 'Empty output' };
  }

  // Check for metadata-only (e.g., "total 123")
  const trimmed = output.trim();
  if (trimmed.split('\n').length === 1) {
    const parts = trimmed.split(/\s+/);
    if (parts.length === 2 && parts[0].toLowerCase() === 'total' && /^\d+$/.test(parts[1])) {
      return { accept: false, reason: 'Metadata-only output (total line)' };
    }
  }

  if (output.length < 3 || !/[a-zA-Z0-9]/.test(output)) {
    return { accept: false, reason: 'Lacks substantive content' };
  }

  return { accept: true };
}

/**
 * Display output
 */
function render(html: string): void {
  const out = document.getElementById('output-content')!;
  out.innerHTML += html;
  document.getElementById('output')!.classList.remove('hidden');
}

/**
 * Detect if query requires environmental data (heuristic)
 * Used to enforce tool obligations
 */
function requiresEnvironmentalData(query: string): boolean {
  const lower = query.toLowerCase();

  // Keywords indicating DOM/page content access
  const domKeywords = ['page', 'title', 'heading', 'text', 'element', 'dom', 'html', 'extract', 'read', 'get', 'show', 'display'];

  // Keywords indicating network access
  const networkKeywords = ['fetch', 'url', 'http', 'api', 'request', 'download', 'load'];

  return domKeywords.some(kw => lower.includes(kw)) || networkKeywords.some(kw => lower.includes(kw));
}

/**
 * Run agent (simplified, single-pass)
 */
async function runAgent(query: string): Promise<void> {
  if (!wasm || !engine) throw new Error('Not initialized');

  render('<div class="status info">Starting...</div>');

  // Create agent state
  let stateJson = wasm.create_agent_state(query);

  // System prompt with tool obligations
  const systemPrompt = `You are an agent with tools:
- read_dom: {"tool":"read_dom","selector":"h1"}
- fetch_url: {"tool":"fetch_url","url":"https://httpbin.org/json"}

CRITICAL RULES:
1. If a task requires information from the ENVIRONMENT (DOM content, external URLs), you MUST invoke a tool.
2. If a task can be answered with your own knowledge (math, general facts), answer DIRECTLY without tools.
3. Do NOT explain what tool should be used.
4. Do NOT describe how to solve the task.

Examples:
User: Extract the page title
Assistant: {"tool":"read_dom","selector":"title"}

User: Fetch data from https://httpbin.org/json
Assistant: {"tool":"fetch_url","url":"https://httpbin.org/json"}

User: What is 2 + 2?
Assistant: 4

User: What is the capital of France?
Assistant: Paris

Respond with JSON to use a tool, or plain text to answer directly.`;

  const userPrompt = `${systemPrompt}\n\nUser: ${query}\n\nAssistant:`;

  // Get LLM response
  render('<div class="status info">Calling WebLLM...</div>');
  const response = await engine.chat.completions.create({
    messages: [{ role: 'user', content: userPrompt }],
    temperature: 0.7,
    max_tokens: 256
  });

  const modelOutput = response.choices[0].message.content;
  render(`<div class="tool-call"><span class="label">Model:</span> ${modelOutput}</div>`);

  // Process with WASM
  const stepInput = { state_json: stateJson, model_output: modelOutput };
  const stepOutput = JSON.parse(wasm.run_agent_step(JSON.stringify(stepInput)));
  const decision: AgentDecision = stepOutput.decision;

  if (decision.type === 'invoke_tool') {
    render(`<div class="tool-call"><span class="label">→ Tool:</span> ${decision.tool}</div>`);

    // Execute tool
    const result = await executeTool(decision.tool, decision.params);

    if (result.success) {
      render(`<div class="tool-call"><span class="label">Output:</span> ${result.output.substring(0, 300)}</div>`);

      // Validate with guardrail
      const validation = validateOutput(result.output);

      if (validation.accept) {
        render('<div class="guardrail-result accept">✓ Guardrail: Accept</div>');
        render(`<div class="final-answer"><h3>Result</h3>${result.output}</div>`);
      } else {
        render(`<div class="guardrail-result reject">✗ Guardrail: Reject - ${validation.reason}</div>`);
        render(`<div class="status error">
          <strong>❌ TASK FAILED</strong><br><br>
          Tool executed successfully, but output was semantically invalid.<br>
          Guardrail rejection: ${validation.reason}<br><br>
          This model cannot complete this task correctly.<br>
          <strong>The system refused to return incorrect results (by design).</strong>
        </div>`);
      }
    } else {
      render(`<div class="status error">Tool error: ${result.error}</div>`);
    }

  } else if (decision.type === 'done') {
    // Check if this task required a tool but none was invoked
    if (requiresEnvironmentalData(query)) {
      // Task required environmental data but model didn't invoke a tool
      render(`<div class="status error">
        <strong>❌ TASK FAILED</strong><br><br>
        The model did not invoke a required tool.<br>
        This task cannot be completed correctly without accessing the environment.<br><br>
        Model response: "${decision.answer.substring(0, 200)}"<br><br>
        <strong>The system refused to return an incorrect result (by design).</strong>
      </div>`);
    } else {
      // Task doesn't require environmental data - accept answer
      render(`<div class="final-answer"><h3>Final Answer</h3>${decision.answer}</div>`);
    }

  } else {
    render(`<div class="status error">Model produced inconclusive output. Task cannot be completed.</div>`);
  }
}

/**
 * Initialize
 */
async function init(): Promise<void> {
  const input = document.getElementById('query') as HTMLInputElement;
  const btn = document.getElementById('run-btn') as HTMLButtonElement;

  try {
    await loadWASM();
    await initWebLLM();

    input.disabled = false;
    btn.disabled = false;

    btn.onclick = async () => {
      const query = input.value.trim();
      if (!query) return;

      btn.disabled = true;
      input.disabled = true;

      try {
        document.getElementById('output-content')!.innerHTML = '';
        await runAgent(query);
      } catch (error: any) {
        render(`<div class="status error">Error: ${error.message}</div>`);
      } finally {
        btn.disabled = false;
        input.disabled = false;
      }
    };
  } catch (error: any) {
    document.getElementById('model-status')!.textContent = `Init failed: ${error.message}`;
    document.getElementById('model-status')!.className = 'status error';
  }
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
