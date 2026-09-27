use super::harness::*;
use super::*;

    #[tokio::test]
    async fn transient_failure_is_retried_once_then_succeeds() {
        let step = ScriptedStep { text: "Hola de nuevo".to_string(), ..Default::default() };
        let runner = Arc::new(FlakyRunner { failures_left: Mutex::new(1), transient: true, step, calls: Mutex::new(0) });
        let harness = service_with_runner(runner.clone());
        let r0 = harness.service.start_session(None).await.expect("start ok");
        assert_eq!(*runner.calls.lock().unwrap(), 0, "welcome needs no model call");
        let sid = r0.session.session_id.clone();

        let result = harness.service.send_message(None, &sid, "Hola").await.expect("recovers after one retry");
        assert!(result.message.contains("Hola de nuevo"));
        assert_eq!(*runner.calls.lock().unwrap(), 2);
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn transient_failure_exhausting_the_budget_surfaces_the_error() {
        let step = ScriptedStep::default();
        // One more failure than the retry budget allows.
        let runner = Arc::new(FlakyRunner {
            failures_left: Mutex::new(RoadmapService::MAX_EXECUTION_ATTEMPTS as usize),
            transient: true,
            step,
            calls: Mutex::new(0),
        });
        let harness = service_with_runner(runner.clone());
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();

        let err = harness.service.send_message(None, &sid, "Hola").await.expect_err("must fail once the budget runs out");
        assert!(matches!(err, AppError::AgentExecutionFailed(_)));
        assert_eq!(*runner.calls.lock().unwrap(), RoadmapService::MAX_EXECUTION_ATTEMPTS as usize, "no attempt beyond the budget");
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn permanent_failure_is_not_retried() {
        let step = ScriptedStep::default();
        let runner = Arc::new(FlakyRunner { failures_left: Mutex::new(5), transient: false, step, calls: Mutex::new(0) });
        let harness = service_with_runner(runner.clone());
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();

        let err = harness.service.send_message(None, &sid, "Hola").await.expect_err("missing key must fail fast");
        assert!(matches!(err, AppError::ProviderKeyMissing(_)));
        assert_eq!(*runner.calls.lock().unwrap(), 1);
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    /// The UI's "Reintentar" (and its one automatic re-attempt): the send's
    /// whole budget died on the DeepSeek connection error, then the blip
    /// cleared. The retry must resume that already-persisted turn — never
    /// re-record it, or the log would show the student's message twice.
    #[tokio::test]
    async fn retry_resumes_a_failed_turn_without_duplicating_the_user_message() {
        let step = ScriptedStep { text: "Respuesta tras el fallo".to_string(), ..Default::default() };
        let runner = Arc::new(SeqRunner::new(vec![step]));
        runner.fail_next(RoadmapService::MAX_EXECUTION_ATTEMPTS as usize);
        let harness = service_with_runner(runner.clone());
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();

        harness.service.send_message(None, &sid, "Hola").await.expect_err("budget exhausted");
        runner.fail_next(0); // the blip cleared

        let result = harness.service.retry_pending_turn(None, &sid).await.expect("retry ok");
        assert_eq!(result.message, "Respuesta tras el fallo");

        let reloaded = harness.service.get_session(&sid).expect("reload");
        let users: Vec<&ChatTurn> = reloaded.messages.iter().filter(|m| m.role == "user").collect();
        assert_eq!(users.len(), 1, "the failed turn's user message is resumed, not re-recorded");
        assert_eq!(users[0].text, "Hola");
        assert_eq!(
            *runner.calls.lock().unwrap(),
            RoadmapService::MAX_EXECUTION_ATTEMPTS as usize + 1,
            "exactly one more model call on top of the spent budget"
        );
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    /// Gate 3a is the one turn that follows an answer instead of a message:
    /// if the model dies there, the FINAL answer must already be on disk
    /// (saved before the model call) so `retry_pending_turn` can find a
    /// fully-answered battery and re-run the propose turn.
    #[tokio::test]
    async fn retry_recreates_the_propose_turn_after_the_last_answer_died_mid_model() {
        let battery = full_battery();
        let gate12 = ScriptedStep {
            text: "Aquí va tu batería".to_string(),
            assessment: Some(full_assessment("Ciclo de Krebs", 4, 3.0, EntryLevel::TheoreticalFoundations)),
            diagnostic_battery: Some(battery.clone()),
            ..Default::default()
        };
        let propose = full_propose_step("Te propongo esto", 4, 3.0, "Ciclo de Krebs");
        let runner = Arc::new(SeqRunner::new(vec![gate12, propose]));
        let harness = service_with_runner(runner.clone());
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();

        harness.service.send_message(None, &sid, "Mi meta").await.expect("gates 1+2 chain");
        // The last answer hands off to Gate 3a — and that model call dies.
        runner.fail_next(RoadmapService::MAX_EXECUTION_ATTEMPTS as usize);
        for (i, q) in battery.questions.iter().enumerate() {
            let answered = harness
                .service
                .answer_diagnostic_question(None, &sid, i, &q.correct_option)
                .await;
            if i + 1 == battery.questions.len() {
                answered.expect_err("the propose turn died mid-model");
            } else {
                answered.expect("mid-battery answers never touch the model");
            }
        }

        let saved = harness.service.get_session(&sid).expect("reload");
        let pending = saved.pending_diagnostic_battery.expect("battery still pending");
        assert_eq!(pending.answers.len(), battery.questions.len(), "final answer persisted before the model call");
        assert!(saved.proposed_plan.is_none(), "the propose turn never landed");

        runner.fail_next(0);
        let result = harness.service.retry_pending_turn(None, &sid).await.expect("retry ok");
        assert_eq!(result.message, "Te propongo esto");
        assert!(result.session.proposed_plan.is_some(), "Gate 3a recovered on retry");
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    /// A retry is only meaningful when a turn is actually hanging: a fresh
    /// session (or a sealed one) has nothing to resume, so the UI's retry
    /// affordance gets a typed error instead of a bogus model call.
    #[tokio::test]
    async fn retry_with_nothing_pending_is_rejected() {
        let harness = service_with_runner(Arc::new(SeqRunner::new(vec![])));
        let r0 = harness.service.start_session(None).await.expect("start ok");

        let err = harness
            .service
            .retry_pending_turn(None, &r0.session.session_id)
            .await
            .expect_err("greeting-only session has no pending turn");
        assert!(matches!(err, AppError::InvalidInput(_)));
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

