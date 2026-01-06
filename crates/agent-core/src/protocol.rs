use crate::tool::ToolRequest;

/// Parse model output to determine if it contains a tool call
///
/// Protocol:
/// - If the output contains valid JSON with a "tool" field, it's a tool call
/// - Otherwise, it's treated as a final answer
pub fn parse_model_output(output: &str) -> ParseResult {
    // Try to parse as JSON
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(output.trim()) {
        // Check if it has a "tool" field
        if value.get("tool").is_some() {
            // Try to deserialize as ToolRequest
            if let Ok(tool_request) = serde_json::from_value::<ToolRequest>(value) {
                return ParseResult::ToolCall(tool_request);
            }
        }
    }

    // Not a tool call - treat as final answer
    ParseResult::FinalAnswer(output.to_string())
}

/// The result of parsing model output
#[derive(Debug, Clone)]
pub enum ParseResult {
    /// The model wants to invoke a tool
    ToolCall(ToolRequest),

    /// The model has produced a final answer
    FinalAnswer(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tool_call() {
        let json = r#"{"tool": "shell", "command": "ls -la"}"#;
        match parse_model_output(json) {
            ParseResult::ToolCall(req) => {
                assert_eq!(req.tool, "shell");
            }
            _ => panic!("Expected tool call"),
        }
    }

    #[test]
    fn test_parse_final_answer() {
        let text = "The current directory contains 5 files.";
        match parse_model_output(text) {
            ParseResult::FinalAnswer(answer) => {
                assert_eq!(answer, text);
            }
            _ => panic!("Expected final answer"),
        }
    }

    #[test]
    fn test_parse_json_without_tool() {
        let json = r#"{"result": "some data"}"#;
        match parse_model_output(json) {
            ParseResult::FinalAnswer(_) => {}
            _ => panic!("Expected final answer"),
        }
    }
}
