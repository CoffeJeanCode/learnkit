//! Commands for the resource-research feature: the search key (kept in the same
//! encrypted vault as the model keys), the per-course research status, and the
//! curated resources of a class.

use serde::Serialize;
use tauri::State;

use crate::domain::resources::{ClassResource, ResearchStatus, normalize_url};
use crate::error::{AppError, AppResult};
use crate::search::{SEARCH_KEY_ENV, SEARCH_KEY_ID};
use crate::state::AppState;

/// Longest key accepted: real keys are well under this, and it bounds what a
/// paste accident can write into the vault.
const MAX_KEY_CHARS: usize = 300;

/// Where the search key comes from. The key itself is never returned.
#[derive(Debug, Serialize)]
pub struct SearchKeyStatus {
    pub configured: bool,
    /// `"vault"` (saved in the app), `"env"` (developer fallback) or `"none"`.
    pub source: &'static str,
}

#[derive(Debug, Serialize)]
pub struct CourseResearch {
    /// `None` = never attempted for this course.
    pub status: Option<ResearchStatus>,
    pub detail: Option<String>,
}

fn env_key_present() -> bool {
    std::env::var(SEARCH_KEY_ENV).map(|k| !k.trim().is_empty()).unwrap_or(false)
}

#[tauri::command]
pub fn get_search_key_status(state: State<'_, AppState>) -> AppResult<SearchKeyStatus> {
    let in_vault = state.vault.has_provider_key(SEARCH_KEY_ID)?;
    Ok(match (in_vault, env_key_present()) {
        (true, _) => SearchKeyStatus { configured: true, source: "vault" },
        (false, true) => SearchKeyStatus { configured: true, source: "env" },
        (false, false) => SearchKeyStatus { configured: false, source: "none" },
    })
}

/// Saves the search key, encrypted. Nothing about the key is logged or echoed
/// back, and a rejected value never reaches the vault.
#[tauri::command]
pub fn save_search_key(state: State<'_, AppState>, key: String) -> AppResult<()> {
    let key = key.trim();
    if key.is_empty() {
        return Err(AppError::InvalidInput("la clave está vacía".to_string()));
    }
    if key.chars().count() > MAX_KEY_CHARS || key.chars().any(char::is_whitespace) {
        return Err(AppError::InvalidInput("la clave no tiene el formato de una API key".to_string()));
    }
    state.vault.save_provider_key(SEARCH_KEY_ID, key)?;
    Ok(())
}

#[tauri::command]
pub fn delete_search_key(state: State<'_, AppState>) -> AppResult<()> {
    state.vault.delete_provider_key(SEARCH_KEY_ID)?;
    Ok(())
}

#[tauri::command]
pub fn get_course_research(state: State<'_, AppState>, course_id: String) -> AppResult<CourseResearch> {
    let current = state.research_service.status(&course_id)?;
    Ok(CourseResearch { status: current.as_ref().map(|(s, _)| *s), detail: current.and_then(|(_, d)| d) })
}

/// Starts (or resumes) the research of a course in the background. Idempotent:
/// a running or finished research is left alone, so the UI can call it freely
/// — on opening the plan, or from a "buscar de nuevo" button after adding a key.
#[tauri::command]
pub fn start_course_research(app: tauri::AppHandle, state: State<'_, AppState>, course_id: String) -> AppResult<()> {
    state.research_service.start(Some(app), course_id);
    Ok(())
}

#[tauri::command]
pub fn get_class_resources(state: State<'_, AppState>, class_id: String) -> AppResult<Vec<ClassResource>> {
    state.notebook_service.get_class_resources(&class_id)
}

/// The stored URL to open for `requested`, or `None` when it is not one of the
/// class's curated resources. The webview can therefore ask the backend to open
/// ONLY a link the research pipeline already vetted — never an arbitrary URL
/// (or a `javascript:`/`file:` one) typed or injected by something on the page.
pub(crate) fn resource_url_to_open(resources: &[ClassResource], requested: &str) -> Option<String> {
    let wanted = normalize_url(requested)?;
    resources.iter().find(|r| normalize_url(&r.url).as_deref() == Some(wanted.as_str())).map(|r| r.url.clone())
}

/// Hands `url` to the operating system's default browser. The URL is a separate
/// argument — never interpolated into a shell string — and callers only pass
/// URLs that already went through [`resource_url_to_open`].
fn open_in_browser(url: &str) -> AppResult<()> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = std::process::Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    command.spawn().map(|_| ()).map_err(|e| AppError::InvalidInput(format!("no pude abrir el navegador: {e}")))
}

/// Opens one of a class's curated resources in the default browser.
#[tauri::command]
pub fn open_class_resource(state: State<'_, AppState>, class_id: String, url: String) -> AppResult<()> {
    let resources = state.notebook_service.get_class_resources(&class_id)?;
    let stored = resource_url_to_open(&resources, &url).ok_or_else(|| AppError::InvalidInput("ese enlace no es un recurso de esta clase".to_string()))?;
    open_in_browser(&stored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::resources::ResourceKind;

    fn resource(url: &str) -> ClassResource {
        ClassResource { title: "T".into(), url: url.into(), kind: ResourceKind::Tutorial, why: "w".into(), what_to_do: "d".into() }
    }

    #[test]
    fn only_a_curated_link_can_be_opened() {
        let resources = [resource("https://real.dev/guide"), resource("https://other.dev/x")];
        assert_eq!(resource_url_to_open(&resources, "https://real.dev/guide").as_deref(), Some("https://real.dev/guide"));
        assert_eq!(resource_url_to_open(&resources, "https://not-curated.example/").as_deref(), None, "a link that is not on the class's list");
        for dangerous in ["javascript:alert(1)", "file:///C:/Windows/System32/calc.exe", "ms-msdt:/id", "calc.exe", "", "https://real.dev.evil.example/guide"] {
            assert_eq!(resource_url_to_open(&resources, dangerous), None, "{dangerous:?} must never reach the OS");
        }
    }

    #[test]
    fn the_stored_spelling_is_what_gets_opened_not_the_requested_one() {
        let resources = [resource("https://real.dev/guide")];
        let opened = resource_url_to_open(&resources, "https://REAL.dev/guide/?utm_source=x#top").expect("same resource through noise");
        assert_eq!(opened, "https://real.dev/guide", "the OS receives the vetted URL, not the caller's variant");
    }

    #[test]
    fn a_class_without_resources_opens_nothing() {
        assert_eq!(resource_url_to_open(&[], "https://real.dev/guide"), None);
    }
}
