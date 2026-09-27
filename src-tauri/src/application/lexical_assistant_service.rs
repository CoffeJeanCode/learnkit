use std::sync::Arc;

use crate::agents::lexical_assistant_agent::LEXICAL_ASSISTANT_AGENT_ID;
use crate::error::AppResult;
use crate::orchestration::Orchestrator;

/// One past exchange in the same popover session — frontend-held, sent back
/// on each call (see `commands::lexical_assistant::ask_lexical_assistant`).
/// Kept as an explicit parameter (not an implicit server-side session
/// lookup) so a future persistent per-student learning-memory store can
/// supply it later without reshaping this call.
#[derive(Debug, Clone)]
pub struct LexicalTurn {
    pub question: String,
    pub answer: String,
}

/// Application service for the popover lexical/clarification assistant.
/// Isolated on the way IN — the model prompt gets only the term, an
/// optional surrounding fragment, the lesson's key concept NAMES, recent
/// popover history, and the question, never any gate/session state — and
/// FILTERED on the way OUT: `ask` always runs the reply through a
/// deterministic, code-level redaction guard before returning it, because
/// prompt compliance alone is never a trustworthy safety boundary.
pub struct LexicalAssistantService {
    orchestrator: Arc<Orchestrator>,
}

impl LexicalAssistantService {
    pub fn new(orchestrator: Arc<Orchestrator>) -> Self {
        Self { orchestrator }
    }

    /// `answer_bearing_strings` are the CURRENT gate's option texts / correct
    /// branch / model solution — used ONLY for the output-side redaction
    /// check below, never sent into the model prompt itself (that's what
    /// keeps this agent's input genuinely isolated).
    pub async fn ask(
        &self,
        app: Option<&tauri::AppHandle>,
        term: &str,
        fragment_context: Option<&str>,
        key_concepts: &[String],
        history: &[LexicalTurn],
        question: &str,
        answer_bearing_strings: &[String],
    ) -> AppResult<String> {
        let input = render_input(term, fragment_context, key_concepts, history, question);
        let output = self.orchestrator.run_agent(app, LEXICAL_ASSISTANT_AGENT_ID, &input, None).await?;
        Ok(redact_leaked_answers(&output.text, answer_bearing_strings))
    }
}

fn render_input(term: &str, fragment_context: Option<&str>, key_concepts: &[String], history: &[LexicalTurn], question: &str) -> String {
    let context = serde_json::json!({
        "selectedTerm": term,
        "fragmentContext": fragment_context,
        "lessonKeyConcepts": key_concepts,
        "recentHistory": history.iter().map(|t| serde_json::json!({"question": t.question, "answer": t.answer})).collect::<Vec<_>>(),
        "studentQuestion": question,
    });
    format!("Responde según tu system prompt, en las 3 capas obligatorias.\n\n{}", serde_json::to_string_pretty(&context).unwrap_or_default())
}

/// The actual automatable safety net for the anti-shortcut protocol: if the
/// model's reply contains any answer-bearing string verbatim (case
/// insensitive), or matches a small blocklist of direct-answer phrasings,
/// replace it with a generic refusal instead of ever returning it.
fn redact_leaked_answers(reply: &str, answer_bearing_strings: &[String]) -> String {
    const REFUSAL: &str =
        "Eso ya toca la respuesta del ejercicio — no puedo dártela aquí. Puedo explicarte el concepto general si quieres.";
    let lower = reply.to_lowercase();

    for s in answer_bearing_strings {
        let s_trim = s.trim();
        // Skip trivially short strings (e.g. a single-letter option like
        // "A") — matching those against arbitrary reply text would redact
        // almost anything and defeats the point of the filter.
        if s_trim.chars().count() < 3 {
            continue;
        }
        if lower.contains(&s_trim.to_lowercase()) {
            return REFUSAL.to_string();
        }
    }

    const LEAK_PATTERNS: &[&str] =
        &["la respuesta es", "la respuesta correcta es", "la opción correcta es", "debes elegir la opción", "la rama correcta es"];
    if LEAK_PATTERNS.iter().any(|p| lower.contains(p)) {
        return REFUSAL.to_string();
    }

    reply.to_string()
}

#[cfg(test)]
mod tests;
