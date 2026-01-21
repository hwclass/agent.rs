mod llama_cpp_backend;
mod llm;
mod skill_discovery;

use agent_core::{
    agent::{apply_tool_result, process_model_output, AgentDecision, AgentState, Role},
    guardrail::{GuardrailChain, GuardrailContext, GuardrailResult, PlausibilityGuard},
    skill::{
        parse_skill_output, validate_extraction_output, ExtractionInput, ExtractionTarget,
        SkillError, SkillRequest, SkillResult_,
    },
    tool::{ToolRequest, ToolResult},
};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use llama_cpp_backend::LlamaCppBackend;
use llm::{LLMBackend, LLMInput};
use serde_json::json;
use skill_discovery::{build_available_skills_prompt, discover_skills};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

const BASE_SYSTEM_PROMPT: &str = r#"You are a helpful AI agent with access to tools and skills.

Available tools:
- shell: Execute shell commands

Available skills:
- extract: Extract structured information from text (email, url, date, entity, name)

To invoke a tool, respond with JSON:
{"tool": "shell", "command": "your command here"}

To invoke a skill, respond with JSON:
{"skill": "extract", "text": "the text to extract from", "target": "email"}

Supported extraction targets: email, url, date, entity, name

IMPORTANT:
- Only output JSON when you want to invoke a tool or skill
- For final answers, respond in plain text (no JSON)
- Be concise and helpful

Example tool invocation:
{"tool": "shell", "command": "ls -la"}

Example skill invocation:
{"skill": "extract", "text": "Contact us at hello@agent.rs", "target": "email"}

Example final answer:
The directory contains 5 files including README.md and src/."#;

const TOOL_RESPONSE_SCHEMA: &str = r#"When responding after tool usage:
- First provide an OBSERVATIONS section containing factual information derived directly from tool output.
- Then provide a FINAL ANSWER section that directly answers the user request.

Both sections are required."#;

const DEFAULT_MODEL_PATH: &str = "./granite-4.0-micro-Q8_0.gguf";

fn build_system_prompt(available_skills_prompt: &str) -> String {
    let mut prompt = String::new();
    prompt.push_str(BASE_SYSTEM_PROMPT);
    if !available_skills_prompt.trim().is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(available_skills_prompt);
    }
    prompt
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Use `extract` to call the skill directly, or omit for full agent loop
    #[command(subcommand)]
    command: Option<CliCommand>,

    /// Path to the GGUF model file (agent mode)
    #[arg(short, long)]
    model: Option<PathBuf>,

    /// The user query to process (agent mode)
    #[arg(short, long)]
    query: Option<String>,

    /// Maximum number of agent loop iterations
    #[arg(short = 'i', long, default_value = "5")]
    max_iterations: usize,

    /// Number of tokens to generate per iteration
    #[arg(short = 'n', long, default_value = "256")]
    max_tokens: usize,
}

#[derive(Subcommand, Debug)]
enum CliCommand {
    /// Invoke the extraction skill directly (bypasses agent loop)
    Extract {
        /// Text to extract from
        #[arg(long)]
        text: String,
        /// Target to extract (email, url, date, entity)
        #[arg(short, long, value_parser = parse_target)]
        target: ExtractionTarget,
        /// Path to the GGUF model file
        #[arg(short, long)]
        model: Option<PathBuf>,
        /// Number of tokens to generate
        #[arg(short = 'n', long, default_value = "256")]
        max_tokens: usize,
    },
    /// Invoke a specific skill explicitly (extensible for future skills)
    Skill {
        #[command(subcommand)]
        command: SkillCommand,
    },
}

#[derive(Subcommand, Debug)]
enum SkillCommand {
    /// Extract structured data from text
    Extract {
        /// Text to extract from
        #[arg(long)]
        text: String,
        /// Target to extract (email, url, date, entity)
        #[arg(short, long, value_parser = parse_target)]
        target: ExtractionTarget,
        /// Path to the GGUF model file
        #[arg(short, long)]
        model: Option<PathBuf>,
        /// Number of tokens to generate
        #[arg(short = 'n', long, default_value = "256")]
        max_tokens: usize,
    },
}

#[derive(Debug)]
struct AgentArgs {
    model: PathBuf,
    query: String,
    max_iterations: usize,
    max_tokens: usize,
}

