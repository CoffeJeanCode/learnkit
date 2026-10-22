//! Deterministic stand-ins for everything the model is NOT asked to
//! fabricate (diagnostic summary, micromodules, fallback notebookâ€¦).
use super::grounding::session_count_bounds;
use super::*;

pub(super) fn synth_profile_card(draft: &serde_json::Value) -> LearnerProfileCard {
    let last_message = draft
        .get("messages")
        .and_then(|m| m.as_array())
        .and_then(|a| a.last())
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let topic = if last_message.is_empty() {
        "Tema por definir".to_string()
    } else {
        clean_topic(last_message)
    };
    let timeframe_weeks = 4u16;
    let weekly_commitment_hours = 3.0f32;
    LearnerProfileCard {
        topic,
        target_goal: "Meta de aprendizaje general".to_string(),
        timeframe_weeks,
        weekly_commitment_hours,
        total_available_hours: timeframe_weeks as f32 * weekly_commitment_hours,
        // No reliable signal in `draft` to pick absolute_zero vs. applied —
        // theoretical_foundations is the safe middle default for a
        // force-closed session (never assumes zero, never assumes fluency).
        entry_level: EntryLevel::TheoreticalFoundations,
    }
}

pub(super) fn synth_diagnostic_card() -> DiagnosticSummaryCard {
    DiagnosticSummaryCard {
        core_focus: "Comprensión funcional del tema, paso a paso".to_string(),
        identified_needs: vec!["Entender la idea general antes de los detalles".to_string(), "Practicar con ejemplos concretos".to_string()],
        learning_strategy: "Aprendizaje guiado con ejemplos prácticos".to_string(),
    }
}

/// Splits a week's total hours into the minimum number of equal sessions the
/// validator allows (see `grounding::session_count_bounds`), so each stays
/// under 4h and the spread is zero. Used by the force-close fallback so even a
/// synthesized syllabus respects the same rule a real `propose_syllabus_plan`
/// call is held to.
pub(super) fn synth_micromodules(total_hours: f32) -> Vec<Micromodule> {
    let sessions = session_count_bounds(total_hours).0;
    let per_session = total_hours / sessions as f32;
    (1..=sessions)
        .map(|i| Micromodule {
            label: format!("Sesión {i}"),
            hours: per_session,
            focus: Some(format!("Conceptos centrales de la sesión {i}, aplicados a un caso concreto del curso")),
            deliverable: Deliverable {
                artifact_type: DeliverableArtifactType::WorkingDemo,
                description: format!("Entregable verificable de la sesión {i}"),
            },
            objective: Some(format!("Aplicar los conceptos clave de la sesión {i} a un caso concreto del curso")),
            interactive_blocks: vec!["socratic_prediction".to_string(), "hands_on_mission".to_string(), "metacognitive_closure".to_string()],
        })
        .collect()
}

pub(super) fn synth_roadmap_package(profile: &LearnerProfileCard) -> RoadmapSyllabusPackage {
    let total_weeks = profile.timeframe_weeks.max(1);
    let milestones = (1..=total_weeks)
        .map(|week| Milestone {
            week,
            title: format!("Semana {week}: avance progresivo"),
            deliverable: "Entregable de la semana".to_string(),
            weekly_goal: Some("Avanzar un paso concreto hacia la meta declarada".to_string()),
            micromodules: synth_micromodules(profile.weekly_commitment_hours),
        })
        .collect();
    let capstone_project = CapstoneProject {
        title: format!("Proyecto terminal: {}", profile.topic),
        description: format!("Aplicar lo aprendido en un escenario real que demuestre {}", profile.target_goal),
        verifiable_evidence: "Artefacto final entregado y revisado contra la meta declarada".to_string(),
    };
    RoadmapSyllabusPackage {
        course_title: profile.topic.clone(),
        total_weeks,
        pace_hours_per_week: profile.weekly_commitment_hours,
        milestones,
        capstone_project,
    }
}

// Deterministic stand-ins for the force-close path (profile card, summary,
// syllabus). NOTE: this module deliberately does NOT synthesize a diagnostic
// battery — the old `synth_diagnostic_battery` fabricated canned
// self-perception questions graded right/wrong, which real declared-beginner
// users experienced as absurd. Force-close now seals with
// `diagnostic_battery: None` (see `flow::force_close_session`): the frontend
// renders no battery card in that case, and silence beats nonsense.

/// Max length (chars) of the synthesized `topic` derived from the student's
/// raw last message.
const MAX_TOPIC_CHARS: usize = 80;

/// Leading intent phrases `clean_topic` strips from the raw message. Only the
/// VOLITION part is stripped — the action verb that follows ("aprender",
/// "montar"…) IS the topic and must survive ("Quiero montar mi primer
/// servidor" → "Montar mi primer servidor", "Quiero aprender Rust" →
/// "Aprender Rust"). Ordered longest-first so a phrase is never shadowed by a
/// shorter one that happens to be its prefix.
const TOPIC_INTENT_PREFIXES: [&str; 7] =
    ["me gustaría ", "necesito ", "quiero ", "quisiera ", "pretendo ", "voy a ", "aspiro a "];

