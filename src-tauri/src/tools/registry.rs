use serde::Serialize;

/// Static metadata for one tool. The executable implementation lives next to
/// it (see [`echo`](super::echo)); the factory wires implementations by id so
/// new tools only need: implementation + one entry here.
#[derive(Debug, Clone, Serialize)]
pub struct ToolInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

pub fn list_tools() -> Vec<ToolInfo> {
    vec![ToolInfo {
        id: "echo".to_string(),
        name: "Echo".to_string(),
        description: "Echo the input text back verbatim. Useful for testing tool calling.".to_string(),
    }]
}

pub fn is_known_tool(id: &str) -> bool {
    list_tools().iter().any(|t| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_echo() {
        let tools = list_tools();
        assert!(tools.iter().any(|t| t.id == "echo"));
        assert!(is_known_tool("echo"));
        assert!(!is_known_tool("shell"));
    }
}
