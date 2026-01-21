//! # agent-core
//!
//! Pure Rust agent logic with no dependencies on OS, FFI, or specific LLM implementations.
//!
//! This crate provides the core agent loop semantics:
//! - Parse model output to detect tool calls, skill invocations, or final answers
//! - Manage agent state and conversation history
//! - Make deterministic decisions about next actions
//! - Enforce semantic guardrails on outputs
//!
//! This crate compiles to `wasm32-unknown-unknown` without any feature flags.

#![forbid(unsafe_code)]

pub mod agent;
pub mod guardrail;
pub mod protocol;
pub mod skill;
pub mod skill_manifest;
pub mod tool;

// Re-export commonly used types
pub use agent::{AgentDecision, AgentState, Message, Role};
pub use guardrail::{
    GuardrailChain, GuardrailContext, GuardrailResult, PlausibilityGuard, SemanticGuardrail,
};
pub use protocol::{parse_model_output, ParseResult};
pub use skill::{
    is_valid_skill, parse_skill_output, validate_extraction_output, ExtractionInput,
    ExtractionOutput, ExtractionTarget, SkillError, SkillMetadata, SkillRequest, SkillResult,
    AVAILABLE_SKILLS, EXTRACTION_SKILL,
};
pub use tool::{ToolRequest, ToolResult};
