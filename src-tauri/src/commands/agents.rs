use tauri::State;

use crate::domain::agent::{AgentDefinition, AgentOutput};
use crate::error::AppResult;
use crate::state::AppState;
use crate::tools::ToolInfo;

#[tauri::command]
pub fn list_agents(state: State<'_, AppState>) -> AppResult<Vec<AgentDefinition>> {
    Ok(state.agent_service.list())
}

#[tauri::command]
pub fn get_agent(state: State<'_, AppState>, agent_id: String) -> AppResult<AgentDefinition> {
    state.agent_service.get(&agent_id)
}

#[tauri::command]
pub fn create_agent(state: State<'_, AppState>, agent: AgentDefinition) -> AppResult<AgentDefinition> {
    state.agent_service.create(agent)
}

#[tauri::command]
pub fn delete_agent(state: State<'_, AppState>, agent_id: String) -> AppResult<()> {
    state.agent_service.delete(&agent_id)
}

#[tauri::command]
pub async fn run_agent(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    agent_id: String,
    input: String,
) -> AppResult<AgentOutput> {
    state.agent_service.run(Some(&app), &agent_id, &input).await
}

#[tauri::command]
pub fn list_tools(state: State<'_, AppState>) -> Vec<ToolInfo> {
    let _ = &state;
    state.agent_service.tools()
}