fn parse_target(value: &str) -> Result<ExtractionTarget, String> {
    ExtractionTarget::from_str(value).ok_or_else(|| {
        format!(
            "Invalid target '{}'. Expected one of: email, url, date, entity",
            value
        )
    })
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Some(CliCommand::Extract {
            text,
            target,
            model,
            max_tokens,
        }) => {
            let model_path = model
                .clone()
                .unwrap_or_else(|| PathBuf::from(DEFAULT_MODEL_PATH));
            run_extract_mode(text, *target, model_path, *max_tokens)
        }
        Some(CliCommand::Skill { command }) => match command {
            SkillCommand::Extract {
                text,
                target,
                model,
                max_tokens,
            } => {
                let model_path = model
                    .clone()
                    .unwrap_or_else(|| PathBuf::from(DEFAULT_MODEL_PATH));
                run_extract_mode(text, *target, model_path, *max_tokens)
            }
        },
        None => {
            let model = cli
                .model
                .clone()
                .unwrap_or_else(|| PathBuf::from(DEFAULT_MODEL_PATH));
            let query = cli
                .query
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Missing required --query argument"))?;

            let args = AgentArgs {
                model,
                query,
                max_iterations: cli.max_iterations,
                max_tokens: cli.max_tokens,
            };

            let discovered_skills = discover_skills(&[PathBuf::from("skills")]);
            let available_skills_prompt = build_available_skills_prompt(&discovered_skills);
            let system_prompt = build_system_prompt(&available_skills_prompt);

            run_agent(args, system_prompt)
        }
    }
}

