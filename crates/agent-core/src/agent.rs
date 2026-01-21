use crate::protocol::{parse_model_output, ParseResult};
use crate::skill::SkillRequest;
use crate::tool::{ToolRequest, ToolResult};
use serde::{Deserialize, Serialize};

/// The state of the agent during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    /// The conversation history (user messages, model responses, tool results)
    pub history: Vec<Message>,

    /// Whether the agent has reached a final answer
    pub is_complete: bool,

    /// The final answer, if complete
    pub final_answer: Option<String>,
}

/// A message in the conversation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// The role of a message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    Tool,
}

impl AgentState {
    /// Create a new agent state with an initial user query
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            history: vec![Message {
                role: Role::User,
                content: query.into(),
            }],
            is_complete: false,
            final_answer: None,
        }
    }

    /// Add a message to the history
    pub fn add_message(&mut self, role: Role, content: impl Into<String>) {
        self.history.push(Message {
            role,
            content: content.into(),
        });
    }
}

/// The decision made by the agent after processing model output
#[derive(Debug, Clone)]
pub enum AgentDecision {
    /// The agent wants to invoke a tool
    InvokeTool(ToolRequest),

    /// The agent wants to invoke a skill
    /// Skills are contract-based, guardrail-enforced operations
    InvokeSkill(SkillRequest),

    /// The agent has produced a final answer
    Done(String),

    /// The agent produced inconclusive output (reasoning without action)
    /// This indicates the model failed to follow instructions properly
    Inconclusive(String),
}

/// Process model output and decide the next action
///
/// This is the core agent loop logic:
/// 1. Parse the model output
/// 2. Decide if it's a tool call, skill invocation, final answer, or inconclusive
/// 3. Return the appropriate decision
///
/// This function is pure, deterministic, and has no side effects.
pub fn process_model_output(
    state: &mut AgentState,
    model_output: impl Into<String>,
) -> AgentDecision {
    let output = model_output.into();

    match parse_model_output(&output) {
        ParseResult::ToolCall(tool_request) => {
            // Add the model's tool call to history
            state.add_message(Role::Assistant, output);
            AgentDecision::InvokeTool(tool_request)
        }
        ParseResult::SkillCall(skill_request) => {
            // Add the model's skill invocation to history
            state.add_message(Role::Assistant, output);
            AgentDecision::InvokeSkill(skill_request)
        }
        ParseResult::FinalAnswer(answer) => {
            // Add the final answer to history
            state.add_message(Role::Assistant, answer.clone());
            state.is_complete = true;
            state.final_answer = Some(answer.clone());
            AgentDecision::Done(answer)
        }
        ParseResult::Inconclusive(output) => {
            // Model produced reasoning/explanation without completing the task
            // Don't add to history yet - runtime will handle corrective retry
            AgentDecision::Inconclusive(output)
        }
    }
}

/// Apply a tool result to the agent state
///
/// This adds the tool result to the conversation history so the model
/// can see what happened when it invoked the tool.
pub fn apply_tool_result(state: &mut AgentState, result: &ToolResult) {
    let content = if result.success {
        format!("Tool output:\n{}", result.output)
    } else {
        format!(
            "Tool failed: {}",
            result.error.as_deref().unwrap_or("unknown error")
        )
    };

    state.add_message(Role::Tool, content);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_agent_state() {
        let state = AgentState::new("Hello");
        assert_eq!(state.history.len(), 1);
        assert!(!state.is_complete);
        assert!(state.final_answer.is_none());
    }

    #[test]
    fn test_process_tool_call() {
        let mut state = AgentState::new("List files");
        let output = r#"{"tool": "shell", "command": "ls"}"#;

        match process_model_output(&mut state, output) {
            AgentDecision::InvokeTool(req) => {
                assert_eq!(req.tool, "shell");
            }
            _ => panic!("Expected tool invocation"),
        }

        assert_eq!(state.history.len(), 2); // user + assistant
        assert!(!state.is_complete);
    }

    #[test]
    fn test_process_final_answer() {
        let mut state = AgentState::new("What is 2+2?");
        let output = "The answer is 4.";

        match process_model_output(&mut state, output) {
            AgentDecision::Done(answer) => {
                assert_eq!(answer, "The answer is 4.");
            }
            _ => panic!("Expected final answer"),
        }

        assert!(state.is_complete);
        assert_eq!(state.final_answer, Some("The answer is 4.".to_string()));
    }

    #[test]
    fn test_apply_tool_result() {
        let mut state = AgentState::new("Test");
        let result = ToolResult::success("file1.txt\nfile2.txt");

        apply_tool_result(&mut state, &result);

        assert_eq!(state.history.len(), 2);
        assert!(matches!(state.history[1].role, Role::Tool));
    }
}
