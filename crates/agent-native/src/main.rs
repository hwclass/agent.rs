use agent_core::{
    agent::{apply_tool_result, process_model_output, AgentDecision, AgentState, Role},
    tool::{ToolRequest, ToolResult},
};
use anyhow::{Context, Result};
use clap::Parser;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::model::{AddBos, Special};
use llama_cpp_2::token::data_array::LlamaTokenDataArray;
use std::io::{self, Write};
use std::num::NonZeroU32;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::process::Command;

const SYSTEM_PROMPT: &str = r#"You are a helpful AI agent with access to tools.

Available tools:
- shell: Execute shell commands

To invoke a tool, respond with JSON in this exact format:
{"tool": "shell", "command": "your command here"}

IMPORTANT:
- Only output JSON when you want to invoke a tool
- For final answers, respond in plain text (no JSON)
- Be concise and helpful

Example tool invocation:
{"tool": "shell", "command": "ls -la"}

Example final answer:
The directory contains 5 files including README.md and src/."#;

const TOOL_RESPONSE_SCHEMA: &str = r#"When responding after tool usage:
- First provide an OBSERVATIONS section containing factual information derived directly from tool output.
- Then provide a FINAL ANSWER section that directly answers the user request.

Both sections are required."#;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the GGUF model file
    #[arg(short, long)]
    model: PathBuf,

    /// The user query to process
    #[arg(short, long)]
    query: String,

    /// Maximum number of agent loop iterations
    #[arg(short = 'i', long, default_value = "5")]
    max_iterations: usize,

    /// Number of tokens to generate per iteration
    #[arg(short = 'n', long, default_value = "256")]
    max_tokens: usize,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("=== agent.rs ===");
    println!("Query: {}\n", args.query);

    // Initialize llama.cpp backend
    let backend = LlamaBackend::init()?;

    // Load model
    let model_params = LlamaModelParams::default();
    let model = LlamaModel::load_from_file(&backend, &args.model, &model_params)
        .context("Failed to load model")?;

    // Create context (Metal logs will appear during first inference, not here)
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(2048)); // API expects Option<NonZeroU32>

    let mut ctx = model
        .new_context(&backend, ctx_params)
        .context("Failed to create context")?;

    // Initialize agent state
    let mut state = AgentState::new(&args.query);
    let mut iteration = 0;
    let mut current_pos: i32 = 0; // Track KV cache position
    let mut tool_used = false; // Track if any tool has been invoked
    let mut first_generation = true; // Track first decode (Metal shader compilation)

    // Agent loop
    while iteration < args.max_iterations {
        iteration += 1;

        // Lifecycle callback: before_llm_call
        let prompt = before_llm_call(&state, tool_used);

        let (response, tokens_generated) = generate(&model, &mut ctx, &prompt, args.max_tokens, current_pos, first_generation)?;
        current_pos += tokens_generated;
        first_generation = false;

        // Process the output
        match process_model_output(&mut state, response) {
            AgentDecision::InvokeTool(tool_request) => {
                // Execute tool
                let result = execute_tool(&tool_request)?;

                // Apply result to state
                apply_tool_result(&mut state, &result);

                // Lifecycle callback: after_tool_execution
                after_tool_execution(&mut state, &result);
                tool_used = true;
            }
            AgentDecision::Done(answer) => {
                println!("\n{}", answer);
                return Ok(());
            }
        }
    }

    println!("\nWarning: Agent reached maximum iterations without completing.");
    Ok(())
}

/// Lifecycle callback: before_llm_call
/// Constructs the prompt and injects response schema if tools have been used
fn before_llm_call(state: &AgentState, tool_used: bool) -> String {
    let mut prompt = String::new();

    // Add system prompt
    prompt.push_str(SYSTEM_PROMPT);
    prompt.push_str("\n\n");

    // Add conversation history
    for msg in &state.history {
        match msg.role {
            Role::User => {
                prompt.push_str("User: ");
                prompt.push_str(&msg.content);
                prompt.push_str("\n\n");
            }
            Role::Assistant => {
                prompt.push_str("Assistant: ");
                prompt.push_str(&msg.content);
                prompt.push_str("\n\n");
            }
            Role::Tool => {
                prompt.push_str(&msg.content);
                prompt.push_str("\n\n");
            }
        }
    }

    // Inject response schema if at least one tool has been used
    if tool_used {
        prompt.push_str(TOOL_RESPONSE_SCHEMA);
        prompt.push_str("\n\n");
    }

    prompt.push_str("Assistant: ");
    prompt
}

/// Lifecycle callback: after_tool_execution
/// Logs tool execution details and validates result
fn after_tool_execution(_state: &mut AgentState, tool_result: &ToolResult) {
    // Silent - tool result is already in state history
    // Future: add logging, metrics, validation here
    let _ = tool_result; // Suppress unused warning
}

