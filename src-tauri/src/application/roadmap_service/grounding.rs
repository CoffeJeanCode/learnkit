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

/// The minimum `description` length required when `artifactType` is `other`
/// — the one bucket in `DeliverableArtifactType` that isn't already
/// self-describing, so it's the only place a vague one-liner could otherwise
/// hide behind a technically-non-empty string.
pub(super) const OTHER_DELIVERABLE_MIN_LEN: usize = 20;

/// Content-level grounding for a `Micromodule::deliverable`: replaces a
/// denylist-only check (`is_vague_deliverable` used to be the WHOLE story)
/// with a structural bar too. `description` must be non-empty and non-vague
/// for every `artifactType`; when `artifactType` is `other` it must ALSO
/// clear `OTHER_DELIVERABLE_MIN_LEN`, so "otro: algo" can't be the whole
/// answer. `context` is the human-readable location used in every message
/// (e.g. `la sesión "X" de la semana 1`).
pub(super) fn deliverable_violations(context: &str, deliverable: &Deliverable) -> Vec<String> {
    let mut v = Vec::new();
    let desc = deliverable.description.trim();
    if desc.is_empty() {
        v.push(format!("{context} tiene deliverable vacío"));
    } else if is_vague_deliverable(desc) {
        v.push(format!("el deliverable \"{desc}\" de {context} es vago — debe ser un artefacto verificable"));
    } else if deliverable.artifact_type == DeliverableArtifactType::Other && desc.chars().count() < OTHER_DELIVERABLE_MIN_LEN {
        v.push(format!(
            "el deliverable \"{desc}\" de {context} usa artifactType \"other\" (el único escape del catálogo cerrado) pero \
             tiene menos de {OTHER_DELIVERABLE_MIN_LEN} caracteres — descríbelo con más detalle"
        ));
    } else {
        check_student_facing_text(&mut v, &format!("deliverable de {context}"), desc);
    }
    v
}

/// The `interactive_blocks` catalog's own vocabulary (snake_case, as stored,
/// and spaced, as it'd read if naively humanized) — the ONE thing this check
/// hard-rejects on. Deliberately narrow: generic process jargon ("sprint",
/// "compuerta", "gate"...) is instead a soft instruction in
/// `agents::roadmap_agent`'s "CERO META-LENGUAJE" section, not a hard
/// validator here, because those words can be legitimate domain content
/// (e.g. "compuertas lógicas" in a digital-electronics syllabus) — rejecting
/// on them would false-positive a valid proposal into an endless retry loop.
/// `interactive_blocks` itself is exempt from this check: it's pure backend
/// metadata for `notebook_generator`, never rendered by the frontend — see
/// `Micromodule::interactive_blocks`.
pub(super) const LEAKED_JARGON_PHRASES: [&str; 10] = [
    "socratic_prediction",
    "socratic prediction",
    "error_audit_challenge",
    "error audit challenge",
    "interactive_visual_anchor",
    "interactive visual anchor",
    "hands_on_mission",
    "hands-on mission",
    "metacognitive_closure",
    "metacognitive closure",
];

pub(super) fn has_leaked_jargon(text: &str) -> bool {
    let lower = text.to_lowercase();
    LEAKED_JARGON_PHRASES.iter().any(|p| lower.contains(p))
}

fn check_student_facing_text(v: &mut Vec<String>, field_name: &str, text: &str) {
    if has_leaked_jargon(text) {
        v.push(format!("\"{text}\" ({field_name}) expone jerga metodológica interna — el estudiante nunca debe verla"));
    }
}

/// EXACTLY 2 sessions per week, homogeneous in hours (max 1h of spread
/// between them) and never more than 4h each — the anti-monolito dosing rule
/// (see `agents::roadmap_agent`'s "DOSIFICACIÓN HORARIA HOMOGÉNEA").
pub(super) const MAX_SESSION_HOURS: f32 = 4.0;
pub(super) const MAX_SESSION_HOUR_SPREAD: f32 = 1.0;

