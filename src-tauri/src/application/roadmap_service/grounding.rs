//! Grounding rules for the roadmap turn: every validator the model's output
//! must pass (profile card, syllabus, diagnostic battery) plus the builders
//! those validators read from.
use super::*;

pub(super) fn build_profile_card(a: &DiagnosticAssessmentArgs) -> LearnerProfileCard {
    LearnerProfileCard {
        topic: a.topic.clone(),
        target_goal: a.target_goal.clone(),
        timeframe_weeks: a.timeframe_weeks,
        weekly_commitment_hours: a.weekly_commitment_hours,
        // Computed here, never asked of the model — arithmetic grounding
        // failures on this exact field were a recurring bug when it was a
        // model-supplied value.
        total_available_hours: a.timeframe_weeks as f32 * a.weekly_commitment_hours,
        entry_level: a.entry_level,
    }
}

/// Content-level grounding for a `LearnerProfileCard`: empty or
/// out-of-range fields. No arithmetic check needed anymore —
/// `totalAvailableHours` is computed, never model-supplied. `entryLevel` is
/// a closed enum now — nothing to validate there, an invalid value simply
/// fails to parse before reaching here.
pub(super) fn profile_grounding_violations(card: &LearnerProfileCard) -> Vec<String> {
    let mut v = Vec::new();
    if card.topic.trim().is_empty() {
        v.push("topic está vacío".to_string());
    }
    if card.target_goal.trim().is_empty() {
        v.push("targetGoal está vacío".to_string());
    }
    if !(1..=52).contains(&card.timeframe_weeks) {
        v.push(format!("timeframeWeeks ({}) fuera de rango razonable (1-52)", card.timeframe_weeks));
    }
    if !(0.5..=80.0).contains(&card.weekly_commitment_hours) {
        v.push(format!("weeklyCommitmentHours ({}) fuera de rango razonable (0.5-80)", card.weekly_commitment_hours));
    }
    v
}

/// Content-level grounding for a `RoadmapSyllabusPackage`, grounded against
/// the assessment already confirmed: week count and pace must match — a
/// model that quietly drifts from the learner's declared timeframe or
/// availability defeats the whole premise.
/// The fixed 5-item methodological catalog (see `agents::roadmap_agent`) —
/// every `Micromodule.interactive_blocks` entry must be one of these; unlike
/// the retired flat `Milestone.interactive_blocks`, this vocabulary IS
/// closed, since the spec ties each choice to a specific epistemological
/// role rather than letting the model invent free-form slugs.
pub(super) const VALID_INTERACTIVE_BLOCKS: [&str; 5] =
    ["interactive_visual_anchor", "socratic_prediction", "error_audit_challenge", "hands_on_mission", "metacognitive_closure"];

/// Weak but cheap heuristic for "vague, unverifiable deliverable" — the spec
/// bans exactly these kinds of verb phrases (comprehension/exposure verbs
/// with no checkable artifact) in favor of authentic ones ("un circuito
/// simulado de 2 qubits con histograma analizado").
pub(super) const VAGUE_DELIVERABLE_PHRASES: [&str; 5] =
    ["comprender la teoría", "leer sobre el tema", "entender el concepto", "familiarizarse con", "aprender sobre"];

pub(super) fn is_vague_deliverable(text: &str) -> bool {
    let lower = text.to_lowercase();
    VAGUE_DELIVERABLE_PHRASES.iter().any(|p| lower.contains(p))
}

