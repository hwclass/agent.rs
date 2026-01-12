use crate::tool::ToolRequest;

/// Parse model output to determine if it contains a tool call
///
/// Protocol:
/// - If the output contains valid JSON with a "tool" field, it's a tool call
/// - If the output appears to be reasoning/explanation without action, it's inconclusive
/// - Otherwise, it's treated as a final answer
pub fn parse_model_output(output: &str) -> ParseResult {
    let trimmed = output.trim();

    // Try to parse as JSON
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        // Check if it has a "tool" field
        if value.get("tool").is_some() {
            // Try to deserialize as ToolRequest
            if let Ok(tool_request) = serde_json::from_value::<ToolRequest>(value) {
                return ParseResult::ToolCall(tool_request);
            }
        }
    }

    // Detect inconclusive outputs - reasoning without action
    if is_inconclusive(trimmed) {
        return ParseResult::Inconclusive(trimmed.to_string());
    }

    // Otherwise, treat as final answer
    ParseResult::FinalAnswer(trimmed.to_string())
}

/// Detect if output is inconclusive (reasoning without action)
///
/// An output is inconclusive if it describes intent or approach but doesn't
/// actually complete the task or invoke a tool.
fn is_inconclusive(output: &str) -> bool {
    let lower = output.to_lowercase();

    // Indicators that the model is explaining what it will do, not doing it
    let planning_phrases = [
        "i will",
        "i'll",
        "let me",
        "let's",
        "we can",
        "we will",
        "to do this",
        "first,",
        "step 1",
        "the command",
        "using the",
        "by using",
    ];

    // Check if output contains planning phrases and is relatively short
    // (longer responses are more likely to be complete answers)
    if output.len() < 300 {
        for phrase in &planning_phrases {
            if lower.contains(phrase) {
                return true;
            }
        }
    }

    false
}

/// The result of parsing model output
#[derive(Debug, Clone)]
pub enum ParseResult {
    /// The model wants to invoke a tool
    ToolCall(ToolRequest),

    /// The model has produced a final answer
    FinalAnswer(String),

    /// The model produced output that doesn't complete the task or invoke a tool
    /// (reasoning, explanation, or malformed output)
    Inconclusive(String),
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
