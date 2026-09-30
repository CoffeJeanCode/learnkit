use super::harness::*;
use super::*;

    #[tokio::test]
    async fn a_clean_assessment_without_battery_triggers_the_gate2_followup_in_the_same_turn() {
        // The analyst (scope Assessment) can only land
        // `submit_diagnostic_assessment` — presenting the battery is its own
        // scoped follow-up turn right behind a clean main turn, not a
        // chained tool call inside one prompt.
        let step1 = ScriptedStep {
            text: "Cuéntame tu base actual.".to_string(),
            assessment: Some(full_assessment("Tema", 4, 3.0, EntryLevel::TheoreticalFoundations)),
            ..Default::default()
        };
        let step2 = ScriptedStep {
            text: "Antes de armar tu plan, revisemos esto:".to_string(),
            diagnostic_battery: Some(full_battery()),
            ..Default::default()
        };
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![step1, step2])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();

        let r = harness.service.send_message(None, &sid, "Tema, 4 semanas, 3h").await.expect("turn ok");

        assert!(r.session.learner_profile_card.is_some(), "the main turn landed the assessment");
        assert!(r.session.pending_diagnostic_battery.is_some(), "the battery follow-up ran in the same advance");
        assert_eq!(r.message.trim(), "Antes de armar tu plan, revisemos esto:", "the follow-up's text replaces the main turn's");
        assert_eq!(r.session.turn_count_in_phase, 0, "battery present → stable, no gathering turn spent");
        assert_eq!(r.session.consecutive_grounding_failures, 0);
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn an_absolute_zero_assessment_skips_the_battery_and_the_propose_followup_fires() {
        let step1 = ScriptedStep {
            text: "Entendido.".to_string(),
            assessment: Some(full_assessment("Tema", 4, 3.0, EntryLevel::AbsoluteZero)),
            ..Default::default()
        };
        let step2 = full_propose_step("Con eso ya puedo proponerte esto. ¿Te parece?", 4, 3.0, "Tema");
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![step1, step2])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();

        let r = harness.service.send_message(None, &sid, "Tema, 4 semanas, 3h").await.expect("turn ok");

        assert!(r.session.pending_diagnostic_battery.is_none(), "absolute_zero never shows a battery");
        assert!(r.session.proposed_plan.is_some(), "the propose follow-up fired right behind the assessment");
        assert_eq!(r.message.trim(), "Con eso ya puedo proponerte esto. ¿Te parece?");
        assert_eq!(r.session.turn_count_in_phase, 0, "proposal pending → stable, no gathering turn spent");
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    /// The durability gap this fixed: Gate 1 (assessment) and Gate 3a
    /// (propose) chain inside the SAME `advance()` call for the
    /// `absolute_zero` skip path (see the test above) — with only ONE save
    /// at the very end, AFTER both gates already ran. If the chained propose
    /// call died hard, the already-computed profile card had never touched
    /// disk and was lost with it. Proves the intermediate save added right
    /// before that chained call makes the profile card durable independently
    /// of whether the propose call ever lands.
    #[tokio::test]
    async fn absolute_zero_checkpoints_the_profile_card_before_the_chained_propose_call_dies() {
        let step1 = ScriptedStep {
            text: "Entendido.".to_string(),
            assessment: Some(full_assessment("Tema", 4, 3.0, EntryLevel::AbsoluteZero)),
            ..Default::default()
        };
        let runner = Arc::new(SucceedOnceThenFailRunner { first_step: Mutex::new(Some(step1)), calls: Mutex::new(0) });
        let harness = service_with_runner(runner.clone());
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();

        let err = harness
            .service
            .send_message(None, &sid, "Tema, 4 semanas, 3h")
            .await
            .expect_err("the chained propose call dies hard right after gate 1 succeeds");
        assert!(matches!(err, AppError::ProviderKeyMissing(_)));
        assert_eq!(*runner.calls.lock().unwrap(), 2, "gate 1 succeeded, then the chained propose call failed");

        let reloaded = harness.service.get_session(&sid).expect("reload from disk");
        assert!(
            reloaded.learner_profile_card.is_some(),
            "gate 1's profile card must be durable even though the chained propose call never landed"
        );
        assert!(reloaded.diagnostic_summary_card.is_none(), "propose never landed, so nothing past gate 1 is set");
        assert!(reloaded.proposed_plan.is_none());
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[test]
    fn the_analyst_drives_until_the_assessment_exists_then_the_roadmap_agent_takes_over() {
        let mut session = RoadmapSession::new("SID".to_string(), 0);
        let (agent, scope) = RoadmapService::turn_agent_and_scope(&session);
        assert_eq!(agent, crate::agents::analyst_agent::ANALYST_AGENT_ID, "Gate 1 runs on the one-tool analyst");
        assert_eq!(scope, DiagnosticToolScope::Assessment);

        session.learner_profile_card = Some(crate::domain::roadmap::LearnerProfileCard {
            topic: "Tema".to_string(),
            target_goal: "Aprobar el examen".to_string(),
            timeframe_weeks: 4,
            weekly_commitment_hours: 3.0,
            total_available_hours: 12.0,
            entry_level: EntryLevel::TheoreticalFoundations,
        });
        let (agent, scope) = RoadmapService::turn_agent_and_scope(&session);
        assert_eq!(agent, crate::agents::roadmap_agent::ROADMAP_AGENT_ID, "everything after Gate 1 is the full roadmap agent");
        assert_eq!(scope, DiagnosticToolScope::All);
    }
