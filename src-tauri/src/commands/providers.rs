use tauri::State;

use crate::domain::model::ModelInfo;
use crate::domain::provider::{ProviderConfig, ProviderKind, ProviderWithStatus};
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub fn list_providers(state: State<'_, AppState>) -> AppResult<Vec<ProviderWithStatus>> {
    Ok(state.provider_service.list())
}

#[tauri::command]
pub fn save_provider(state: State<'_, AppState>, config: ProviderConfig) -> AppResult<ProviderWithStatus> {
    state.provider_service.save_provider(config)
}

#[tauri::command]
pub fn delete_provider(state: State<'_, AppState>, provider_id: String) -> AppResult<()> {
    state.provider_service.delete_provider(&provider_id)
}

/// Store the API key. The key is write-only: it is never returned or logged.
#[tauri::command]
pub fn save_provider_key(
    state: State<'_, AppState>,
    provider_id: String,
    api_key: String,
) -> AppResult<bool> {
    let saved = state.provider_service.save_provider_key(&provider_id, &api_key)?;
    // Onboarding: a freshly saved key may be exactly what the roadmap agent
    // was waiting for.
    state.heal_roadmap_agent_model();
    Ok(saved)
}

#[tauri::command]
pub fn provider_has_key(state: State<'_, AppState>, provider_id: String) -> AppResult<bool> {
    state.provider_service.provider_has_key(&provider_id)
}

#[tauri::command]
pub fn delete_provider_key(state: State<'_, AppState>, provider_id: String) -> AppResult<()> {
    state.provider_service.delete_provider_key(&provider_id)
}

#[tauri::command]
pub async fn test_provider(state: State<'_, AppState>, provider_id: String) -> AppResult<()> {
    state.provider_service.test_provider(&provider_id).await
}

#[tauri::command]
pub async fn list_models(state: State<'_, AppState>, provider_id: String) -> AppResult<Vec<ModelInfo>> {
    state.provider_service.list_models(&provider_id).await
}

#[tauri::command]
pub fn suggested_models(state: State<'_, AppState>, provider: ProviderKind) -> Vec<String> {
    state.provider_service.suggested_models(provider)
}
