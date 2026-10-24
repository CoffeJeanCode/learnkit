pub mod agents;
pub mod application;
pub mod commands;
pub mod dev_trace;
pub mod domain;
pub mod error;
pub mod notebook_store;
pub mod orchestration;
pub mod persistence;
pub mod prompt_encoding;
pub mod providers;
pub mod search;
pub mod secrets;
pub mod state;
pub mod tools;

use tauri::Manager;

use state::AppState;

/// Release builds have no console (`windows_subsystem = "windows"`), and a
/// panic inside a WebView2/tao callback aborts the process (0xc0000409), so
/// the message would be lost. Append it, with a backtrace, to `crash.log` in
/// the app data dir before the abort happens.
fn install_panic_log() {
    let Some(local) = std::env::var_os("LOCALAPPDATA").or_else(|| std::env::var_os("HOME")) else {
        return;
    };
    let dir = std::path::PathBuf::from(local).join("com.learnkit.app").join("learnkit");
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        use std::io::Write;
        let thread = std::thread::current();
        let entry = format!(
            "[{:?}] panic on thread '{}': {info}\n{}\n\n",
            std::time::SystemTime::now(),
            thread.name().unwrap_or("<unnamed>"),
            std::backtrace::Backtrace::force_capture(),
        );
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("crash.log")) {
            let _ = f.write_all(entry.as_bytes());
        }
        previous(info);
    }));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_panic_log();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("learnkit=info")),
        )
        .init();

    tauri::Builder::default()
        // Updater + relaunch: the frontend checks `plugins.updater.endpoints`
        // on startup and on demand, downloads the signed artifact and asks
        // the process plugin to restart into the new version.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
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
            // Developer mode: restore the persisted switch and let traces
            // reach the UI as `dev://trace`.
            dev_trace::set_app(app.handle().clone());
            dev_trace::set_enabled(app.state::<AppState>().notebook_service.dev_mode_enabled());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::providers::list_providers,
            commands::providers::save_provider,
            commands::providers::set_active_provider,
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
            commands::research::get_search_key_status,
            commands::research::save_search_key,
            commands::research::delete_search_key,
            commands::research::get_course_research,
            commands::research::start_course_research,
            commands::research::get_class_resources,
            commands::research::open_class_resource,
            commands::roadmap::propose_context_questions,
            commands::roadmap::submit_roadmap_profile,
            commands::roadmap::confirm_roadmap_plan,
            commands::roadmap::adjust_roadmap_plan,
            commands::roadmap::answer_diagnostic_question,
            commands::roadmap::skip_diagnostic_battery,
            commands::notebook::import_course_from_roadmap,
            commands::notebook::list_courses,
            commands::notebook::list_course_classes,
            commands::notebook::start_class_notebook,
            commands::notebook::get_class_notebook_progress,
            commands::notebook::submit_gate_response,
            commands::notebook::request_easier_step,
            commands::notebook::submit_retrieval_answer,
            commands::notebook::reveal_retrieval_answer,
            commands::notebook::get_course_skill_status,
            commands::notebook::get_capability_map,
            commands::notebook::get_study_settings,
            commands::notebook::set_study_variant_mode,
            commands::notebook::log_study_event,
            commands::notebook::export_study_report,
            commands::notebook::get_skill_evidence,
            commands::notebook::get_course_evidence,
            commands::notebook::grade_closure_reflection,
            commands::notebook::retry_pending_block,
            commands::notebook::regenerate_notebook_block,
            commands::notebook::reset_class_notebook,
            commands::notebook::save_notebook_state,
            commands::notebook::save_block_annotation,
            commands::notebook::get_concept_graph,
            commands::notebook::save_concept,
            commands::notebook::delete_concept,
            commands::notebook::save_concept_link,
            commands::notebook::delete_concept_link,
            commands::notebook::get_course_diagnostic_battery,
            commands::notebook::save_diagnostic_battery_answers,
            commands::notebook::get_learner_memory,
            commands::lexical_assistant::ask_lexical_assistant,
            commands::dev::get_dev_mode,
            commands::dev::set_dev_mode,
            commands::dev::get_dev_trace,
            commands::dev::clear_dev_trace,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
