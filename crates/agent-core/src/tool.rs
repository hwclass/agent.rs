use serde::{Deserialize, Serialize};

/// A tool request parsed from model output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequest {
    /// The tool name (e.g., "shell")
    pub tool: String,

    /// The command or parameters for the tool
    #[serde(flatten)]
    pub params: serde_json::Value,
}

/// The result of executing a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Whether the tool executed successfully
    pub success: bool,

    /// The output from the tool
    pub output: String,

    /// Optional error message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ToolResult {
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            error: None,
        }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            output: String::new(),
            error: Some(error.into()),
        }
    }
}