/// Generate text from the model
fn generate(
    model: &LlamaModel,
    ctx: &mut llama_cpp_2::context::LlamaContext,
    prompt: &str,
    max_tokens: usize,
    current_pos: i32,
    suppress_stderr: bool,
) -> Result<(String, i32)> {
    // Suppress stderr during first decode (Metal shader compilation logs)
    let _stderr_redirect = if suppress_stderr {
        Some(suppress_stderr_temporarily())
    } else {
        None
    };

    // Tokenize prompt
    let tokens = model
        .str_to_token(prompt, AddBos::Always)
        .context("Failed to tokenize prompt")?;

    // Create batch with size based on prompt length + generation headroom
    let batch_size = (tokens.len() + max_tokens).max(512);
    let mut batch = LlamaBatch::new(batch_size, 1);
    for (i, token) in tokens.iter().enumerate() {
        let is_last = i == tokens.len() - 1;
        batch.add(*token, current_pos + i as i32, &[0], is_last)?;
    }

    // Decode the prompt
    ctx.decode(&mut batch)
        .context("Failed to decode batch")?;

    // Generate tokens
    let mut result = String::new();
    let mut n_generated = 0;
    let prompt_len = tokens.len() as i32;

    while n_generated < max_tokens {
        // Get token candidates and sample greedily
        let candidates = ctx.candidates();
        let mut candidates_array = LlamaTokenDataArray::from_iter(candidates, false);

        // Select token with highest probability (greedy sampling)
        candidates_array.sample_token_greedy();
        let token = match candidates_array.selected_token() {
            Some(t) => t,
            None => break, // No token selected, end generation
        };

        // Check for EOS
        if model.is_eog_token(token) {
            break;
        }

        // Decode token
        if let Ok(piece) = model.token_to_str(token, Special::Tokenize) {
            result.push_str(&piece);
        }

        // Prepare next batch
        batch.clear();
        batch.add(token, current_pos + prompt_len + n_generated as i32, &[0], true)?;

        ctx.decode(&mut batch)
            .context("Failed to decode batch")?;

        n_generated += 1;

        // Early stopping heuristics
        if result.trim().starts_with('{') {
            // For JSON tool calls: stop when we have valid complete JSON
            if result.contains('}') {
                if let Ok(_) = serde_json::from_str::<serde_json::Value>(result.trim()) {
                    break;
                }
            }
        } else {
            // For text responses: stop when we see natural ending patterns
            // Check for double newline after sentence (paragraph break)
            if result.contains("\n\n") && (result.trim_end().ends_with('.') || result.trim_end().ends_with('!') || result.trim_end().ends_with('?')) {
                break;
            }
        }
    }

    // Return generated text and total tokens processed (prompt + generated)
    Ok((result.trim().to_string(), prompt_len + n_generated as i32))
}

/// Execute a tool request
fn execute_tool(request: &ToolRequest) -> Result<ToolResult> {
    match request.tool.as_str() {
        "shell" => execute_shell_tool(request),
        _ => Ok(ToolResult::failure(format!(
            "Unknown tool: {}",
            request.tool
        ))),
    }
}

/// Execute the shell tool with human approval
fn execute_shell_tool(request: &ToolRequest) -> Result<ToolResult> {
    // Extract command from params
    let command = request
        .params
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'command' parameter"))?;

    println!("\n→ shell: {}", command);
    print!("  Execute? (y/n): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !input.trim().eq_ignore_ascii_case("y") {
        println!("  ✗ Rejected\n");
        return Ok(ToolResult::failure("Command rejected by user"));
    }
    let output = Command::new("sh").arg("-c").arg(command).output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        let result = stdout.to_string();

        // Always show output section, even if empty
        if !result.is_empty() {
            println!("\n{}", result);
        } else {
            println!("  (no output)\n");
        }

        // Send to model (empty output is valid)
        Ok(ToolResult::success(result))
    } else {
        let error = if !stderr.is_empty() {
            stderr.to_string()
        } else {
            format!("Command exited with status {}", output.status)
        };

        println!("  ✗ {}\n", error);
        Ok(ToolResult::failure(error))
    }
}

/// Temporarily suppress stderr (for Metal shader compilation logs)
fn suppress_stderr_temporarily() -> impl Drop {
    use std::fs::OpenOptions;
    use std::os::unix::io::FromRawFd;

    struct StderrRedirect {
        old_stderr: i32,
    }

    impl Drop for StderrRedirect {
        fn drop(&mut self) {
            unsafe {
                libc::dup2(self.old_stderr, 2);
                libc::close(self.old_stderr);
            }
        }
    }

    unsafe {
        let old_stderr = libc::dup(2);
        let devnull = OpenOptions::new()
            .write(true)
            .open("/dev/null")
            .expect("Failed to open /dev/null");
        libc::dup2(devnull.as_raw_fd(), 2);

        StderrRedirect { old_stderr }
    }
}
