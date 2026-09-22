use std::sync::Arc;

use crate::domain::agent::AgentOutput;
use crate::domain::workflow::WorkflowDefinition;
use crate::error::AppResult;
use crate::orchestration::Orchestrator;

/// Minimal workflow service. No engine yet: exposes the demo delegation
/// (`researcher -> writer`) and reserves workflow CRUD for later.
pub struct WorkflowService {
    orchestrator: Arc<Orchestrator>,
}

impl WorkflowService {
    pub fn new(orchestrator: Arc<Orchestrator>) -> Self {
        Self { orchestrator }
    }

    pub fn list(&self) -> Vec<WorkflowDefinition> {
        vec![WorkflowDefinition {
            id: "research_to_draft".to_string(),
            name: "Research → Draft".to_string(),
            description: Some("Demo: researcher findings rewritten by writer.".to_string()),
            steps: vec![
                crate::domain::workflow::WorkflowStep::RunAgent {
                    agent_id: "researcher".to_string(),
                    input: "{{topic}}".to_string(),
                },
                crate::domain::workflow::WorkflowStep::Delegate {
                    from_agent: "researcher".to_string(),
                    to_agent: "writer".to_string(),
                    task: "{{findings}}".to_string(),
                },
            ],
        }]
    }

    /// Run the demo chain sequentially: researcher output feeds the writer.
    #[tracing::instrument(skip(self), fields(topic_len = topic.len()))]
    pub async fn run_research_to_draft(
        &self,
        app: Option<&tauri::AppHandle>,
        topic: &str,
    ) -> AppResult<AgentOutput> {
        let findings = self.orchestrator.run_agent(app, "researcher", topic, None).await?;
        let task = format!(
            "Transform these structured findings into a clear final response:\n\n{}",
            findings.text
        );
        self.orchestrator.delegate(app, "researcher", "writer", &task).await
    }
}