/// Trailing skill-disclaimer clauses `clean_topic` cuts off ("…, y no tengo
/// nada de conocimiento", "… desde cero", …). A declared beginner's raw
/// message carried these into the course topic verbatim before; the topic is
/// the goal, not the disclaimer.
const TOPIC_SKILL_DISCLAIMERS: [&str; 12] = [
    ", y no tengo nada de conocimiento",
    " y no tengo nada de conocimiento",
    ", y no tengo conocimientos",
    " y no tengo conocimientos",
    ", y no sé nada",
    " y no sé nada",
    ", no tengo nada de conocimiento",
    " desde cero total",
    " desde ceros",
    " desde cero",
    " sin experiencia",
    " sin conocimientos previos",
];

/// Case-insensitive (ASCII folding) substring search returning a BYTE index.
/// Safe to slice the original `&str` at the returned index: a byte-exact
/// (case-folded) match can only start at a char boundary, and the needle
/// length is irrelevant to the caller — it just cuts there.
fn find_ascii_fold(haystack: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    (0..=h.len() - n.len()).find(|&i| h[i..i + n.len()].eq_ignore_ascii_case(n))
}

/// Deterministic topic derivation for the force-close fallback: strips ONE
/// trailing skill-disclaimer clause (at its earliest occurrence) and ONE
/// leading intent prefix from the student's raw last message, trims
/// whitespace/punctuation, capitalizes the first char (char-safe) and
/// truncates to [`MAX_TOPIC_CHARS`]. Never returns empty — falls back to the
/// trimmed raw message.
pub(super) fn clean_topic(raw: &str) -> String {
    let mut t = raw.trim();
    // The disclaimer first, so a comma-led clause ("…servidor, y no tengo
    // nada de conocimiento") is cut before prefix handling runs.
    if let Some(idx) = TOPIC_SKILL_DISCLAIMERS.iter().filter_map(|d| find_ascii_fold(t, d)).min() {
        t = &t[..idx];
    }
    for p in TOPIC_INTENT_PREFIXES {
        // `eq_ignore_ascii_case` on the byte prefix — never `to_lowercase()`,
        // which can change byte length for non-ASCII input. A match implies
        // the same byte length, and the (ASCII-terminated) prefix end is a
        // char boundary of `t`, so the slice below is safe.
        if t.len() >= p.len() && t.as_bytes()[..p.len()].eq_ignore_ascii_case(p.as_bytes()) {
            t = t[p.len()..].trim_start();
            break; // ONE prefix only
        }
    }
    let t = t.trim_matches(|c: char| c.is_whitespace() || matches!(c, ',' | '.' | ';'));
    if t.is_empty() {
        return raw.trim().chars().take(MAX_TOPIC_CHARS).collect();
    }
    let mut chars = t.chars();
    let capitalized: String = match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    };
    capitalized.chars().take(MAX_TOPIC_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_topic_strips_the_intent_prefix_and_the_skill_disclaimer() {
        assert_eq!(
            clean_topic("Quiero montar mi primer servidor, y no tengo nada de conocimiento"),
            "Montar mi primer servidor"
        );
    }

    #[test]
    fn clean_topic_keeps_the_action_verb_as_part_of_the_topic() {
        assert_eq!(clean_topic("Quiero aprender Rust"), "Aprender Rust");
    }

    #[test]
    fn clean_topic_passes_a_plain_topic_through() {
        assert_eq!(clean_topic("Redes domésticas"), "Redes domésticas");
    }

    #[test]
    fn clean_topic_falls_back_to_the_raw_message_when_stripping_empties_it() {
        assert_eq!(clean_topic("Quiero "), "Quiero");
    }

    #[test]
    fn clean_topic_truncates_to_80_chars() {
        let long = format!("Quiero {}", "a".repeat(200));
        assert_eq!(clean_topic(&long).chars().count(), MAX_TOPIC_CHARS);
    }

    #[test]
    fn synthesized_weeks_satisfy_the_session_rules_for_any_pace() {
        use super::super::grounding::{MAX_SESSION_HOUR_SPREAD, MAX_SESSION_HOURS, micromodule_violations};
        for pace in [3.0f32, 8.0, 14.0, 20.0, 21.0, 80.0] {
            let modules = synth_micromodules(pace);
            let (min, max) = session_count_bounds(pace);
            assert!((min..=max).contains(&modules.len()), "pace {pace}: {} sessions outside {min}..={max}", modules.len());
            assert_eq!(modules.len(), min, "pace {pace}: the fallback uses the minimum session count");
            assert!(modules.iter().all(|m| m.hours > 0.0 && m.hours <= MAX_SESSION_HOURS), "pace {pace}: a session exceeds 4h");
            let sum: f32 = modules.iter().map(|m| m.hours).sum();
            assert!((sum - pace).abs() <= 0.5, "pace {pace}: sum {sum}");
            let hours: Vec<f32> = modules.iter().map(|m| m.hours).collect();
            let spread = hours.iter().cloned().fold(f32::MIN, f32::max) - hours.iter().cloned().fold(f32::MAX, f32::min);
            assert!(spread <= MAX_SESSION_HOUR_SPREAD);
            // The count/hours/sum/spread rules must all pass through the real validator.
            let violations = micromodule_violations(1, pace, &modules);
            assert!(violations.is_empty(), "pace {pace}: {violations:?}");
        }
    }
}