pub(super) fn micromodule_violations(week: u16, pace_hours_per_week: f32, modules: &[Micromodule]) -> Vec<String> {
    let mut v = Vec::new();
    if !(1..=3).contains(&modules.len()) {
        v.push(format!("la semana {week} tiene {} micromodules, debe tener 1-3", modules.len()));
    }
    let mut total_hours = 0.0f32;
    for m in modules {
        if m.label.trim().is_empty() {
            v.push(format!("un micromodule de la semana {week} tiene label vacío"));
        }
        if !(m.hours > 0.0 && m.hours <= 5.0) {
            v.push(format!(
                "el micromodule \"{}\" de la semana {week} tiene {} horas — debe ser mayor a 0 y como máximo 5 (anti-monolito)",
                m.label, m.hours
            ));
        }
        if m.deliverable.trim().is_empty() {
            v.push(format!("el micromodule \"{}\" de la semana {week} tiene deliverable vacío", m.label));
        } else if is_vague_deliverable(&m.deliverable) {
            v.push(format!(
                "el deliverable \"{}\" del micromodule \"{}\" (semana {week}) es vago — debe ser un artefacto verificable",
                m.deliverable, m.label
            ));
        }
        match m.objective.as_deref().map(str::trim) {
            None | Some("") => {
                v.push(format!(
                    "el micromodule \"{}\" de la semana {week} no tiene objective — es la frase que dice qué sabrá \
                     hacer el estudiante al terminarlo (observable, en infinitivo), distinta del deliverable",
                    m.label
                ));
            }
            Some(objective) if is_vague_deliverable(objective) => {
                v.push(format!(
                    "el objective \"{}\" del micromodule \"{}\" (semana {week}) es vago — describe una capacidad \
                     observable, no \"comprender la teoría\"",
                    objective, m.label
                ));
            }
            Some(_) => {}
        }
        if !(3..=4).contains(&m.interactive_blocks.len()) {
            v.push(format!(
                "el micromodule \"{}\" de la semana {week} tiene {} interactiveBlocks, debe tener 3-4",
                m.label,
                m.interactive_blocks.len()
            ));
        }
        for b in &m.interactive_blocks {
            if !VALID_INTERACTIVE_BLOCKS.contains(&b.as_str()) {
                v.push(format!(
                    "el bloque \"{b}\" del micromodule \"{}\" (semana {week}) no está en el catálogo válido {VALID_INTERACTIVE_BLOCKS:?}",
                    m.label
                ));
            }
        }
        total_hours += m.hours;
    }
    if (total_hours - pace_hours_per_week).abs() > 0.5 {
        v.push(format!(
            "la suma de horas de los micromodules de la semana {week} ({total_hours}) no coincide con paceHoursPerWeek ({pace_hours_per_week})"
        ));
    }
    v
}

pub(super) fn syllabus_violations(profile: &LearnerProfileCard, syllabus: &RoadmapSyllabusPackage) -> Vec<String> {
    let mut v = Vec::new();
    if syllabus.course_title.trim().is_empty() {
        v.push("courseTitle está vacío".to_string());
    }
    if syllabus.milestones.is_empty() {
        v.push("milestones está vacío".to_string());
    }
    if syllabus.milestones.len() as u16 != syllabus.total_weeks {
        v.push(format!("milestones tiene {} elementos pero totalWeeks es {}", syllabus.milestones.len(), syllabus.total_weeks));
    }
    if syllabus.total_weeks != profile.timeframe_weeks {
        v.push(format!(
            "totalWeeks ({}) no coincide con timeframeWeeks del diagnóstico ({})",
            syllabus.total_weeks, profile.timeframe_weeks
        ));
    }
    if (syllabus.pace_hours_per_week - profile.weekly_commitment_hours).abs() > 0.5 {
        v.push(format!(
            "paceHoursPerWeek ({}) no coincide con weeklyCommitmentHours del diagnóstico ({})",
            syllabus.pace_hours_per_week, profile.weekly_commitment_hours
        ));
    }
    for m in &syllabus.milestones {
        if m.title.trim().is_empty() {
            v.push(format!("la semana {} tiene título vacío", m.week));
        }
        if m.deliverable.trim().is_empty() {
            v.push(format!("la semana {} tiene entregable vacío", m.week));
        } else if is_vague_deliverable(&m.deliverable) {
            v.push(format!("el entregable de la semana {} (\"{}\") es vago — debe ser un artefacto verificable", m.week, m.deliverable));
        }
        v.extend(micromodule_violations(m.week, syllabus.pace_hours_per_week, &m.micromodules));
    }
    let mut weeks: Vec<u16> = syllabus.milestones.iter().map(|m| m.week).collect();
    weeks.sort_unstable();
    let expected: Vec<u16> = (1..=syllabus.total_weeks).collect();
    if weeks != expected {
        v.push(format!("los números de semana de milestones ({weeks:?}) no son 1..totalWeeks consecutivos"));
    }
    v
}

