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

/// Note: `submit_diagnostic_assessment` / `generate_syllabus_execution` (see
/// `roadmap_tools`) and `publish_notebook_block` / `grade_gate_submission` /
/// `submit_block_audit` (see `notebook_tools`) are NOT gated through this
/// registry — they're wired directly into their dedicated execution turns
/// (`ProviderFactory::run_diagnostic_prompt` / `run_notebook_block_prompt` /
/// `run_gate_grading_prompt` / `run_block_audit_prompt`), not through an
/// `AgentDefinition.tools: Vec<String>` list. They're listed here only so
/// `list_tools()` stays a truthful inventory of what the app can call.
pub fn list_tools() -> Vec<ToolInfo> {
    vec![
        ToolInfo {
            id: "echo".to_string(),
            name: "Echo".to_string(),
            description: "Echo the input text back verbatim. Useful for testing tool calling.".to_string(),
        },
        ToolInfo {
            id: "submit_diagnostic_assessment".to_string(),
            name: "Submit Diagnostic Assessment".to_string(),
            description: "Guarda el diagnóstico del estudiante en cuanto se conocen los datos mínimos.".to_string(),
        },
        ToolInfo {
            id: "generate_syllabus_execution".to_string(),
            name: "Generate Syllabus Execution".to_string(),
            description: "Publica el temario del curso.".to_string(),
        },
        ToolInfo {
            id: "publish_notebook_block".to_string(),
            name: "Publish Notebook Block".to_string(),
            description: "Genera el siguiente bloque pedagógico de una clase (uno a la vez, nunca la clase completa).".to_string(),
        },
        ToolInfo {
            id: "grade_gate_submission".to_string(),
            name: "Grade Gate Submission".to_string(),
            description: "Califica en texto libre la entrega de un estudiante a una compuerta de maestría.".to_string(),
        },
        ToolInfo {
            id: "grade_closure_submission".to_string(),
            name: "Grade Closure Submission".to_string(),
            description: "Califica la reflexión de cierre del estudiante y le devuelve un feedback visible.".to_string(),
        },
        ToolInfo {
            id: "submit_block_audit".to_string(),
            name: "Submit Block Audit".to_string(),
            description: "Emite el veredicto del crítico pedagógico sobre un bloque recién generado.".to_string(),
        },
    ]
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