fn run_agent(args: AgentArgs, system_prompt: String) -> Result<()> {
    println!("=== agent.rs ===");
    println!("Query: {}\n", args.query);

    // Initialize LLM backend (llama.cpp in this case)
    let mut llm_backend =
        LlamaCppBackend::new(&args.model).context("Failed to initialize LLM backend")?;

    // Initialize semantic guardrail chain
    let guardrail_chain = GuardrailChain::new().add(Box::new(PlausibilityGuard::new()));

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
        let prompt = before_llm_call(&state, tool_used, false, &system_prompt);

        // Call LLM backend
        let llm_output = llm_backend.infer(LLMInput {
            prompt,
            max_tokens: args.max_tokens,
            current_pos,
            first_generation,
        })?;

        current_pos += llm_output.tokens_processed;
        first_generation = false;

        // Process the output
        match process_model_output(&mut state, llm_output.text) {
            AgentDecision::InvokeSkill(skill_request) => {
                // Execute skill
                let result = execute_skill(
                    &skill_request,
                    &mut llm_backend,
                    args.max_tokens,
                    &mut current_pos,
                )?;

                if result.success {
                    // Apply result to state
                    state.add_message(Role::Tool, format!("Skill output:\n{}", result.to_json()));
                    println!("\n✓ Skill result: {}", result.to_json());
                } else {
                    // Skill failed - add error to state
                    let error_msg = result.error.as_deref().unwrap_or("unknown error");
                    state.add_message(Role::Tool, format!("Skill failed: {}", error_msg));
                    eprintln!("\n✗ Skill error: {}", error_msg);
                }
            }
            AgentDecision::InvokeTool(tool_request) => {
                // Execute tool
                let result = execute_tool(&tool_request)?;

                // Validate tool output with semantic guardrails
                let guard_ctx = GuardrailContext {
                    state: &state,
                    tool_request: &tool_request,
                    tool_result: &result,
                };

                match guardrail_chain.validate(&guard_ctx) {
                    GuardrailResult::Accept => {
                        // Apply result to state
                        apply_tool_result(&mut state, &result);

                        // Lifecycle callback: after_tool_execution
                        after_tool_execution(&mut state, &result);
                        tool_used = true;
                    }
                    GuardrailResult::Reject { reason } => {
                        // Guardrail rejected output - treat as inconclusive
                        eprintln!("\n⚠️  Guardrail rejected tool output:");
                        eprintln!("   {}", reason);
                        eprintln!("\n   Attempting corrective retry...\n");

                        // Corrective retry with stricter instructions
                        let corrective_prompt =
                            before_llm_call(&state, tool_used, true, &system_prompt);

                        let retry_output = llm_backend.infer(LLMInput {
                            prompt: corrective_prompt,
                            max_tokens: args.max_tokens,
                            current_pos,
                            first_generation: false,
                        })?;

                        current_pos += retry_output.tokens_processed;

                        // Process retry output
                        match process_model_output(&mut state, retry_output.text) {
                            AgentDecision::InvokeSkill(skill_request) => {
                                // Execute skill on retry
                                let result = execute_skill(
                                    &skill_request,
                                    &mut llm_backend,
                                    args.max_tokens,
                                    &mut current_pos,
                                )?;
                                if result.success {
                                    state.add_message(
                                        Role::Tool,
                                        format!("Skill output:\n{}", result.to_json()),
                                    );
                                } else {
                                    let error_msg =
                                        result.error.as_deref().unwrap_or("unknown error");
                                    state.add_message(
                                        Role::Tool,
                                        format!("Skill failed: {}", error_msg),
                                    );
                                }
                            }
                            AgentDecision::InvokeTool(retry_request) => {
                                // Execute retry
                                let retry_result = execute_tool(&retry_request)?;

                                // Validate retry output
                                let retry_guard_ctx = GuardrailContext {
                                    state: &state,
                                    tool_request: &retry_request,
                                    tool_result: &retry_result,
                                };

                                match guardrail_chain.validate(&retry_guard_ctx) {
                                    GuardrailResult::Accept => {
                                        // Success - apply result
                                        apply_tool_result(&mut state, &retry_result);
                                        after_tool_execution(&mut state, &retry_result);
                                        tool_used = true;
                                    }
                                    GuardrailResult::Reject {
                                        reason: retry_reason,
                                    } => {
                                        report_guardrail_failure(&reason, &retry_reason);
                                    }
                                }
                            }
                            AgentDecision::Done(answer) => {
                                println!("\n{}", answer);
                                return Ok(());
                            }
                            AgentDecision::Inconclusive(retry_output) => {
                                report_inconclusive_after_guardrail_failure(&reason, &retry_output);
                            }
                        }
                    }
                }
            }
            AgentDecision::Done(answer) => {
                println!("\n{}", answer);
                return Ok(());
            }
            AgentDecision::Inconclusive(output) => {
                // Model failed to produce a tool call or complete the task
                eprintln!("\n⚠️  Model produced inconclusive output:");
                eprintln!("   \"{}\"", output.lines().next().unwrap_or(&output));
                eprintln!("\n   Attempting corrective retry with stricter instructions...\n");

                // Corrective retry: re-prompt with explicit tool requirement
                let corrective_prompt = before_llm_call(&state, tool_used, true, &system_prompt);

                let retry_output = llm_backend.infer(LLMInput {
                    prompt: corrective_prompt,
                    max_tokens: args.max_tokens,
                    current_pos,
                    first_generation: false,
                })?;

                current_pos += retry_output.tokens_processed;

                // Process retry output
                match process_model_output(&mut state, retry_output.text) {
                    AgentDecision::InvokeSkill(skill_request) => {
                        // Success - execute skill
                        let result = execute_skill(
                            &skill_request,
                            &mut llm_backend,
                            args.max_tokens,
                            &mut current_pos,
                        )?;
                        if result.success {
                            state.add_message(
                                Role::Tool,
                                format!("Skill output:\n{}", result.to_json()),
                            );
                        } else {
                            let error_msg = result.error.as_deref().unwrap_or("unknown error");
                            state.add_message(Role::Tool, format!("Skill failed: {}", error_msg));
                        }
                    }
                    AgentDecision::InvokeTool(tool_request) => {
                        // Success - execute tool
                        let result = execute_tool(&tool_request)?;
                        apply_tool_result(&mut state, &result);
                        after_tool_execution(&mut state, &result);
                        tool_used = true;
                    }
                    AgentDecision::Done(answer) => {
                        println!("\n{}", answer);
                        return Ok(());
                    }
                    AgentDecision::Inconclusive(retry_output) => {
                        // Still inconclusive after retry - fail loudly
                        eprintln!(
                            "\n❌ ERROR: Model failed to produce a valid response after retry.\n"
                        );
                        eprintln!(
                            "Original output: \"{}\"",
                            output.lines().next().unwrap_or(&output)
                        );
                        eprintln!(
                            "Retry output:    \"{}\"",
                            retry_output.lines().next().unwrap_or(&retry_output)
                        );
                        eprintln!(
                            "\nThe model did not invoke a tool/skill or provide a complete answer."
                        );
                        eprintln!("This is common with small models (3-4B parameters).");
                        eprintln!("\nSuggestions:");
                        eprintln!("  - Use a larger model (7B+ parameters)");
                        eprintln!("  - Use a model specifically tuned for tool use");
                        eprintln!("  - Simplify the query");

                        std::process::exit(1);
                    }
                }
            }
        }
    }

    eprintln!("\n⚠️  Warning: Agent reached maximum iterations without completing.");
    std::process::exit(1)
}

