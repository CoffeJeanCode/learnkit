use crate::domain::agent::AgentDefinition;
use crate::domain::model::ModelRef;

/// Demo agents used to validate the multi-agent path (potentially different
/// providers/models each). Seeded on first run if the registry is empty.
pub fn researcher_definition() -> AgentDefinition {
    AgentDefinition {
        id: "researcher".to_string(),
        name: "Researcher".to_string(),
        description: Some("Analyzes a task and returns concise structured findings.".to_string()),
        system_prompt: "You analyze a task and return concise structured findings.".to_string(),
        model: ModelRef::new("openai", "gpt-5"),
        tools: vec!["echo".to_string()],
    }
}

pub fn writer_definition() -> AgentDefinition {
    AgentDefinition {
        id: "writer".to_string(),
        name: "Writer".to_string(),
        description: Some("Transforms structured findings into a clear final response.".to_string()),
        system_prompt: "You transform structured findings into a clear final response.".to_string(),
        model: ModelRef::new("anthropic", "claude-sonnet-4-6"),
        tools: vec![],
    }
}

pub fn demo_definitions() -> Vec<AgentDefinition> {
    vec![researcher_definition(), writer_definition(), super::roadmap_agent::definition()]
}
