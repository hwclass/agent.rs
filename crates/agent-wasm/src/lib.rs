//! # agent-wasm
//!
//! WASM compilation target for agent-core.
//!
//! This demonstrates that the agent logic is portable and can run in WASM.
//! The LLM inference and tool execution happen outside WASM - this module
//! only proves the decision-making logic is sandboxable.

use agent_core::{agent::process_model_output, AgentState};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Input to the agent step function
#[derive(Debug, Serialize, Deserialize)]
pub struct StepInput {
    /// The current agent state as JSON
    pub state_json: String,

    /// The latest model output
    pub model_output: String,
}

/// Output from the agent step function
#[derive(Debug, Serialize, Deserialize)]
pub struct StepOutput {
    /// The updated agent state as JSON
    pub state_json: String,

    /// The decision made by the agent
    pub decision: DecisionOutput,
}

/// The decision output
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DecisionOutput {
    /// Invoke a tool
    InvokeTool {
        tool: String,
        params: serde_json::Value,
    },

    /// Agent is done
    Done { answer: String },
}

/// Run one step of the agent loop in WASM
///
/// This function:
/// 1. Deserializes the agent state
/// 2. Processes the model output
/// 3. Makes a decision
/// 4. Serializes the result
///
/// # Example
///
/// ```javascript
/// const input = {
///   state_json: '{"history":[...],"is_complete":false,"final_answer":null}',
///   model_output: '{"tool":"shell","command":"ls"}'
/// };
/// const output = run_agent_step(JSON.stringify(input));
/// const result = JSON.parse(output);
/// ```
#[wasm_bindgen]
pub fn run_agent_step(input_json: &str) -> Result<String, JsValue> {
    // Parse input
    let input: StepInput = serde_json::from_str(input_json)
        .map_err(|e| JsValue::from_str(&format!("Invalid input JSON: {}", e)))?;

    // Deserialize state
    let mut state: AgentState = serde_json::from_str(&input.state_json)
        .map_err(|e| JsValue::from_str(&format!("Invalid state JSON: {}", e)))?;

    // Process model output
    let decision = process_model_output(&mut state, input.model_output);

    // Convert decision to output format
    let decision_output = match decision {
        agent_core::AgentDecision::InvokeTool(req) => DecisionOutput::InvokeTool {
            tool: req.tool,
            params: req.params,
        },
        agent_core::AgentDecision::Done(answer) => DecisionOutput::Done { answer },
    };

    // Serialize state
    let state_json = serde_json::to_string(&state)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize state: {}", e)))?;

    // Create output
    let output = StepOutput {
        state_json,
        decision: decision_output,
    };

    // Serialize output
    serde_json::to_string(&output)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize output: {}", e)))
}

/// Create a new agent state with a user query
#[wasm_bindgen]
pub fn create_agent_state(query: &str) -> Result<String, JsValue> {
    let state = AgentState::new(query);
    serde_json::to_string(&state)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize state: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_agent_step_tool_call() {
        let state = AgentState::new("List files");
        let state_json = serde_json::to_string(&state).unwrap();

        let input = StepInput {
            state_json,
            model_output: r#"{"tool":"shell","command":"ls"}"#.to_string(),
        };

        let input_json = serde_json::to_string(&input).unwrap();
        let output_json = run_agent_step(&input_json).unwrap();
        let output: StepOutput = serde_json::from_str(&output_json).unwrap();

        match output.decision {
            DecisionOutput::InvokeTool { tool, .. } => {
                assert_eq!(tool, "shell");
            }
            _ => panic!("Expected tool invocation"),
        }
    }

    #[test]
    fn test_run_agent_step_done() {
        let state = AgentState::new("What is 2+2?");
        let state_json = serde_json::to_string(&state).unwrap();

        let input = StepInput {
            state_json,
            model_output: "The answer is 4.".to_string(),
        };

        let input_json = serde_json::to_string(&input).unwrap();
        let output_json = run_agent_step(&input_json).unwrap();
        let output: StepOutput = serde_json::from_str(&output_json).unwrap();

        match output.decision {
            DecisionOutput::Done { answer } => {
                assert_eq!(answer, "The answer is 4.");
            }
            _ => panic!("Expected done"),
        }
    }

    #[test]
    fn test_create_agent_state() {
        let state_json = create_agent_state("Test query").unwrap();
        let state: AgentState = serde_json::from_str(&state_json).unwrap();

        assert_eq!(state.history.len(), 1);
        assert!(!state.is_complete);
    }
}
