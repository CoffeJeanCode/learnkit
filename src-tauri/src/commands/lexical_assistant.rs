use tauri::State;

use crate::application::lexical_assistant_service::LexicalTurn as ServiceLexicalTurn;
use crate::error::AppResult;
use crate::state::AppState;

/// Wire shape for one past popover exchange — mirrors
/// `application::lexical_assistant_service::LexicalTurn` but derives
/// `serde` for the Tauri boundary (the service type deliberately doesn't,
/// to keep it a plain internal value type).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LexicalTurn {
    pub question: String,
    pub answer: String,
}

/// Answers one popover question, isolated to `term`/`fragment_context`/
/// `key_concepts`/`history`/`question` — never the current gate or session
/// state. `answer_bearing_strings` (the current gate's option texts /
/// correct branch / model solution, if any) is used ONLY for the
/// deterministic output-side redaction guard, never sent into the model
/// prompt — see `LexicalAssistantService::ask`.
#[tauri::command]
pub async fn ask_lexical_assistant(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    term: String,
    fragment_context: Option<String>,
    key_concepts: Vec<String>,
    history: Vec<LexicalTurn>,
    question: String,
    answer_bearing_strings: Vec<String>,
) -> AppResult<String> {
    let history: Vec<ServiceLexicalTurn> =
        history.into_iter().map(|t| ServiceLexicalTurn { question: t.question, answer: t.answer }).collect();
    state
        .lexical_assistant_service
        .ask(Some(&app), &term, fragment_context.as_deref(), &key_concepts, &history, &question, &answer_bearing_strings)
        .await
}
