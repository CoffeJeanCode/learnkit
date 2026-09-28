pub mod agents;
pub mod application;
pub mod commands;
pub mod domain;
pub mod error;
pub mod notebook_store;
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
            // NOTE: the Stronghold JS plugin is intentionally NOT registered:
            // keys are handled in Rust commands only, and the plugin's own
            // Argon2 init on this thread would add seconds (much more in
            // debug builds) to startup before first paint for zero benefit.
            let started = std::time::Instant::now();
            let state = AppState::new(data_dir).expect("failed to initialize app state");
            tracing::info!(elapsed_ms = started.elapsed().as_millis(), "app state ready");
            app.manage(state);
            // Tool calls emit `agent://tool` through this handle (see
            // `tools::emit_tool_event`) so the UI can animate tool-running
            // turns differently from plain thinking ones.
            tools::set_tool_event_emitter(app.handle().clone());

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
            commands::roadmap::retry_roadmap_turn,
            commands::roadmap::get_roadmap_session,
            commands::roadmap::list_roadmap_sessions,
            commands::roadmap::rename_roadmap_session,
            commands::roadmap::delete_roadmap_session,
            commands::roadmap::ensure_course_imported,
            commands::roadmap::answer_diagnostic_question,
            commands::roadmap::skip_diagnostic_battery,
            commands::notebook::import_course_from_roadmap,
            commands::notebook::list_courses,
            commands::notebook::list_course_classes,
            commands::notebook::start_class_notebook,
            commands::notebook::get_class_notebook_progress,
            commands::notebook::submit_gate_response,
            commands::notebook::grade_closure_reflection,
            commands::notebook::retry_pending_block,
            commands::notebook::regenerate_notebook_block,
            commands::notebook::save_notebook_state,
            commands::notebook::get_course_diagnostic_battery,
            commands::notebook::save_diagnostic_battery_answers,
            commands::lexical_assistant::ask_lexical_assistant,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