pub(super) fn micromodule_violations(week: u16, pace_hours_per_week: f32, modules: &[Micromodule]) -> Vec<String> {
    let mut v = Vec::new();
    if modules.len() != 2 {
        v.push(format!("la semana {week} tiene {} sesiones, debe tener EXACTAMENTE 2 (anti-monolito)", modules.len()));
    }
    let mut total_hours = 0.0f32;
    for m in modules {
        if m.label.trim().is_empty() {
            v.push(format!("una sesión de la semana {week} tiene label vacío"));
        } else {
            check_student_facing_text(&mut v, &format!("label de la semana {week}"), &m.label);
        }
        if !(m.hours > 0.0 && m.hours <= MAX_SESSION_HOURS) {
            v.push(format!(
                "la sesión \"{}\" de la semana {week} tiene {} horas — debe ser mayor a 0 y como máximo {MAX_SESSION_HOURS} (anti-monolito)",
                m.label, m.hours
            ));
        }
        v.extend(deliverable_violations(&format!("la sesión \"{}\" de la semana {week}", m.label), &m.deliverable));
        match m.focus.as_deref().map(str::trim) {
            None | Some("") => {
                v.push(format!(
                    "la sesión \"{}\" de la semana {week} no tiene focus — los conceptos centrales y la relación \
                     causa-efecto que explora, aterrizados en la fricción real que resuelve",
                    m.label
                ));
            }
            Some(focus) if is_vague_deliverable(focus) => {
                v.push(format!("el focus \"{focus}\" de la sesión \"{}\" (semana {week}) es vago", m.label));
            }
            Some(focus) => check_student_facing_text(&mut v, &format!("focus de la sesión \"{}\" (semana {week})", m.label), focus),
        }
        match m.objective.as_deref().map(str::trim) {
            None | Some("") => {
                v.push(format!(
                    "la sesión \"{}\" de la semana {week} no tiene objective — es la frase que dice qué sabrá \
                     hacer el estudiante al terminarla (observable, en infinitivo), distinta del deliverable",
                    m.label
                ));
            }
            Some(objective) if is_vague_deliverable(objective) => {
                v.push(format!(
                    "el objective \"{}\" de la sesión \"{}\" (semana {week}) es vago — describe una capacidad \
                     observable, no \"comprender la teoría\"",
                    objective, m.label
                ));
            }
            Some(objective) => check_student_facing_text(&mut v, &format!("objective de la sesión \"{}\" (semana {week})", m.label), objective),
        }
        if !(3..=4).contains(&m.interactive_blocks.len()) {
            v.push(format!(
                "la sesión \"{}\" de la semana {week} tiene {} interactiveBlocks, debe tener 3-4",
                m.label,
                m.interactive_blocks.len()
            ));
        }
        for b in &m.interactive_blocks {
            if !VALID_INTERACTIVE_BLOCKS.contains(&b.as_str()) {
                v.push(format!(
                    "el bloque \"{b}\" de la sesión \"{}\" (semana {week}) no está en el catálogo válido {VALID_INTERACTIVE_BLOCKS:?}",
                    m.label
                ));
            }
        }
        total_hours += m.hours;
    }
    if modules.len() == 2 {
        let spread = (modules[0].hours - modules[1].hours).abs();
        if spread > MAX_SESSION_HOUR_SPREAD {
            v.push(format!(
                "las 2 sesiones de la semana {week} no son homogéneas — {}h vs {}h, diferencia de {spread}h (máximo {MAX_SESSION_HOUR_SPREAD}h)",
                modules[0].hours, modules[1].hours
            ));
        }
    }
    if (total_hours - pace_hours_per_week).abs() > 0.5 {
        v.push(format!(
            "la suma de horas de las sesiones de la semana {week} ({total_hours}) no coincide con paceHoursPerWeek ({pace_hours_per_week})"
        ));
    }
    v
}

