//! Semantic Guardrail Interface
//!
//! Inspired by Mozilla.ai's any-guardrail pattern and agent.cpp's lifecycle hooks.
//! Provides pluggable validation of tool outputs to prevent false-positive success.
//!
//! This is NOT safety moderation - it's correctness validation.

use crate::agent::AgentState;
use crate::tool::{ToolRequest, ToolResult};

/// Result of guardrail validation
#[derive(Debug, Clone)]
pub enum GuardrailResult {
    /// Output is plausible and can be accepted
    Accept,
    /// Output is invalid and should be rejected
    Reject { reason: String },
}

impl GuardrailResult {
    pub fn accept() -> Self {
        Self::Accept
    }

    pub fn reject(reason: impl Into<String>) -> Self {
        Self::Reject {
            reason: reason.into(),
        }
    }

    pub fn is_accept(&self) -> bool {
        matches!(self, Self::Accept)
    }

    pub fn is_reject(&self) -> bool {
        matches!(self, Self::Reject { .. })
    }
}

/// Context provided to guardrails for validation
#[derive(Debug)]
pub struct GuardrailContext<'a> {
    /// The agent state (conversation history, user query, etc.)
    pub state: &'a AgentState,
    /// The tool that was invoked
    pub tool_request: &'a ToolRequest,
    /// The result from tool execution
    pub tool_result: &'a ToolResult,
}

/// Semantic guardrail trait
///
/// Guardrails validate tool outputs after execution to prevent
/// false-positive success (e.g. accepting metadata instead of actual data).
///
/// This is inspired by:
/// - Mozilla.ai's any-guardrail unified validation pattern
/// - agent.cpp's after_tool_execution lifecycle hook
pub trait SemanticGuardrail {
    /// Validate a tool output
    ///
    /// Returns Accept if output is plausible, Reject if invalid.
    fn validate(&self, context: &GuardrailContext) -> GuardrailResult;

    /// Optional name for debugging
    fn name(&self) -> &str {
        "unnamed_guardrail"
    }
}

/// Composable chain of guardrails
///
/// Executes guards in order. First rejection stops evaluation.
/// This mirrors any-guardrail's "swap validators without changing consumers" philosophy.
pub struct GuardrailChain {
    guards: Vec<Box<dyn SemanticGuardrail>>,
}

impl GuardrailChain {
    /// Create an empty guardrail chain
    pub fn new() -> Self {
        Self { guards: Vec::new() }
    }

    /// Add a guardrail to the chain
    pub fn add(mut self, guard: Box<dyn SemanticGuardrail>) -> Self {
        self.guards.push(guard);
        self
    }

    /// Run all guardrails in order
    ///
    /// Returns the first rejection, or Accept if all pass.
    pub fn validate(&self, context: &GuardrailContext) -> GuardrailResult {
        for guard in &self.guards {
            let result = guard.validate(context);
            if result.is_reject() {
                return result;
            }
        }
        GuardrailResult::Accept
    }

    /// Check if chain is empty
    pub fn is_empty(&self) -> bool {
        self.guards.is_empty()
    }

    /// Number of guardrails in chain
    pub fn len(&self) -> usize {
        self.guards.len()
    }
}

impl Default for GuardrailChain {
    fn default() -> Self {
        Self::new()
    }
}

/// Minimal plausibility guardrail
///
/// Rejects outputs that are obviously invalid:
/// - Empty output
/// - Metadata-only output (e.g. "total 12345")
/// - Outputs with no task-relevant content
///
/// This is sanity checking, not full semantic correctness.
pub struct PlausibilityGuard;

impl PlausibilityGuard {
    pub fn new() -> Self {
        Self
    }

    /// Check if output looks like metadata rather than actual data
    fn is_metadata_only(output: &str) -> bool {
        let trimmed = output.trim();

        // Empty is definitely not data
        if trimmed.is_empty() {
            return true;
        }

        // Single line starting with "total" followed by number (common ls -l header)
        if trimmed.lines().count() == 1 {
            let lower = trimmed.to_lowercase();
            if lower.starts_with("total") && trimmed.chars().any(|c| c.is_ascii_digit()) {
                // Check if it's ONLY "total <number>" with no other content
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() == 2
                    && parts[0].eq_ignore_ascii_case("total")
                    && parts[1].chars().all(|c| c.is_ascii_digit())
                {
                    return true;
                }
            }
        }

        false
    }

    /// Check if output has minimal substance
    fn has_minimal_substance(output: &str) -> bool {
        let trimmed = output.trim();

        // Require at least some non-whitespace content
        if trimmed.len() < 3 {
            return false;
        }

        // Require at least one alphanumeric character
        if !trimmed.chars().any(|c| c.is_alphanumeric()) {
            return false;
        }

        true
    }
}