/// Content-level grounding for a `DiagnosticBattery`: empty fields, out-of-
/// range question count, or a `correctOption` that isn't even one of its
/// own `options` — the same category of drift check as `syllabus_violations`.
/// Justification connectors that must never appear inside an option's own
/// text — the "why" belongs exclusively in `diagnosticInsight`, shown only
/// AFTER the student answers. An option that already explains itself lets a
/// student pick it by spotting the explanation, not by holding the right
/// mental model.
pub(super) const JUSTIFICATION_CONNECTORS: [&str; 5] = ["porque", "ya que", "debido a", "puesto que", "lo cual significa"];

pub(super) fn option_has_embedded_justification(option: &str) -> bool {
    let lower = option.to_lowercase();
    JUSTIFICATION_CONNECTORS.iter().any(|c| lower.contains(c))
}

pub(super) fn diagnostic_battery_violations(battery: &DiagnosticBattery) -> Vec<String> {
    let mut v = Vec::new();
    if battery.goal_alignment.trim().is_empty() {
        v.push("diagnosticBattery.goalAlignment está vacío".to_string());
    }
    if !(3..=4).contains(&battery.questions.len()) {
        v.push(format!("diagnosticBattery.questions tiene {} elementos, debe tener 3-4", battery.questions.len()));
    }
    for (i, q) in battery.questions.iter().enumerate() {
        if !(2..=4).contains(&q.options.len()) {
            v.push(format!("diagnosticBattery.questions[{i}] tiene {} opciones, debe tener 2-4", q.options.len()));
        }
        if !q.options.contains(&q.correct_option) {
            v.push(format!("diagnosticBattery.questions[{i}].correctOption \"{}\" no está en options", q.correct_option));
        }
        let lengths: Vec<usize> = q.options.iter().map(|o| o.chars().count()).collect();
        if let (Some(&max_len), Some(&min_len)) = (lengths.iter().max(), lengths.iter().min()) {
            if max_len > 0 && (max_len - min_len) as f32 / max_len as f32 > 0.15 {
                v.push(format!(
                    "diagnosticBattery.questions[{i}] tiene opciones con longitudes muy dispares (sesgo de longitud) — \
                     min {min_len} / max {max_len} caracteres, variación permitida < 15%"
                ));
            }
        }
        for opt in &q.options {
            if option_has_embedded_justification(opt) {
                v.push(format!(
                    "diagnosticBattery.questions[{i}] tiene una opción (\"{opt}\") con su propia justificación — \
                     eso pertenece a diagnosticInsight, nunca al texto de la opción"
                ));
            }
        }
    }
    let dimensions: std::collections::HashSet<DiagnosticDimension> = battery.questions.iter().map(|q| q.dimension).collect();
    for required in [DiagnosticDimension::Intuition, DiagnosticDimension::Mechanics, DiagnosticDimension::CriticalCase] {
        if !dimensions.contains(&required) {
            v.push(format!("diagnosticBattery.questions no cubre la dimensión obligatoria {required:?}"));
        }
    }
    v
}

/// Gate 2's completion condition for Gate 3a to fire: either the student
/// declared `AbsoluteZero` (the battery is skipped entirely by design — see
/// `agents::roadmap_agent`'s "COMPORTAMIENTO ANTE NIVEL CERO"), or a battery
/// was generated AND fully answered.
pub(super) fn battery_satisfied(profile: &LearnerProfileCard, pending: &Option<DiagnosticBatteryState>) -> bool {
    profile.entry_level == EntryLevel::AbsoluteZero || pending.as_ref().is_some_and(|p| p.fully_answered())
}

