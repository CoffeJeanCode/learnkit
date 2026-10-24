//! Prompt-side data encoding: TOON (Token-Oriented Object Notation, spec
//! v4.4) for structured context embedded in prompt text. Input-only — model
/// OUTPUT stays native tool-JSON (see tools/*).
use serde::Serialize;

/// One-line orientation for weaker models reading the TOON payload. Stated
/// ONCE in each TOON-consuming agent's system prompt (see [`with_toon_hint`]),
/// not repeated in every user input: the system prompt is the cacheable prefix.
pub(crate) const TOON_HINT: &str = "Los datos del mensaje del usuario están en formato TOON (no JSON): la indentación \
                                   anida niveles y una cabecera como nombre[3]: encabeza 3 filas de datos con sus columnas.";

/// Appends [`TOON_HINT`] to a system prompt, once.
pub(crate) fn with_toon_hint(system_prompt: &str) -> String {
    format!("{system_prompt}\n\n{TOON_HINT}")
}

/// Encodes a value as TOON for embedding in a prompt. Falls back to COMPACT
/// JSON (never pretty, never empty — the old `.unwrap_or_default()` could
/// emit `""`) if encoding fails, logging the failure: a prompt payload must
/// always carry the data, even in a degraded format.
pub(crate) fn toon_payload<T: Serialize>(value: &T) -> String {
    match toon_format::encode_default(value) {
        Ok(toon) => toon,
        Err(e) => {
            tracing::warn!(error = %e, "TOON encoding failed; falling back to compact JSON for prompt payload");
            serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Representative block-generation context: nested objects, an array of
    /// objects (tabular in TOON), Spanish/unicode text, and every delimiter
    /// character a naive encoder could mangle.
    fn sample_payload() -> serde_json::Value {
        serde_json::json!({
            "course": { "title": "Pensamiento computacional", "targetGoal": "Resolver problemas con abstracción" },
            "class": { "title": "Introducción a las pilas", "classNumber": 3 },
            "blocksSoFar": ["anchored_micro_theory", "interactive_prediction_gate"],
            "contentSoFar": [
                { "blockType": "anchored_micro_theory", "content": { "title": "La pila: LIFO", "analogy": "Un plato de panqueques: el último en entrar, el primero en salir." } },
                { "blockType": "interactive_prediction_gate", "content": { "question": "¿Qué imprime pop() tras push(1); push(2)?" } },
            ],
            "tricky": "colons: inside | pipes\nand a real newline too",
            "emoji": "la recursión 🌀 sin base",
        })
    }

    #[test]
    fn toon_payload_round_trips_back_to_the_same_json_value() {
        let original = sample_payload();
        let encoded = toon_payload(&original);

        let decoded: serde_json::Value = toon_format::decode_default(&encoded).expect("TOON payload decodes");

        assert_eq!(decoded, serde_json::to_value(&original).expect("serializes"));
    }

    #[test]
    fn toon_payload_is_strictly_smaller_than_pretty_json() {
        let original = sample_payload();
        let pretty_len = serde_json::to_string_pretty(&original).expect("pretty").len();
        let compact_len = serde_json::to_string(&original).expect("compact").len();
        let toon_len = toon_payload(&original).len();

        assert!(toon_len < pretty_len, "TOON ({toon_len}) must beat pretty JSON ({pretty_len})");
        // Informational bound against compact JSON — asserted only while it
        // holds; TOON's edge comes from dropping braces/quotes, which dense
        // payloads can negate.
        assert!(toon_len <= compact_len, "TOON ({toon_len}) vs compact JSON ({compact_len})");
    }

    #[test]
    fn every_toon_consuming_agent_states_the_hint_once_and_no_input_repeats_it() {
        use crate::agents::{
            analyst_agent, block_generator_agent, closure_feedback_grader_agent, context_questions_agent, lexical_assistant_agent, notebook_gate_grader_agent,
            pedagogical_critic_agent, plan_outline_agent, plan_week_agent, research_curator_agent, research_planner_agent, roadmap_agent,
        };
        for def in [
            analyst_agent::definition(),
            block_generator_agent::definition(),
            closure_feedback_grader_agent::definition(),
            context_questions_agent::definition(),
            lexical_assistant_agent::definition(),
            notebook_gate_grader_agent::definition(),
            pedagogical_critic_agent::definition(),
            plan_outline_agent::definition(),
            plan_week_agent::definition(),
            research_curator_agent::definition(),
            research_planner_agent::definition(),
            roadmap_agent::definition(),
        ] {
            assert_eq!(def.system_prompt.matches(TOON_HINT).count(), 1, "{} must carry the TOON hint exactly once", def.id);
        }
        // The inputs are built from these files; none may interpolate the hint again.
        for (name, source) in [
            ("notebook_service/render.rs", include_str!("application/notebook_service/render.rs")),
            ("roadmap_service/render.rs", include_str!("application/roadmap_service/render.rs")),
            ("lexical_assistant_service.rs", include_str!("application/lexical_assistant_service.rs")),
        ] {
            let production = source.split("#[cfg(test)]").next().unwrap_or(source);
            assert!(!production.contains("TOON_HINT"), "{name} must not repeat the hint in user inputs");
        }
    }

    /// The documented encode error (>256 nesting levels) must degrade to
    /// compact JSON — never empty, never a panic.
    #[test]
    fn toon_payload_falls_back_to_compact_json_when_encoding_errors() {
        let mut deep = serde_json::json!(0);
        for _ in 0..300 {
            deep = serde_json::json!({ "n": deep });
        }
        assert!(toon_format::encode_default(&deep).is_err(), "300 levels exceed the documented depth limit");

        let encoded = toon_payload(&deep);

        assert_eq!(encoded, serde_json::to_string(&deep).expect("compact fallback"));
    }
}
