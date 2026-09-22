use rig_agent::tool::{Tool, ToolContext};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

/// Arguments for the `echo` tool.
#[derive(Debug, Deserialize)]
pub struct EchoArgs {
    pub text: String,
}

/// Output of the `echo` tool.
#[derive(Debug, Serialize)]
pub struct EchoOutput {
    pub echoed: String,
}

/// Minimal tool that returns its input. Exists to validate tool calling
/// end-to-end. Future tools (`read_file`, `http_request`, `subagent`, …)
/// follow this same shape and are registered in [`ToolRegistry`](super::registry).
#[derive(Debug, Clone, Copy)]
pub struct EchoTool;

impl Tool for EchoTool {
    const NAME: &'static str = "echo";
    type Args = EchoArgs;
    type Output = EchoOutput;
    type Error = Infallible;

    fn description(&self) -> String {
        "Echo the input text back verbatim. Useful for testing tool calling.".to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "Text to echo back" }
            },
            "required": ["text"]
        })
    }

    async fn call(&self, _context: &mut ToolContext, args: Self::Args) -> Result<Self::Output, Self::Error> {
        Ok(EchoOutput { echoed: args.text })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn echo_returns_input() {
        let tool = EchoTool;
        let mut ctx = ToolContext::new();
        let out = tool
            .call(&mut ctx, EchoArgs { text: "hello".to_string() })
            .await
            .expect("echo never fails");
        assert_eq!(out.echoed, "hello");
    }
}
