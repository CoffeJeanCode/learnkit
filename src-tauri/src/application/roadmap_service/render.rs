//! Turn input rendering: the compact, machine-readable context handed to
//! the model on every turn, plus the rejection directive fed back on retry.
use super::*;

/// Builds the compact, machine-readable context sent as `input` on every
/// turn. Deliberately does NOT resend the model's own prior replies (the
/// backend keeps none) — only everything the STUDENT has said so far (see
/// `record_message`) and how many turns remain before the hard cap forces a
/// close.
/// Shared by both render functions below — fed with WHATEVER the session's
/// `last_rejection_reasons` are at render time, which is what makes
/// [`RoadmapService::run_diagnostic_turn_with_grounding_retry`]'s in-turn
/// retry actually productive: each retry re-renders input, sees the freshly
/// recorded reasons from the attempt that just failed, and hands them
/// straight back to the model.
pub(super) fn rejection_directive(reasons: &[String]) -> Option<String> {
    if reasons.is_empty() {
        return None;
    }
    Some(format!(
        "Tu intento anterior de llamar una herramienta fue RECHAZADO por estas razones: {}. \
         Corrígelo y vuelve a llamarla en este turno — no se lo menciones al estudiante, es \
         un ajuste interno.",
        reasons.join("; ")
    ))
}

pub(super) fn render_turn_input(session: &RoadmapSession, user_message: &str, scope: DiagnosticToolScope) -> String {
    let turns_remaining = RoadmapService::MAX_GATHER_TURNS.saturating_sub(session.turn_count_in_phase);
    let mut directives: Vec<String> = Vec::new();
    directives.extend(rejection_directive(&session.last_rejection_reasons));
    if turns_remaining <= 1 {
        // Scope-aware: the analyst turn only registers
        // `submit_diagnostic_assessment`, so telling IT to also present the
        // battery would name a tool it can't call.
        if scope == DiagnosticToolScope::Assessment {
            directives.push(
                "Este es tu último turno antes de que el sistema cierre automáticamente con \
                 valores asumidos. Actúa AHORA: si te falta algo, asúmelo con un valor razonable y \
                 llama a submit_diagnostic_assessment en este mismo turno — es tu única herramienta \
                 disponible."
                    .to_string(),
            );
        } else {
            directives.push(
                "Este es tu último turno antes de que el sistema cierre automáticamente con \
                 valores asumidos. Actúa AHORA: si te falta algo, asúmelo con un valor razonable y \
                 llama a submit_diagnostic_assessment y present_diagnostic_battery en este mismo \
                 turno (NO llames propose_syllabus_plan ni confirm_syllabus_plan todavía)."
                    .to_string(),
            );
        }
    }
    if scope.allows_confirm() {
        if let Some(plan) = &session.proposed_plan {
            directives.push(format!(
                "Ya propusiste un plan y esperas la respuesta del estudiante a esta pregunta de \
                 cierre: \"{}\". Si el mensaje del estudiante confirma (acepta, dice que sí, que está \
                 bien así), llama a confirm_syllabus_plan con SOLO el notebook de la primera clase — \
                 el temario ya quedó acordado, no lo repitas. Si en cambio pide cambios, llama a \
                 propose_syllabus_plan otra vez con un temario revisado y una nueva closingQuestion.",
                plan.closing_question
            ));
        }
    }

    let context = serde_json::json!({
        "session_id": session.session_id,
        "turns_remaining": turns_remaining,
        "student_said_so_far": session.draft.get("messages").cloned().unwrap_or(serde_json::json!([])),
        "assessment_already_saved": session.learner_profile_card,
        "proposed_plan_pending": session.proposed_plan,
        "system_directives": directives,
    });

    format!(
        "CONTEXTO_DE_SESION (uso interno, no lo muestres al estudiante):\n{}\n\nÚLTIMO_MENSAJE_DEL_ESTUDIANTE:\n{}",
        serde_json::to_string_pretty(&context).unwrap_or_default(),
        user_message
    )
}

/// Input for the Gate 2 follow-up turn: the assessment already landed in
/// the previous (main) turn, so this scoped turn's ONLY job is presenting
/// the situational battery — mirroring what `render_propose_syllabus_input`
/// does for Gate 3a.
pub(super) fn render_battery_followup_input(session: &RoadmapSession) -> String {
    let directives: Vec<String> = rejection_directive(&session.last_rejection_reasons).into_iter().collect();
    let context = serde_json::json!({
        "learnerProfile": session.learner_profile_card,
        "system_directives": directives,
        "instruction": "El diagnóstico base ya quedó guardado. Tu ÚNICA tarea en este turno es llamar \
            a present_diagnostic_battery con la batería situacional (3-4 preguntas según tus reglas \
            psicométricas) — no vuelvas a llamar submit_diagnostic_assessment ni propongas el temario \
            todavía.",
    });
    format!(
        "Presenta la batería de diagnóstico llamando a present_diagnostic_battery según tu system \
         prompt.\n\n{}",
        serde_json::to_string_pretty(&context).unwrap_or_default()
    )
}

/// Builds the input for the Gate 3a (propose) turn: the model's job here is
/// ONLY to call `propose_syllabus_plan`, calibrated by the concrete
/// diagnostic results (`DiagnosticBatteryState::profile_summary`) when a
/// battery was answered, or by the plain `absolute_zero` declaration when it
/// was skipped — never the vague pre-diagnosis inference Gate 1 used to do.
pub(super) fn render_propose_syllabus_input(profile: &LearnerProfileCard, pending: Option<&DiagnosticBatteryState>, last_rejection_reasons: &[String]) -> String {
    let diagnostic_results = match pending {
        Some(p) => p.profile_summary(),
        None => serde_json::json!({ "answered": false, "note": "battery was skipped: entryLevel is absolute_zero" }),
    };
    let directives: Vec<String> = rejection_directive(last_rejection_reasons).into_iter().collect();
    let context = serde_json::json!({
        "learnerProfile": profile,
        "diagnosticResults": diagnostic_results,
        "system_directives": directives,
        "instruction": "El diagnóstico ya está resuelto (batería respondida, o nivel absolute_zero \
            declarado). Llama AHORA a propose_syllabus_plan con el diagnóstico consolidado, el \
            temario, y una closingQuestion — no llames ninguna otra herramienta ni persistas nada \
            todavía.",
    });
    format!(
        "Propón el diagnóstico consolidado y el temario llamando a propose_syllabus_plan según tu \
         system prompt.\n\n{}",
        serde_json::to_string_pretty(&context).unwrap_or_default()
    )
}

