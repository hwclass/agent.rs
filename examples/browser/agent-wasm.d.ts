// Type declarations for agent-wasm module
// The actual module is loaded from public/ at runtime via Vite

declare module '/public/agent-wasm/agent_wasm.js' {
  export default function init(): Promise<void>;
  export function create_agent_state(query: string): string;
  export function run_agent_step(inputJson: string): string;
}