impl Default for PlausibilityGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticGuardrail for PlausibilityGuard {
    fn validate(&self, context: &GuardrailContext) -> GuardrailResult {
        // Only validate successful tool executions
        if !context.tool_result.success {
            // Tool already failed - don't double-reject
            return GuardrailResult::Accept;
        }

        let output = &context.tool_result.output;

        // Check for empty output
        if output.trim().is_empty() {
            return GuardrailResult::reject(
                "Tool output is empty - no data returned"
            );
        }

        // Check for metadata-only output
        if Self::is_metadata_only(output) {
            return GuardrailResult::reject(
                "Tool output contains only metadata (e.g. 'total' line), not actual data"
            );
        }

        // Check for minimal substance
        if !Self::has_minimal_substance(output) {
            return GuardrailResult::reject(
                "Tool output lacks substantive content"
            );
        }

        GuardrailResult::Accept
    }

    fn name(&self) -> &str {
        "plausibility_guard"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_context<'a>(
        state: &'a AgentState,
        tool_request: &'a ToolRequest,
        tool_result: &'a ToolResult,
    ) -> GuardrailContext<'a> {
        GuardrailContext {
            state,
            tool_request,
            tool_result,
        }
    }

    #[test]
    fn test_plausibility_guard_accepts_valid_output() {
        let state = AgentState::new("test");
        let request = ToolRequest {
            tool: "shell".to_string(),
            params: json!({"command": "ls"}),
        };
        let result = ToolResult::success("file1.txt\nfile2.txt\n");

        let guard = PlausibilityGuard::new();
        let ctx = make_context(&state, &request, &result);
        let validation = guard.validate(&ctx);

        assert!(validation.is_accept());
    }

    #[test]
    fn test_plausibility_guard_rejects_empty() {
        let state = AgentState::new("test");
        let request = ToolRequest {
            tool: "shell".to_string(),
            params: json!({"command": "ls"}),
        };
        let result = ToolResult::success("");

        let guard = PlausibilityGuard::new();
        let ctx = make_context(&state, &request, &result);
        let validation = guard.validate(&ctx);

        assert!(validation.is_reject());
    }

    #[test]
    fn test_plausibility_guard_rejects_total_line() {
        let state = AgentState::new("test");
        let request = ToolRequest {
            tool: "shell".to_string(),
            params: json!({"command": "ls -l"}),
        };
        let result = ToolResult::success("total 7079928");

        let guard = PlausibilityGuard::new();
        let ctx = make_context(&state, &request, &result);
        let validation = guard.validate(&ctx);

        assert!(validation.is_reject());
        if let GuardrailResult::Reject { reason } = validation {
            assert!(reason.contains("metadata"));
        }
    }

    #[test]
    fn test_plausibility_guard_accepts_with_total_plus_data() {
        let state = AgentState::new("test");
        let request = ToolRequest {
            tool: "shell".to_string(),
            params: json!({"command": "ls -l"}),
        };
        let result = ToolResult::success("total 8\n-rw-r--r-- 1 user group 1234 file.txt");

        let guard = PlausibilityGuard::new();
        let ctx = make_context(&state, &request, &result);
        let validation = guard.validate(&ctx);

        assert!(validation.is_accept());
    }

    #[test]
    fn test_guardrail_chain() {
        let state = AgentState::new("test");
        let request = ToolRequest {
            tool: "shell".to_string(),
            params: json!({"command": "ls"}),
        };
        let result = ToolResult::success("total 123");

        let chain = GuardrailChain::new()
            .add(Box::new(PlausibilityGuard::new()));

        let ctx = make_context(&state, &request, &result);
        let validation = chain.validate(&ctx);

        assert!(validation.is_reject());
    }

    #[test]
    fn test_guardrail_chain_stops_on_first_reject() {
        struct AlwaysReject;
        impl SemanticGuardrail for AlwaysReject {
            fn validate(&self, _: &GuardrailContext) -> GuardrailResult {
                GuardrailResult::reject("first reject")
            }
        }

        struct NeverCalled;
        impl SemanticGuardrail for NeverCalled {
            fn validate(&self, _: &GuardrailContext) -> GuardrailResult {
                panic!("Should not be called");
            }
        }

        let state = AgentState::new("test");
        let request = ToolRequest {
            tool: "shell".to_string(),
            params: json!({"command": "ls"}),
        };
        let result = ToolResult::success("data");

        let chain = GuardrailChain::new()
            .add(Box::new(AlwaysReject))
            .add(Box::new(NeverCalled));

        let ctx = make_context(&state, &request, &result);
        let validation = chain.validate(&ctx);

        assert!(validation.is_reject());
    }
}