fn run_extract_mode(
    text: &str,
    target: ExtractionTarget,
    model: PathBuf,
    max_tokens: usize,
) -> Result<()> {
    println!("=== agent.rs | extract ===");
    println!("Model: {}", model.display());
    println!("Target: {}", target.as_str());
    println!("Text: \"{}\"\n", truncate_string(text, 80));

    let mut llm_backend =
        LlamaCppBackend::new(&model).context("Failed to initialize LLM backend")?;

    let mut current_pos: i32 = 0;
    let request = SkillRequest::new(
        "extract",
        json!({
            "text": text,
            "target": target.as_str()
        }),
    );

    let result =
        execute_extraction_skill(&request, &mut llm_backend, max_tokens, &mut current_pos)?;

    if result.success {
        println!("{}", result.to_json());
        Ok(())
    } else {
        let msg = result
            .error
            .clone()
            .unwrap_or_else(|| "unknown error".to_string());
        Err(anyhow::anyhow!(msg))
    }
}

/// Lifecycle callback: before_llm_call
/// Constructs the prompt and injects response schema if tools have been used
/// If `corrective` is true, adds stricter instructions for tool invocation
fn before_llm_call(
    state: &AgentState,
    tool_used: bool,
    corrective: bool,
    system_prompt: &str,
) -> String {
    let mut prompt = String::new();

    // Add system prompt
    prompt.push_str(system_prompt);
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

    // Add corrective instruction if this is a retry
    // This prompt addresses common LLM failures: reasoning instead of action,
    // and generating commands that produce unusable outputs (headers, summaries).
    if corrective {
        prompt.push_str("CRITICAL: You MUST call a tool to complete this task.\n");
        prompt.push_str("Respond ONLY with valid JSON in the exact format shown above.\n");
        prompt.push_str(
            "Do NOT explain what you will do. Do NOT use plain text. Output JSON only.\n\n",
        );

        prompt.push_str("IMPORTANT: The tool command must directly produce the final answer.\n");
        prompt.push_str("Avoid commands that output headers, summaries, or non-answer lines.\n");
        prompt.push_str(
            "The tool output should be the actual data requested, not metadata about it.\n\n",
        );

        // NOTE: Semantic guardrails validate tool outputs at runtime.
        // TODO: Future enhancement - Tool-defined postconditions
        //
        // Tools should optionally declare explicit semantic contracts (postconditions)
        // that replace heuristic guardrails. This aligns with agent.cpp's callback
        // extensibility and any-guardrail's pluggable validation model.
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

/// Report guardrail failure to user with structured output
///
/// Event: AgentFailedAfterGuardrails
/// Triggered when the agent fails after guardrails reject both initial and retry attempts.
fn report_guardrail_failure(initial_reason: &str, retry_reason: &str) -> ! {
    let message = format!(
        r#"
❌ TASK FAILED: Agent could not produce valid output

What happened:
  • The agent attempted to complete your task
  • Tool commands were executed successfully
  • However, the tool outputs were semantically invalid
  • A corrective retry was attempted
  • The retry also produced invalid output

Validation failures:
  Initial attempt: {}
  Retry attempt:   {}

Why this happened:
  This model lacks sufficient tool-reasoning capability for this task.
  The system refused to return incorrect results (this is by design).

What you can do:
  • Use a larger model (7B+ parameters recommended)
  • Use a model specifically fine-tuned for tool use
  • Simplify the query to reduce reasoning complexity
  • Verify the task is achievable with available tools

Note: A correct system that fails honestly is better than one that
      returns plausible-looking but incorrect results.
"#,
        initial_reason, retry_reason
    );

    eprintln!("{}", message);
    std::process::exit(1);
}

/// Report model failure to produce tool call after guardrail rejection
fn report_inconclusive_after_guardrail_failure(guardrail_reason: &str, model_output: &str) -> ! {
    let message = format!(
        r#"
❌ TASK FAILED: Model could not recover from validation failure

What happened:
  • A tool was executed but its output was rejected by validation
  • Guardrail rejection: {}
  • A corrective retry was attempted
  • The model failed to produce a valid tool call
  • Model output: "{}"

Why this happened:
  The model cannot adjust its approach in response to validation feedback.
  This indicates insufficient tool-reasoning capability.

What you can do:
  • Use a larger model (7B+ parameters recommended)
  • Use a model specifically fine-tuned for tool use
  • Simplify the query
"#,
        guardrail_reason,
        model_output.lines().next().unwrap_or(model_output)
    );

    eprintln!("{}", message);
    std::process::exit(1);
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

/// Execute a skill request
///
/// Skills are contract-based operations with built-in guardrails.
/// The host executes the skill by:
/// 1. Validating input
/// 2. Calling LLM with extraction prompt
/// 3. Validating output against schema and anti-hallucination rules
fn execute_skill(
    request: &SkillRequest,
    llm_backend: &mut LlamaCppBackend,
    max_tokens: usize,
    current_pos: &mut i32,
) -> Result<SkillResult_> {
    match request.skill.as_str() {
        "extract" => execute_extraction_skill(request, llm_backend, max_tokens, current_pos),
        _ => Ok(SkillResult_::failure(SkillError::UnknownSkill(
            request.skill.clone(),
        ))),
    }
}

/// Execute the extraction skill
fn execute_extraction_skill(
    request: &SkillRequest,
    llm_backend: &mut LlamaCppBackend,
    max_tokens: usize,
    current_pos: &mut i32,
) -> Result<SkillResult_> {
    // Parse and validate input
    let input = match request.parse_extraction_input() {
        Ok(input) => input,
        Err(e) => return Ok(SkillResult_::failure(e)),
    };

    let target = match input.validate() {
        Ok(target) => target,
        Err(e) => return Ok(SkillResult_::failure(e)),
    };

    println!("\n→ skill: extract (target: {})", target.as_str());
    println!("  Text: \"{}\"", truncate_string(&input.text, 50));

    // Build extraction prompt
    let extraction_prompt = build_extraction_prompt(&input, target);

    // Call LLM
    let llm_output = llm_backend.infer(LLMInput {
        prompt: extraction_prompt,
        max_tokens,
        current_pos: *current_pos,
        first_generation: false,
    })?;

    *current_pos += llm_output.tokens_processed;

    // Parse LLM output
    let output = match parse_skill_output(&llm_output.text, target) {
        Ok(output) => output,
        Err(e) => {
            eprintln!("  ✗ {}", e);
            return Ok(SkillResult_::failure(e));
        }
    };

    // Validate output (anti-hallucination)
    if let Err(e) = validate_extraction_output(&input, &output, target) {
        eprintln!("  ✗ {}", e);
        return Ok(SkillResult_::failure(e));
    }

    // Success
    Ok(SkillResult_::success(output.result))
}

/// Build prompt for extraction skill
fn build_extraction_prompt(input: &ExtractionInput, target: ExtractionTarget) -> String {
    let target_desc = match target {
        ExtractionTarget::Email => "email addresses",
        ExtractionTarget::Url => "URLs",
        ExtractionTarget::Date => "dates (in ISO format YYYY-MM-DD)",
        ExtractionTarget::Entity => "named entities (people, organizations, locations)",
        ExtractionTarget::Name => "person names (first name, last name, full names)",
    };

    let output_format = match target {
        ExtractionTarget::Entity => {
            r#"{"entity": {"people": [...], "organizations": [...], "locations": [...]}}"#
        }
        _ => &format!(r#"{{"{}": [...]}}"#, target.as_str()),
    };

    format!(
        r#"Extract {target_desc} from the following text.

IMPORTANT:
- Output ONLY valid JSON
- Only include values that ACTUALLY APPEAR in the text
- Do NOT invent or hallucinate values
- If no matches found, return an empty array

Text: "{text}"

Output format: {output_format}

JSON output:"#,
        target_desc = target_desc,
        text = input.text,
        output_format = output_format
    )
}

/// Truncate string for display
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}
