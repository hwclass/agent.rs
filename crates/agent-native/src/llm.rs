//! LLM backend abstraction
//!
//! This module defines the interface between the host runtime and LLM inference engines.
//! The agent core never depends on this - it only sees text input/output.

use anyhow::Result;

/// Input to an LLM inference call
#[derive(Debug, Clone)]
pub struct LLMInput {
    /// The full prompt text to process
    pub prompt: String,

    /// Maximum number of tokens to generate
    pub max_tokens: usize,

    /// Current position in the KV cache (for append-only context)
    pub current_pos: i32,

    /// Whether this is the first generation (may require special handling like stderr suppression)
    pub first_generation: bool,
}

/// Output from an LLM inference call
#[derive(Debug, Clone)]
pub struct LLMOutput {
    /// The generated text
    pub text: String,

    /// Total tokens processed (prompt + generated)
    pub tokens_processed: i32,
}

/// Host-side LLM backend interface
///
/// Implementors provide actual inference capabilities.
/// The agent core never sees this trait - it remains environment-agnostic.
pub trait LLMBackend {
    /// Perform inference on the given input
    fn infer(&mut self, input: LLMInput) -> Result<LLMOutput>;
}
