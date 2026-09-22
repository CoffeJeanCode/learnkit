use tauri::State;

use crate::domain::agent::AgentOutput;
use crate::domain::workflow::WorkflowDefinition;
use crate::error::AppResult;
use crate::state::AppState;

/// Chat/playground commands. History stays in frontend memory; the backend is stateless.
#[tauri::command]
pub async fn delegate_agent(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    from_agent_id: String,
    to_agent_id: String,
    task: String,
) -> AppResult<AgentOutput> {
    state
        .orchestrator
        .delegate(Some(&app), &from_agent_id, &to_agent_id, &task)
        .await
}

#[tauri::command]
pub fn list_workflows(state: State<'_, AppState>) -> Vec<WorkflowDefinition> {
    state.workflow_service.list()
}

#[tauri::command]
pub async fn run_research_to_draft(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    topic: String,
) -> AppResult<AgentOutput> {
    state.workflow_service.run_research_to_draft(Some(&app), &topic).await
}
