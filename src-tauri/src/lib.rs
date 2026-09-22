pub mod agents;
pub mod application;
pub mod commands;
pub mod domain;
pub mod error;
pub mod orchestration;
pub mod persistence;
pub mod providers;
pub mod secrets;
pub mod state;
pub mod tools;

use tauri::Manager;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("learnkit=info")),
        )
        .init();

    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app
                .path()
                .app_local_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let state = AppState::new(data_dir).expect("failed to initialize app state");
            app.manage(state);

            // Stronghold engine for API-key storage (keys are handled in Rust
            // commands only; the JS bindings are available for future non-secret use).
            let salt_path = app
                .path()
                .app_local_data_dir()
                .map(|p| p.join("salt.txt"))
                .unwrap_or_else(|_| std::path::PathBuf::from("salt.txt"));
            app.handle()
                .plugin(tauri_plugin_stronghold::Builder::with_argon2(&salt_path).build())?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::providers::list_providers,
            commands::providers::save_provider,
            commands::providers::delete_provider,
            commands::providers::save_provider_key,
            commands::providers::provider_has_key,
            commands::providers::delete_provider_key,
            commands::providers::test_provider,
            commands::providers::list_models,
            commands::providers::suggested_models,
            commands::agents::list_agents,
            commands::agents::get_agent,
            commands::agents::create_agent,
            commands::agents::delete_agent,
            commands::agents::run_agent,
            commands::agents::list_tools,
            commands::chat::delegate_agent,
            commands::chat::list_workflows,
            commands::chat::run_research_to_draft,
            commands::roadmap::start_roadmap_session,
            commands::roadmap::send_roadmap_message,
            commands::roadmap::get_roadmap_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