pub(super) fn syllabus_violations(profile: &LearnerProfileCard, syllabus: &RoadmapSyllabusPackage) -> Vec<String> {
    let mut v = Vec::new();
    if syllabus.course_title.trim().is_empty() {
        v.push("courseTitle está vacío".to_string());
    } else {
        check_student_facing_text(&mut v, "courseTitle", &syllabus.course_title);
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
        } else {
            check_student_facing_text(&mut v, &format!("título de la semana {}", m.week), &m.title);
        }
        if m.deliverable.trim().is_empty() {
            v.push(format!("la semana {} tiene entregable vacío", m.week));
        } else if is_vague_deliverable(&m.deliverable) {
            v.push(format!("el entregable de la semana {} (\"{}\") es vago — debe ser un artefacto verificable", m.week, m.deliverable));
        } else {
            check_student_facing_text(&mut v, &format!("entregable de la semana {}", m.week), &m.deliverable);
        }
        match m.weekly_goal.as_deref().map(str::trim) {
            None | Some("") => {
                v.push(format!(
                    "la semana {} no tiene weeklyGoal — la meta semanal que dice qué problema resuelve y qué \
                     capacidad desbloquea (Backward Design)",
                    m.week
                ));
            }
            Some(goal) if is_vague_deliverable(goal) => {
                v.push(format!("el weeklyGoal de la semana {} (\"{goal}\") es vago", m.week));
            }
            Some(goal) => check_student_facing_text(&mut v, &format!("weeklyGoal de la semana {}", m.week), goal),
        }
        v.extend(micromodule_violations(m.week, syllabus.pace_hours_per_week, &m.micromodules));
    }
    let mut weeks: Vec<u16> = syllabus.milestones.iter().map(|m| m.week).collect();
    weeks.sort_unstable();
    let expected: Vec<u16> = (1..=syllabus.total_weeks).collect();
    if weeks != expected {
        v.push(format!("los números de semana de milestones ({weeks:?}) no son 1..totalWeeks consecutivos"));
    }
    v.extend(capstone_project_violations(&syllabus.capstone_project));
    v
}

/// The terminal transfer project's own content-level grounding — same
/// "explicit, verifiable artifact" bar as a `Micromodule.deliverable`, but
/// applied to the whole-roadmap capstone instead of a single session (see
/// `domain::roadmap::CapstoneProject`). Distinct from the last milestone's
/// `deliverable`: this must exist and be non-vague on its own, never
/// derived from it.
pub(super) fn capstone_project_violations(capstone: &CapstoneProject) -> Vec<String> {
    let mut v = Vec::new();
    if capstone.title.trim().is_empty() {
        v.push("capstoneProject.title está vacío".to_string());
    } else {
        check_student_facing_text(&mut v, "capstoneProject.title", &capstone.title);
    }
    if capstone.description.trim().is_empty() {
        v.push("capstoneProject.description está vacío".to_string());
    } else if is_vague_deliverable(&capstone.description) {
        v.push(format!(
            "capstoneProject.description (\"{}\") es vago — debe ser un proyecto de transferencia real, no una lista de temas",
            capstone.description
        ));
    } else {
        check_student_facing_text(&mut v, "capstoneProject.description", &capstone.description);
    }
    if capstone.verifiable_evidence.trim().is_empty() {
        v.push("capstoneProject.verifiableEvidence está vacío".to_string());
    } else if is_vague_deliverable(&capstone.verifiable_evidence) {
        v.push(format!(
            "capstoneProject.verifiableEvidence (\"{}\") es vago — debe ser un artefacto tangible que certifique el cierre",
            capstone.verifiable_evidence
        ));
    } else {
        check_student_facing_text(&mut v, "capstoneProject.verifiableEvidence", &capstone.verifiable_evidence);
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

