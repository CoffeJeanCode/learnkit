//! Deterministic stand-ins for everything the model is NOT asked to
//! fabricate (diagnostic summary, micromodules, fallback notebookâ€¦).
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
        last_message.chars().take(80).collect()
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

/// Splits a week's total hours into EXACTLY 2 homogeneous sessions (the
/// anti-monolith dosing rule — see `grounding::micromodule_violations`),
/// used by the force-close fallback so even a synthesized syllabus respects
/// the same rule a real `propose_syllabus_plan` call is held to.
pub(super) fn synth_micromodules(total_hours: f32) -> Vec<Micromodule> {
    let half = total_hours / 2.0;
    (1..=2)
        .map(|i| Micromodule {
            label: format!("Sesión {i}"),
            hours: half,
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

/// Fallback battery for the force-close path — generated alongside the
/// syllabus (see `synth_roadmap_package`), never as part of the notebook.
pub(super) fn synth_diagnostic_battery(profile: &LearnerProfileCard) -> DiagnosticBattery {
    DiagnosticBattery {
        goal_alignment: format!("Mide el punto de partida real frente a la meta: {}", profile.target_goal),
        questions: vec![
            DiagnosticQuestion {
                dimension: DiagnosticDimension::Intuition,
                prompt: format!("En términos cotidianos, ¿qué asocias primero con {}?", profile.topic),
                options: vec!["Una idea concreta".to_string(), "No tengo ninguna intuición todavía".to_string()],
                correct_option: "Una idea concreta".to_string(),
                diagnostic_insight: "Elegir la segunda opción indica que hace falta partir de una analogía cotidiana, sin términos técnicos".to_string(),
            },
            DiagnosticQuestion {
                dimension: DiagnosticDimension::Mechanics,
                prompt: "Si cambias una variable clave del tema, ¿esperas que el resultado cambie de forma predecible?".to_string(),
                options: vec!["Sí, hay una relación clara".to_string(), "No estoy seguro".to_string()],
                correct_option: "Sí, hay una relación clara".to_string(),
                diagnostic_insight: "Elegir la segunda opción indica que aún falta ver cómo interactúan las variables del tema".to_string(),
            },
            DiagnosticQuestion {
                dimension: DiagnosticDimension::CriticalCase,
                prompt: format!("Ante un caso límite típico de {}, ¿confías en identificar el error más común?", profile.topic),
                options: vec!["Sí".to_string(), "No todavía".to_string()],
                correct_option: "Sí".to_string(),
                diagnostic_insight: "Elegir la segunda opción marca dónde debe reforzar la práctica antes del objetivo final".to_string(),
            },
        ],
    }
}

