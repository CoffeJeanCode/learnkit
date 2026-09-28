use super::harness::*;
use super::*;

    #[tokio::test]
    async fn full_gated_flow_seals_only_after_battery_and_explicit_confirmation() {
        let battery = full_battery();
        let step1 = ScriptedStep {
            text: "Antes de armar tu plan, quiero ver qué tanto conectas ya con esto:".to_string(),
            assessment: Some(full_assessment("Ciclo de Krebs", 4, 3.0, EntryLevel::TheoreticalFoundations)),
            diagnostic_battery: Some(battery.clone()),
            propose_syllabus_plan: None,
            confirm_syllabus_plan: None,
        };
        let step2 = full_propose_step(
            "Con eso ya puedo armar tu plan. ¿Te parece adecuada esta distribución?",
            4,
            3.0,
            "Ciclo de Krebs",
        );
        let step3 = full_confirm_step("¡Plan de 4 semanas generado y guardado! Arrancamos con tu primera práctica a continuación:");
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![step1, step2, step3])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        assert_eq!(r0.session.status, SessionStatus::Active);
        let sid = r0.session.session_id.clone();

        let diag = harness.service.send_message(None, &sid, "Ciclo de Krebs, 4 semanas, 3h").await.expect("turn ok");
        assert_eq!(diag.session.phase, RoadmapPhase::Diagnostic, "assessment chains straight into the battery, no confirmation turn");
        assert!(diag.session.pending_diagnostic_battery.is_some());
        assert_eq!(diag.session.status, SessionStatus::Active, "not sealed until the battery is answered");

        // No extra user action beyond answering — the last answer triggers
        // Gate 3a (propose) automatically.
        let proposed = answer_all_questions(&harness, &sid, &battery).await;
        assert_eq!(proposed.session.status, SessionStatus::Active, "a proposal is not a seal — it waits for confirmation");
        assert!(proposed.session.proposed_plan.is_some());
        assert!(proposed.session.imported_course_id.is_none(), "nothing persisted yet at the propose stage");

        // The student's free-text confirmation is what actually seals.
        let result = harness.service.send_message(None, &sid, "Sí, me parece bien.").await.expect("confirm turn ok");

        assert_eq!(result.session.status, SessionStatus::Sealed, "sealed only once the student explicitly confirmed");
        assert!(result.session.imported_course_id.is_some());
        assert!(result.session.first_class_id.is_some());
        assert!(result.session.pending_diagnostic_battery.is_none(), "transferred onto the course, not kept on the session");
        assert!(result.session.proposed_plan.is_none(), "cleared once confirmed");
        assert!(!result.message.to_lowercase().contains("¿lo guardamos"));

        let classes = harness
            .service
            .notebook_store
            .list_classes_for_course(result.session.imported_course_id.as_deref().unwrap())
            .expect("list classes");
        assert_eq!(classes.len(), 8, "2 sessions per week imported (4 weeks x 2 sessions)");

        assert!(
            harness.service.notebook_store.load_notebook_by_class(result.session.first_class_id.as_deref().unwrap()).expect("query").is_none(),
            "no notebook is generated at seal time anymore — it's lazy, the first time the class is opened"
        );

        let saved_battery = harness
            .service
            .notebook_store
            .get_course_diagnostic_battery(result.session.imported_course_id.as_deref().unwrap())
            .expect("query")
            .expect("battery persisted onto the course");
        assert_eq!(saved_battery.answers.len(), battery.questions.len(), "the REAL answers, not an empty battery");

        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn requesting_changes_re_proposes_instead_of_confirming() {
        // Gate 3a -> student asks for changes (free text) -> Gate 3a again
        // (revised syllabus) -> only THEN does an explicit confirmation seal.
        let battery = full_battery();
        let step1 = ScriptedStep {
            text: "Antes de armar tu plan, quiero ver qué tanto conectas ya con esto:".to_string(),
            assessment: Some(full_assessment("Ciclo de Krebs", 4, 3.0, EntryLevel::TheoreticalFoundations)),
            diagnostic_battery: Some(battery.clone()),
            propose_syllabus_plan: None,
            confirm_syllabus_plan: None,
        };
        let step2 = full_propose_step(
            "Con eso ya puedo armar tu plan. ¿Te parece adecuada esta distribución?",
            4,
            3.0,
            "Ciclo de Krebs",
        );
        // The revised proposal: still 4 weeks / 3h (must match the declared
        // timeframeWeeks/weeklyCommitmentHours from Gate 1 — that's locked in
        // by `syllabus_violations`, an adjustment can't silently change it),
        // but a different course title — enough to prove it's a genuinely
        // new proposal, not the same one resent.
        let step3 = full_propose_step("Ajusté el enfoque. ¿Así sí te funciona?", 4, 3.0, "Ciclo de Krebs (repaso reforzado)");
        let step4 = full_confirm_step("¡Plan de 4 semanas generado y guardado! Arrancamos con tu primera práctica a continuación:");
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![step1, step2, step3, step4])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();

        harness.service.send_message(None, &sid, "Ciclo de Krebs, 4 semanas, 3h").await.expect("turn ok");
        let proposed = answer_all_questions(&harness, &sid, &battery).await;
        assert_eq!(proposed.session.proposed_plan.as_ref().unwrap().syllabus.course_title, "Ciclo de Krebs");

        // Student asks for changes instead of confirming — a normal chat
        // turn, since the battery is fully answered (not mid-click anymore).
        let adjusted = harness
            .service
            .send_message(None, &sid, "Prefiero reforzar más el repaso antes de avanzar.")
            .await
            .expect("adjust turn ok");
        assert_eq!(adjusted.session.status, SessionStatus::Active, "still just a revised proposal, not sealed");
        assert_eq!(
            adjusted.session.proposed_plan.as_ref().unwrap().syllabus.course_title,
            "Ciclo de Krebs (repaso reforzado)",
            "the proposal was replaced with the revised one, not left untouched"
        );
        assert!(adjusted.session.imported_course_id.is_none(), "still nothing persisted after an adjustment");

        let result = harness.service.send_message(None, &sid, "Ahora sí, así está perfecto.").await.expect("confirm turn ok");
        assert_eq!(result.session.status, SessionStatus::Sealed);
        assert!(result.session.imported_course_id.is_some());

        let classes = harness
            .service
            .notebook_store
            .list_classes_for_course(result.session.imported_course_id.as_deref().unwrap())
            .expect("list classes");
        assert_eq!(classes.len(), 8, "sealed with the REVISED syllabus, still 4 weeks x 2 sessions");

        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn a_plain_clarifying_question_does_not_seal_and_spends_one_turn() {
        let step = ScriptedStep { text: "¿Qué tema quieres dominar, en cuántas semanas y con cuántas horas por semana?".to_string(), ..Default::default() };
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![step])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();

        let result = harness.service.send_message(None, &sid, "Aún no lo sé").await.expect("turn ok");

        assert_eq!(result.session.status, SessionStatus::Active);
        assert_eq!(result.session.turn_count_in_phase, 1);
        assert!(result.session.learner_profile_card.is_none());
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn force_closes_after_the_gather_cap_without_a_third_model_call() {
        let q1 = ScriptedStep { text: "¿Qué quieres lograr?".to_string(), ..Default::default() };
        let q2 = ScriptedStep { text: "¿Cuánto tiempo tienes?".to_string(), ..Default::default() };
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![q1, q2])));

        // Start is free: local welcome, no turn spent, no model call.
        let r0 = harness.service.start_session(None).await.expect("start ok");
        assert_eq!(r0.session.turn_count_in_phase, 0);
        let sid = r0.session.session_id.clone();

        let r1 = harness.service.send_message(None, &sid, "Quiero aprender Rust").await.expect("turn 1 ok");
        assert_eq!(r1.session.turn_count_in_phase, 1);

        let r2 = harness.service.send_message(None, &sid, "6 semanas, 3h").await.expect("turn 2 ok");
        assert_eq!(r2.session.turn_count_in_phase, 2);
        assert_eq!(r2.session.status, SessionStatus::Active, "cap reached but not yet force-closed");
        drop(r2);

        // Cap already reached before this call — must NOT call the model
        // again (only 2 scripted steps were queued; a 3rd call panics).
        let r3 = harness.service.send_message(None, &sid, "cualquier cosa").await.expect("force-close turn ok");
        assert_eq!(r3.session.status, SessionStatus::Sealed);
        assert!(r3.session.learner_profile_card.is_some());
        assert!(r3.session.imported_course_id.is_some(), "force-close still imports into the notebook store");
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn confirm_syllabus_plan_before_assessment_is_rejected() {
        let step = full_confirm_step("..");
        // Queued twice: the in-turn grounding retry (see
        // `run_diagnostic_turn_with_grounding_retry`) consumes a 2nd
        // scripted step when the 1st attempt fails, same content again.
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![step.clone(), step])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();

        let result = harness.service.send_message(None, &sid, "Hazlo").await.expect("turn ok");

        assert_eq!(result.session.status, SessionStatus::Active);
        assert!(result.session.roadmap_package.is_none());
        assert!(!result.session.last_rejection_reasons.is_empty());
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn propose_syllabus_plan_before_the_battery_is_answered_is_rejected() {
        // Assessment + battery succeed normally, but the model tries to jump
        // straight to propose_syllabus_plan in that SAME turn instead of
        // waiting for the student's answers.
        let battery = full_battery();
        let mut step = full_propose_step("..", 6, 3.0, "Tema");
        step.assessment = Some(full_assessment("Tema", 6, 3.0, EntryLevel::TheoreticalFoundations));
        step.diagnostic_battery = Some(battery);
        // Queued twice: the in-turn grounding retry consumes a 2nd scripted
        // step when the propose call fails again — the assessment/battery
        // resend inside it is a harmless no-op (already saved).
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![step.clone(), step])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();

        let result = harness.service.send_message(None, &sid, "Hazlo").await.expect("turn ok");

        assert_eq!(result.session.status, SessionStatus::Active, "must not propose before the battery is answered");
        assert!(result.session.pending_diagnostic_battery.is_some(), "the battery itself still saved");
        assert!(result.session.proposed_plan.is_none());
        assert!(result.session.last_rejection_reasons.iter().any(|r| r.contains("batería")));
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn propose_syllabus_plan_drifting_from_assessment_is_rejected() {
        let battery = full_battery();
        let step1 = ScriptedStep {
            text: "..".to_string(),
            assessment: Some(full_assessment("Tema", 6, 3.0, EntryLevel::TheoreticalFoundations)),
            diagnostic_battery: Some(battery.clone()),
            ..Default::default()
        };
        // total_weeks (4) drifts from the assessment's timeframeWeeks (6).
        let step2 = full_propose_step("..", 4, 3.0, "Tema");
        // step2 queued twice: `finish_diagnostic_stage`'s in-turn grounding
        // retry consumes a 2nd scripted step when the drifting propose fails
        // again.
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![step1, step2.clone(), step2])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();
        harness.service.send_message(None, &sid, "Hazlo").await.expect("turn ok");

        let result = answer_all_questions(&harness, &sid, &battery).await;

        assert_eq!(result.session.status, SessionStatus::Active, "drift must block sealing");
        assert!(result.session.learner_profile_card.is_some(), "the assessment itself still saved");
        assert!(result.session.last_rejection_reasons.iter().any(|r| r.contains("totalWeeks")));
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn repeated_propose_failures_across_turns_eventually_force_progress() {
        // The exact bug this fixed: `pending_diagnostic_battery.is_some()`
        // freezes `turn_count_in_phase`/`MAX_GATHER_TURNS` FOREVER once the
        // battery exists (by design — Gate 3 is meant to be student-paced,
        // not model-retry-capped). But a model that never lands a valid
        // `propose_syllabus_plan` could loop indefinitely anyway — the
        // student kept seeing the honest fallback text turn after turn with
        // nothing ever resolving. `consecutive_grounding_failures` bounds
        // THAT, independently of `turn_count_in_phase`.
        let battery = full_battery();
        let step1 = ScriptedStep {
            text: "..".to_string(),
            assessment: Some(full_assessment("Tema", 4, 3.0, EntryLevel::TheoreticalFoundations)),
            diagnostic_battery: Some(battery.clone()),
            ..Default::default()
        };
        // Always the SAME failure (totalWeeks 6 drifts from the assessment's
        // declared 4), so every attempt fails identically and deterministically.
        let failing_propose = || full_propose_step("..", 6, 3.0, "Tema");
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![
            step1,
            failing_propose(),
            failing_propose(),
            failing_propose(),
            failing_propose(),
        ])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();
        harness.service.send_message(None, &sid, "Tema, 4 semanas, 3h").await.expect("turn ok");

        // Answering the battery triggers finish_diagnostic_stage — consumes
        // 2 scripted failing steps via its own in-turn grounding retry.
        let after_battery = answer_all_questions(&harness, &sid, &battery).await;
        assert!(after_battery.session.proposed_plan.is_none(), "propose never validated");
        assert_eq!(after_battery.session.consecutive_grounding_failures, 1);

        // First real follow-up turn — consumes the other 2 scripted steps.
        let r1 = harness.service.send_message(None, &sid, "sigue").await.expect("turn ok");
        assert_eq!(r1.session.status, SessionStatus::Active);
        assert_eq!(r1.session.consecutive_grounding_failures, 2);

        // Cap reached — this next turn must force progress WITHOUT calling
        // the model again (no scripted step left; a 6th call would panic).
        let r2 = harness.service.send_message(None, &sid, "sigue").await.expect("force-progress turn ok");
        assert_eq!(r2.session.status, SessionStatus::Sealed, "must never loop forever once grounding keeps failing across turns");
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn a_persistently_failing_diagnostic_battery_still_force_closes_after_the_cap() {
        // Every turn re-sends a VALID assessment (as a real model might,
        // redundantly) alongside a present_diagnostic_battery call that
        // keeps failing grounding (too few questions) — the same class of
        // bug the syllabus-side fix addressed: an always-succeeding sibling
        // call must never mask a persistently-failing one from the gather
        // cap, or the conversation could loop forever.
        let failing_step = || ScriptedStep {
            text: "Antes de armar tu plan, quiero ver qué tanto conectas ya con esto:".to_string(),
            assessment: Some(full_assessment("Redes", 6, 1.0, EntryLevel::TheoreticalFoundations)),
            diagnostic_battery: Some(DiagnosticBattery { goal_alignment: "x".to_string(), questions: vec![] }), // 0 questions, needs 3-5
            ..Default::default()
        };
        // 4 steps: each of the 2 user-facing sends below internally retries
        // once (MAX_GROUNDING_RETRIES) since the battery keeps failing, so
        // each send consumes 2 scripted steps.
        let harness =
            service_with_runner(Arc::new(ScriptedRunner::new(vec![failing_step(), failing_step(), failing_step(), failing_step()])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();

        let r1 = harness.service.send_message(None, &sid, "6 semanas, 1h, novato").await.expect("turn 1 ok");
        assert_eq!(r1.session.status, SessionStatus::Active);
        assert!(r1.session.pending_diagnostic_battery.is_none(), "the battery never validated");
        assert_eq!(r1.session.turn_count_in_phase, 1, "must count even though the assessment succeeded");

        let r2 = harness.service.send_message(None, &sid, "Que?").await.expect("turn 2 ok");
        assert_eq!(r2.session.status, SessionStatus::Active);
        assert_eq!(r2.session.turn_count_in_phase, 2, "cap reached but not yet force-closed");

        // Cap already reached — must force-close without a 5th model call
        // (only 4 scripted steps were queued; a 5th call panics).
        let r3 = harness.service.send_message(None, &sid, "cualquier cosa").await.expect("force-close turn ok");
        assert_eq!(r3.session.status, SessionStatus::Sealed, "the loop must be bounded — it always seals eventually");
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

