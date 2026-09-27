use super::harness::*;
use super::*;

    #[tokio::test]
    async fn start_serves_a_local_welcome_without_calling_the_model() {
        // Empty queue: any model call panics. Start must not need one.
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![])));
        let result = harness.service.start_session(None).await.expect("start ok");

        assert!(ONBOARDING_MESSAGES.contains(&result.message.as_str()));
        assert_eq!(result.session.status, SessionStatus::Active);
        assert_eq!(result.session.turn_count_in_phase, 0, "welcome spends no gather turn");
        // Persisted right away, so the drawer lists it even before the
        // first real reply.
        assert!(harness.service.get_session(&result.session.session_id).is_ok());
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn welcome_varies_across_sessions() {
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![])));
        let mut seen = std::collections::HashSet::new();
        for _ in 0..10 {
            let r = harness.service.start_session(None).await.expect("start ok");
            seen.insert(r.message);
        }
        assert!(seen.len() > 1, "rotating variants, not a single canned greeting");
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn ensure_course_imported_recovers_a_session_sealed_before_the_import_existed() {
        // Uses the absolute_zero skip path (assessment + propose chained,
        // no battery) — shorter, and exercises that path too.
        let step1 = ScriptedStep {
            text: "listo".to_string(),
            assessment: Some(full_assessment("Ciclo de Krebs", 4, 3.0, EntryLevel::AbsoluteZero)),
            diagnostic_battery: None,
            propose_syllabus_plan: Some(ProposeSyllabusPlanArgs {
                core_focus: "Lo esencial".to_string(),
                identified_needs: vec!["Entender el flujo".to_string()],
                learning_strategy: "Guiado".to_string(),
                syllabus: full_syllabus(4, 3.0, "Ciclo de Krebs"),
                closing_question: "¿Arrancamos así?".to_string(),
            }),
            confirm_syllabus_plan: None,
        };
        let step2 = full_confirm_step("listo");
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![step1, step2])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();
        harness.service.send_message(None, &sid, "Ciclo de Krebs, 4 semanas, 3h, no sé nada").await.expect("turn ok");
        let mut sealed = harness.service.send_message(None, &sid, "Sí").await.expect("confirm turn ok").session;
        assert_eq!(sealed.status, SessionStatus::Sealed);

        // Simulate a session sealed by an older build, before the notebook
        // import existed: wipe the ids and re-save, exactly the state that
        // made "Ver tu primera clase" a silent no-op.
        sealed.imported_course_id = None;
        sealed.first_class_id = None;
        harness.service.store.save_roadmap_session(&sealed).expect("seed the pre-import shape");

        let recovered = harness.service.ensure_course_imported(&sid).expect("recovers");
        assert!(recovered.imported_course_id.is_some());
        assert!(recovered.first_class_id.is_some());

        // Persisted, not just returned — reloading the session must see it too.
        let reloaded = harness.service.get_session(&sid).expect("reload");
        assert_eq!(reloaded.imported_course_id, recovered.imported_course_id);

        // Idempotent: calling again must NOT create a second course.
        let course_id = recovered.imported_course_id.clone().unwrap();
        let again = harness.service.ensure_course_imported(&sid).expect("idempotent");
        assert_eq!(again.imported_course_id, Some(course_id));

        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn ensure_course_imported_rejects_a_session_that_was_never_sealed() {
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let err = harness.service.ensure_course_imported(&r0.session.session_id).expect_err("must fail");
        assert!(matches!(err, AppError::InvalidInput(_)));
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn rename_sets_custom_title_and_validates() {
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![])));
        let session = RoadmapSession::new("s1".to_string(), 0);
        harness.service.store.save_roadmap_session(&session).expect("seed");

        let summary = harness.service.rename_session("s1", "  Mi meta  ").expect("rename");
        assert_eq!(summary.title, "Mi meta", "trims whitespace");
        let reloaded = harness.service.get_session("s1").expect("reload");
        assert_eq!(reloaded.custom_title.as_deref(), Some("Mi meta"));

        assert!(matches!(
            harness.service.rename_session("s1", "   ").unwrap_err(),
            AppError::InvalidInput(_)
        ));
        assert!(matches!(
            harness.service.rename_session("s1", &"x".repeat(81)).unwrap_err(),
            AppError::InvalidInput(_)
        ));
        assert!(matches!(
            harness.service.rename_session("missing", "ok").unwrap_err(),
            AppError::InvalidInput(_)
        ));
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn delete_removes_session_and_rejects_unknown_ids() {
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![])));
        let session = RoadmapSession::new("s1".to_string(), 0);
        harness.service.store.save_roadmap_session(&session).expect("seed");

        harness.service.delete_session("s1").expect("delete");
        assert!(harness.service.store.load_roadmap_session("s1").expect("load").is_none());
        assert!(matches!(
            harness.service.delete_session("s1").unwrap_err(),
            AppError::InvalidInput(_)
        ));
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn conversation_log_persists_user_and_assistant_turns() {
        let step = ScriptedStep { text: "Respuesta del mentor".to_string(), ..Default::default() };
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![step])));

        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();
        assert_eq!(r0.session.messages.len(), 1, "greeting opens the log");
        assert_eq!(r0.session.messages[0].role, "assistant");

        harness.service.send_message(None, &sid, "Mi meta es X").await.expect("turn ok");

        let reloaded = harness.service.get_session(&sid).expect("reload");
        assert_eq!(reloaded.messages.len(), 3, "greeting + user + assistant");
        assert_eq!(reloaded.messages[1].role, "user");
        assert_eq!(reloaded.messages[1].text, "Mi meta es X");
        assert_eq!(reloaded.messages[2].role, "assistant");
        assert_eq!(reloaded.messages[2].text, "Respuesta del mentor");
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn user_turn_survives_a_failed_model_call() {
        let step = ScriptedStep::default();
        let runner = Arc::new(FlakyRunner { failures_left: Mutex::new(5), transient: false, step, calls: Mutex::new(0) });
        let harness = service_with_runner(runner);
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();

        harness.service.send_message(None, &sid, "Lo que escribi").await.expect_err("permanent failure");

        let reloaded = harness.service.get_session(&sid).expect("reload");
        assert_eq!(reloaded.messages.len(), 2, "greeting + the user turn, persisted before the model call");
        assert_eq!(reloaded.messages[1].text, "Lo que escribi");
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[test]
    fn old_snapshot_without_custom_title_still_loads() {
        let dir = std::env::temp_dir().join(format!("learnkit-compat-{}", Uuid::new_v4()));
        let store = FileStore::new(dir.clone());
        std::fs::create_dir_all(dir.join("roadmap_sessions")).expect("dir");
        std::fs::write(
            dir.join("roadmap_sessions").join("old.json"),
            r#"{"session_id":"old","phase":"onboarding","status":"active","turn_count_in_phase":0,
                "draft":{"topic":"Viejo"},"learner_profile_card":null,"diagnostic_summary_card":null,
                "roadmap_package":null,"created_at_ms":0,"updated_at_ms":0}"#,
        )
        .expect("write");
        let loaded = store.load_roadmap_session("old").expect("load").expect("present");
        assert!(loaded.custom_title.is_none());
        assert_eq!(loaded.summary().title, "Viejo", "falls back to draft topic");
        let _ = std::fs::remove_dir_all(&dir);
    }
