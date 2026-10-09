    use std::collections::VecDeque;
    use std::sync::Mutex;
    use std::time::Duration;

    use async_trait::async_trait;

    use super::*;
    use crate::agents::AgentRegistry;
    use crate::domain::notebook::{BlockStatus, DynamicBlockType, GateSubmission, GeneratedSectionBlock, PredictionComparison};
    use crate::domain::roadmap::{
        CapstoneProject, Deliverable, DeliverableArtifactType, EntryLevel, LearnerProfileCard, Micromodule, Milestone, RoadmapSyllabusPackage,
        SealedRoadmap, SessionStatus,
    };
    use crate::orchestration::PromptRunner;
    use crate::providers::factory::PromptOutput;
    use crate::tools::{BlockAuditCapture, BlockAuditResult, ClosureFeedbackCapture, ClosureFeedbackResult, GateGradingCapture, GateGradingResult, GateScaffold, NotebookBlockCapture, ScaffoldType};

    fn sealed_roadmap_session() -> RoadmapSession {
        let mut session = RoadmapSession::new("SID".to_string(), 0);
        session.status = SessionStatus::Sealed;
        session.roadmap_package = Some(SealedRoadmap {
            schema_version: "1.0".to_string(),
            package_id: "PID".to_string(),
            session_id: "SID".to_string(),
            generated_at_ms: 0,
            learner_profile: LearnerProfileCard {
                topic: "Ciclo de Krebs".to_string(),
                target_goal: "Aprobar el examen".to_string(),
                timeframe_weeks: 2,
                weekly_commitment_hours: 3.0,
                total_available_hours: 6.0,
                entry_level: EntryLevel::TheoreticalFoundations,
            },
            diagnostic_summary: crate::domain::roadmap::DiagnosticSummaryCard {
                core_focus: "Lo esencial".to_string(),
                identified_needs: vec!["Entender el flujo".to_string()],
                learning_strategy: "Guiado".to_string(),
            },
            syllabus: RoadmapSyllabusPackage {
                course_title: "Dominio del Ciclo de Krebs".to_string(),
                total_weeks: 1,
                pace_hours_per_week: 3.0,
                milestones: vec![Milestone {
                    week: 1,
                    title: "La gran imagen".to_string(),
                    deliverable: "Mapa visual".to_string(),
                    weekly_goal: Some("Ver el ciclo completo antes de entrar en cada paso".to_string()),
                    micromodules: vec![
                        Micromodule {
                            label: "Días 1-2 — Panorama general".to_string(),
                            hours: 1.5,
                            focus: Some("Cómo se conectan las 8 reacciones del ciclo entre sí".to_string()),
                            deliverable: Deliverable {
                                artifact_type: DeliverableArtifactType::FormalDiagram,
                                description: "Mapa visual anotado".to_string(),
                            },
                            objective: Some("Explicar el ciclo de Krebs con un mapa propio".to_string()),
                            interactive_blocks: vec!["interactive_visual_anchor".to_string(), "metacognitive_closure".to_string(), "hands_on_mission".to_string()],
                        },
                        Micromodule {
                            label: "Días 3-4 — Puntos de control".to_string(),
                            hours: 1.5,
                            focus: Some("Qué reacciones regulan la velocidad del ciclo".to_string()),
                            deliverable: Deliverable {
                                artifact_type: DeliverableArtifactType::FormalDiagram,
                                description: "Mapa visual anotado 2".to_string(),
                            },
                            objective: Some("Señalar los puntos de control del ciclo en el mapa".to_string()),
                            interactive_blocks: vec!["error_audit_challenge".to_string(), "metacognitive_closure".to_string(), "hands_on_mission".to_string()],
                        },
                    ],
                }],
                capstone_project: CapstoneProject {
                    title: "Proyecto terminal".to_string(),
                    description: "Integrar lo aprendido en un escenario real de transferencia".to_string(),
                    verifiable_evidence: "Repositorio con la app corriendo + demo grabada".to_string(),
                },
            },
            diagnostic_battery: None,
        });
        session
    }

    fn micro_theory() -> GeneratedSectionBlock {
        GeneratedSectionBlock::AnchoredMicroTheory {
            title: "La gran imagen".to_string(),
            intuitive_hook: "Piensa en una fábrica circular".to_string(),
            analogy_boundary: Some("La fábrica no explica qué pasa cuando el buffer se llena".to_string()),
            system_rule: "Explicación breve del tema.".to_string(),
            frequent_error: "Confundir la entrada con la salida".to_string(),
        }
    }

    fn prediction_gate(correct: &str) -> GeneratedSectionBlock {
        GeneratedSectionBlock::InteractivePredictionGate {
            question: "¿Qué pasa primero?".to_string(),
            options: vec!["A".to_string(), "B".to_string()],
            conceptual_feedback_map: [("A".to_string(), "Casi".to_string()), ("B".to_string(), "Correcto".to_string())]
                .into_iter()
                .collect(),
            correct_option: correct.to_string(),
            visual_aid: None,
        }
    }

    fn hands_on_mission() -> GeneratedSectionBlock {
        GeneratedSectionBlock::HandsOnMission {
            challenge_statement: "Resuelve este caso".to_string(),
            expected_milestone_artifact: "Un diagrama completado".to_string(),
            constraints: vec!["Sin librerías externas".to_string()],
            scaffolding_hints: vec!["Empieza por identificar las entidades".to_string()],
            evaluation_rubric_summary: vec!["Cubre el caso base".to_string()],
            is_transfer: false,
            visual_aid: None,
        }
    }

    /// The transfer challenge that must precede the closure: same skill, NEW
    /// case, constraints unseen in `hands_on_mission()`, and no hints.
    fn transfer_mission() -> GeneratedSectionBlock {
        GeneratedSectionBlock::HandsOnMission {
            challenge_statement: "Aplica la misma idea a otro dominio".to_string(),
            expected_milestone_artifact: "Una solución en el caso nuevo".to_string(),
            constraints: vec!["Datos con valores faltantes".to_string()],
            scaffolding_hints: vec![],
            evaluation_rubric_summary: vec!["Adapta la regla al caso nuevo".to_string()],
            is_transfer: true,
            visual_aid: None,
        }
    }

    fn closure() -> GeneratedSectionBlock {
        GeneratedSectionBlock::MetacognitiveClosure {
            synthesis_task: "Explica con tus palabras lo aprendido".to_string(),
            prediction_comparison: PredictionComparison {
                initial_prediction: "ip".to_string(),
                final_result: "fr".to_string(),
                contrast_narrative: "cn".to_string(),
            },
            self_evaluation_checklist: vec!["Puedo explicarlo sin ayuda".to_string()],
        }
    }

    #[derive(Default)]
    struct ScriptedNotebookRunner {
        blocks: Mutex<VecDeque<Option<GeneratedSectionBlock>>>,
        grading: Mutex<VecDeque<GateGradingResult>>,
        closure_feedback: Mutex<VecDeque<ClosureFeedbackResult>>,
        block_calls: Mutex<usize>,
        grading_calls: Mutex<usize>,
        closure_calls: Mutex<usize>,
    }

    #[async_trait]
    impl PromptRunner for ScriptedNotebookRunner {
        async fn run(&self, _: &str, _: &str, _: &str, _: &str, _: &[String]) -> AppResult<PromptOutput> {
            unreachable!("notebook generation never calls the plain envelope runner")
        }

        async fn run_notebook_block_execution(
            &self,
            _provider_id: &str,
            _model: &str,
            _system_prompt: &str,
            _input: &str,
            capture: NotebookBlockCapture,
        ) -> AppResult<PromptOutput> {
            *self.block_calls.lock().unwrap() += 1;
            if let Some(block) = self.blocks.lock().unwrap().pop_front().flatten() {
                *capture.lock().unwrap() = Some(block);
            }
            Ok(PromptOutput { text: String::new(), tool_calls: vec![] })
        }

        async fn run_gate_grading_execution(
            &self,
            _provider_id: &str,
            _model: &str,
            _system_prompt: &str,
            _input: &str,
            capture: GateGradingCapture,
        ) -> AppResult<PromptOutput> {
            *self.grading_calls.lock().unwrap() += 1;
            if let Some(result) = self.grading.lock().unwrap().pop_front() {
                *capture.lock().unwrap() = Some(result);
            }
            Ok(PromptOutput { text: String::new(), tool_calls: vec![] })
        }

        async fn run_closure_grading_execution(
            &self,
            _provider_id: &str,
            _model: &str,
            _system_prompt: &str,
            _input: &str,
            capture: ClosureFeedbackCapture,
        ) -> AppResult<PromptOutput> {
            *self.closure_calls.lock().unwrap() += 1;
            if let Some(result) = self.closure_feedback.lock().unwrap().pop_front() {
                *capture.lock().unwrap() = Some(result);
            }
            Ok(PromptOutput { text: String::new(), tool_calls: vec![] })
        }
    }

    fn service_with(runner: Arc<dyn PromptRunner>) -> NotebookService {
        service_with_store(NotebookStore::open_in_memory().expect("open store"), runner)
    }

    fn service_with_store(store: NotebookStore, runner: Arc<dyn PromptRunner>) -> NotebookService {
        let agents = Arc::new(AgentRegistry::new());
        agents.register(crate::agents::notebook_agent::definition()).expect("register notebook agent");
        agents.register(crate::agents::notebook_gate_grader_agent::definition()).expect("register gate grader agent");
        agents.register(crate::agents::closure_feedback_grader_agent::definition()).expect("register closure feedback grader agent");
        let orchestrator = Arc::new(Orchestrator::new(agents, runner));
        NotebookService::new(store, orchestrator)
    }

    #[derive(Default)]
    struct CriticRunner {
        blocks: Mutex<VecDeque<Option<GeneratedSectionBlock>>>,
        audits: Mutex<VecDeque<AppResult<BlockAuditResult>>>,
        inputs: Mutex<Vec<String>>,
        block_calls: Mutex<usize>,
        audit_calls: Mutex<usize>,
    }

    #[async_trait]
    impl PromptRunner for CriticRunner {
        async fn run(&self, _: &str, _: &str, _: &str, _: &str, _: &[String]) -> AppResult<PromptOutput> {
            unreachable!("notebook generation never calls the plain envelope runner")
        }

        async fn run_notebook_block_execution(
            &self,
            _provider_id: &str,
            _model: &str,
            _system_prompt: &str,
            input: &str,
            capture: NotebookBlockCapture,
        ) -> AppResult<PromptOutput> {
            *self.block_calls.lock().unwrap() += 1;
            self.inputs.lock().unwrap().push(input.to_string());
            if let Some(block) = self.blocks.lock().unwrap().pop_front().flatten() {
                *capture.lock().unwrap() = Some(block);
            }
            Ok(PromptOutput { text: String::new(), tool_calls: vec![] })
        }

        async fn run_block_audit_execution(
            &self,
            _provider_id: &str,
            _model: &str,
            _system_prompt: &str,
            _input: &str,
            capture: BlockAuditCapture,
        ) -> AppResult<PromptOutput> {
            *self.audit_calls.lock().unwrap() += 1;
            match self.audits.lock().unwrap().pop_front().expect("scripted audit verdict available") {
                Ok(result) => {
                    *capture.lock().unwrap() = Some(result);
                    Ok(PromptOutput { text: String::new(), tool_calls: vec![] })
                }
                Err(e) => Err(e),
            }
        }
    }

    /// Like [`service_with`], but with the pedagogical critic registered —
    /// the production registry always has it; `service_with` leaves it out
    /// so the pre-critic tests exercise the "critic unavailable" fail-open.
    fn service_with_critic(runner: Arc<dyn PromptRunner>) -> NotebookService {
        let agents = Arc::new(AgentRegistry::new());
        agents.register(crate::agents::notebook_agent::definition()).expect("register notebook agent");
        agents.register(crate::agents::notebook_gate_grader_agent::definition()).expect("register gate grader agent");
        agents.register(crate::agents::pedagogical_critic_agent::definition()).expect("register pedagogical critic");
        let orchestrator = Arc::new(Orchestrator::new(agents, runner));
        NotebookService::new(NotebookStore::open_in_memory().expect("open store"), orchestrator)
    }

    fn first_class_id(service: &NotebookService) -> String {
        let classes = service.import_course_from_roadmap(&sealed_roadmap_session()).expect("import");
        classes[0].id.clone()
    }

    /// Lets the current-thread test runtime poll a detached `tokio::spawn`
    /// background chain to completion — the mock runner resolves instantly,
    /// so a short sleep is enough to observe its effects deterministically
    /// in practice.
    async fn let_background_chain_settle() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    #[test]
    fn import_course_from_roadmap_seeds_one_class_per_micromodule() {
        let service = service_with(Arc::new(ScriptedNotebookRunner::default()));
        let classes = service.import_course_from_roadmap(&sealed_roadmap_session()).expect("import ok");
        assert_eq!(classes.len(), 2);
        assert_eq!(classes[0].title, "Semana 1: Días 1-2 — Panorama general");
    }

    #[test]
    fn import_fails_without_a_sealed_syllabus() {
        let service = service_with(Arc::new(ScriptedNotebookRunner::default()));
        let session = RoadmapSession::new("SID".to_string(), 0);
        let err = service.import_course_from_roadmap(&session).expect_err("must fail");
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn start_class_notebook_returns_block_one_fast_and_buffers_block_two_in_the_background() {
        let runner = Arc::new(ScriptedNotebookRunner {
            blocks: Mutex::new(vec![Some(micro_theory()), Some(prediction_gate("B"))].into()),
            ..Default::default()
        });
        let service = service_with(runner.clone());
        let class_id = first_class_id(&service);

        let payload = service.start_class_notebook(None, &class_id).await.expect("start ok");
        assert_eq!(payload.blocks.len(), 1, "only block 1 returned synchronously");
        assert_eq!(payload.blocks[0].block_type, DynamicBlockType::AnchoredMicroTheory);
        assert_eq!(payload.document.current_block_index, 1, "non-gate block 1 auto-unlocks immediately");

        let_background_chain_settle().await;
        let progress = service.get_class_notebook_progress(&class_id).expect("progress");
        assert_eq!(progress.blocks.len(), 2, "block 2 buffered in the background");
        assert_eq!(progress.blocks[1].block_type, DynamicBlockType::InteractivePredictionGate);
        assert_eq!(progress.document.current_block_index, 1, "the gate stays locked, cursor doesn't advance past it");
        assert_eq!(*runner.block_calls.lock().unwrap(), 2);
    }

    #[tokio::test]
    async fn starting_an_already_started_notebook_resumes_instead_of_regenerating() {
        let runner = Arc::new(ScriptedNotebookRunner { blocks: Mutex::new(vec![Some(micro_theory())].into()), ..Default::default() });
        let service = service_with(runner.clone());
        let class_id = first_class_id(&service);

        service.start_class_notebook(None, &class_id).await.expect("first start");
        let_background_chain_settle().await;
        let calls_after_first = *runner.block_calls.lock().unwrap();

        let resumed = service.start_class_notebook(None, &class_id).await.expect("resume");
        assert_eq!(resumed.blocks.len(), 1);
        assert_eq!(*runner.block_calls.lock().unwrap(), calls_after_first, "no new generation call on resume");
    }

    #[tokio::test]
    async fn reset_class_notebook_wipes_the_document_so_a_fresh_start_regenerates_from_scratch() {
        // 3 scripted blocks: block 1 + the first start's auto-buffered block
        // 2 (a gate, so the chain stops there) for the ORIGINAL document,
        // then one more for the fresh document the reset+restart creates.
        let runner = Arc::new(ScriptedNotebookRunner {
            blocks: Mutex::new(vec![Some(micro_theory()), Some(prediction_gate("B")), Some(micro_theory())].into()),
            ..Default::default()
        });
        let service = service_with(runner.clone());
        let class_id = first_class_id(&service);

        service.start_class_notebook(None, &class_id).await.expect("first start");
        let_background_chain_settle().await;
        assert!(service.get_class_notebook_progress(&class_id).is_ok(), "notebook exists after the first start");

        service.reset_class_notebook(&class_id).expect("reset ok");
        assert!(
            service.get_class_notebook_progress(&class_id).is_err(),
            "no notebook left to resume right after a reset — nothing was regenerated yet"
        );

        let restarted = service.start_class_notebook(None, &class_id).await.expect("restart after reset");
        assert_eq!(restarted.blocks.len(), 1, "a plain fresh start, not a resume of anything wiped");
    }

    #[tokio::test]
    async fn reset_class_notebook_rejects_an_unknown_class() {
        let service = service_with(Arc::new(ScriptedNotebookRunner::default()));
        let err = service.reset_class_notebook("does-not-exist").expect_err("unknown class must error");
        assert!(matches!(err, crate::error::AppError::InvalidInput(_)), "{err:?}");
    }

    #[tokio::test]
    async fn a_cursor_left_behind_by_a_lost_race_heals_on_reopen_and_the_gate_becomes_submittable() {
        let runner = Arc::new(ScriptedNotebookRunner {
            blocks: Mutex::new(vec![Some(micro_theory()), Some(prediction_gate("B"))].into()),
            grading: Mutex::new(vec![GateGradingResult { passed: true, rationale: "cumple la rúbrica".to_string(), criteria: vec![], scaffold: None }].into()),
            ..Default::default()
        });
        let service = service_with(runner);
        let class_id = first_class_id(&service);

        let payload = service.start_class_notebook(None, &class_id).await.expect("first block");
        let_background_chain_settle().await; // background chain buffers the gate
        let doc_id = payload.document.id.clone();

        // Reproduce the production wedge: a stale concurrent writer parked
        // the cursor back at block 0, so every gate submit was rejected
        // with "este bloque ya no es el bloque activo de la clase".
        service.store.set_document_current_index(&doc_id, 0).expect("stall cursor");
        let stalled = service.store.get_document(&doc_id).expect("read").expect("present");
        assert_eq!(stalled.current_block_index, 0, "raw document cursor is behind the persisted gate");

        let resumed = service.start_class_notebook(None, &class_id).await.expect("reopen");
        assert_eq!(resumed.document.current_block_index, 1, "reconcile walks the cursor back onto the gate");
        let healed = service.get_class_notebook_progress(&class_id).expect("progress");
        assert_eq!(healed.document.current_block_index, 1, "the heal is persisted, not just in the payload");

        let gate = healed
            .blocks
            .iter()
            .find(|b| b.block_type == DynamicBlockType::InteractivePredictionGate)
            .expect("the buffered gate is returned");
        let result = service
            .submit_gate_response(None, &gate.id, GateSubmission::InteractivePredictionGate { selected_option: "B".to_string() })
            .await
            .expect("the gate is submittable again");
        assert!(result.passed, "cursor heal restores the normal flow");
    }

    #[tokio::test]
    async fn correct_answer_passes_the_gate_advances_the_cursor_and_answer_key_never_reaches_the_client() {
        // metacognitive_closure can't legally land until the student has
        // passed BOTH a conceptual gate (interactive_prediction_gate) and a
        // practice gate (hands_on_mission) — see `grounding::mastery_progress`
        // — so this fixture exercises the full "infinite notebook" arc: gate
        // -> content -> gate -> closure, only once both are actually cleared.
        let runner = Arc::new(ScriptedNotebookRunner {
            blocks: Mutex::new(
                vec![Some(prediction_gate("B")), Some(micro_theory()), Some(hands_on_mission()), Some(transfer_mission()), Some(closure())].into(),
            ),
            grading: Mutex::new(vec![GateGradingResult { passed: true, rationale: "cumple la rúbrica".to_string(), criteria: vec![], scaffold: None }, GateGradingResult { passed: true, rationale: "cumple la rúbrica".to_string(), criteria: vec![], scaffold: None }].into()),
            ..Default::default()
        });
        let service = service_with(runner.clone());
        let class_id = first_class_id(&service);

        let payload = service.start_class_notebook(None, &class_id).await.expect("start");
        let gate_block = &payload.blocks[0];
        assert!(gate_block.content_json.get("correctOption").is_none(), "answer key must never reach the client");
        let gate_id = gate_block.id.clone();

        let result = service
            .submit_gate_response(None, &gate_id, GateSubmission::InteractivePredictionGate { selected_option: "B".to_string() })
            .await
            .expect("submit ok");
        assert!(result.passed);
        assert_eq!(result.block.status, BlockStatus::Passed);

        let_background_chain_settle().await;
        let progress = service.get_class_notebook_progress(&class_id).expect("progress");
        assert_eq!(progress.blocks.len(), 3, "micro_theory auto-unlocked, then stopped at the practice gate");
        assert_eq!(progress.document.current_block_index, 2, "the practice gate is still locked, awaiting submission");
        let mission_block = progress.blocks.last().unwrap();
        assert_eq!(mission_block.block_type, DynamicBlockType::HandsOnMission);

        let mission_result = service
            .submit_gate_response(None, &mission_block.id, GateSubmission::HandsOnMission { submission_text: "mi solución".to_string() })
            .await
            .expect("submit ok");
        assert!(mission_result.passed, "both mastery dimensions are now satisfied");

        // Mastery is not the end: the transfer challenge comes before the closure.
        let_background_chain_settle().await;
        let progress = service.get_class_notebook_progress(&class_id).expect("progress");
        assert_eq!(progress.blocks.len(), 4);
        let transfer_block = progress.blocks.last().unwrap();
        assert_eq!(transfer_block.block_type, DynamicBlockType::HandsOnMission);
        assert_eq!(transfer_block.content_json["isTransfer"], true);

        let transfer_result = service
            .submit_gate_response(None, &transfer_block.id, GateSubmission::HandsOnMission { submission_text: "caso nuevo".to_string() })
            .await
            .expect("submit ok");
        assert!(transfer_result.passed);

        let_background_chain_settle().await;
        let progress = service.get_class_notebook_progress(&class_id).expect("progress");
        assert_eq!(progress.blocks.len(), 5);
        assert_eq!(
            progress.document.current_block_index, 5,
            "advanced past the passed transfer gate and the auto-unlocked closing block"
        );
        assert_eq!(progress.blocks.last().unwrap().block_type, DynamicBlockType::MetacognitiveClosure);

        // The transfer attempt is logged as applied-evidence (first try, independent support, no hints seen).
        let status = service.get_course_skill_status(&service.list_courses().expect("courses")[0].id).expect("status");
        assert!(status[0].solved.achieved && status[0].applied.achieved, "{status:?}");
        assert!(!status[0].retained.achieved, "retention needs a later, written answer");
    }

    #[tokio::test]
    async fn wrong_answer_locks_the_gate_and_returns_a_hint_without_revealing_the_correct_option() {
        let runner = Arc::new(ScriptedNotebookRunner { blocks: Mutex::new(vec![Some(prediction_gate("B"))].into()), ..Default::default() });
        let service = service_with(runner.clone());
        let class_id = first_class_id(&service);
        let payload = service.start_class_notebook(None, &class_id).await.expect("start");
        let gate_id = payload.blocks[0].id.clone();

        let result = service
            .submit_gate_response(None, &gate_id, GateSubmission::InteractivePredictionGate { selected_option: "A".to_string() })
            .await
            .expect("submit ok");
        assert!(!result.passed);
        assert_eq!(result.block.status, BlockStatus::Failed);
        assert_eq!(result.block.attempt_count, 1);

        let progress = service.get_class_notebook_progress(&class_id).expect("progress");
        assert_eq!(progress.document.current_block_index, 0, "still locked, cursor unchanged");
    }

    #[tokio::test]
    async fn three_failed_attempts_escalate_to_a_re_approach_block_never_revealing_the_answer() {
        let runner = Arc::new(ScriptedNotebookRunner { blocks: Mutex::new(vec![Some(prediction_gate("B"))].into()), ..Default::default() });
        let service = service_with(runner.clone());
        let class_id = first_class_id(&service);
        let payload = service.start_class_notebook(None, &class_id).await.expect("start");
        let gate_id = payload.blocks[0].id.clone();

        for _ in 0..2 {
            let r = service
                .submit_gate_response(None, &gate_id, GateSubmission::InteractivePredictionGate { selected_option: "A".to_string() })
                .await
                .expect("submit ok");
            assert!(!r.passed);
            assert!(r.escalation_block.is_none());
        }

        // 3rd fail: the mock's next scripted block IS the re-approach block.
        runner.blocks.lock().unwrap().push_back(Some(hands_on_mission()));
        let third = service
            .submit_gate_response(None, &gate_id, GateSubmission::InteractivePredictionGate { selected_option: "A".to_string() })
            .await
            .expect("submit ok");
        assert!(!third.passed);
        assert_eq!(third.block.status, BlockStatus::Escalated);
        let escalation = third.escalation_block.expect("escalation block present on the 3rd fail");
        assert_eq!(escalation.block_type, DynamicBlockType::HandsOnMission);

        // Every attempt left a row; the 3rd is `escalated` (could continue),
        // never `passed`, and `hints_shown` counts the feedback rounds before it.
        let evidence = service.get_skill_evidence(&class_id).expect("evidence");
        let gate_rows: Vec<_> = evidence.iter().filter(|e| e.block_id.as_deref() == Some(gate_id.as_str())).collect();
        use crate::domain::skill_evidence::{EvidenceKind, EvidenceOutcome};
        assert_eq!(
            gate_rows.iter().map(|e| (e.attempt_number, e.hints_shown, e.outcome)).collect::<Vec<_>>(),
            vec![(1, 0, EvidenceOutcome::Failed), (2, 1, EvidenceOutcome::Failed), (3, 2, EvidenceOutcome::Escalated)]
        );
        assert!(gate_rows.iter().all(|e| e.kind == EvidenceKind::ConceptualGate && e.skill_id == class_id));
        assert!(gate_rows.iter().all(|e| !e.outcome.is_demonstration()));

        // Resubmitting the same escalated gate is rejected — it's locked forever, never retried.
        let err = service
            .submit_gate_response(None, &gate_id, GateSubmission::InteractivePredictionGate { selected_option: "B".to_string() })
            .await
            .expect_err("must reject a resolved gate");
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn free_text_gate_is_graded_by_the_model_and_its_scaffold_never_reveals_the_solution() {
        let runner = Arc::new(ScriptedNotebookRunner {
            blocks: Mutex::new(
                vec![Some(GeneratedSectionBlock::HeuristicErrorAudit {
                    instruction: "Encuentra el error".to_string(),
                    flawed_representation: crate::domain::notebook::FlawedRepresentation {
                        context: "Un caso típico".to_string(),
                        buggy_snippet_or_diagram: "x = x + 1 // nunca se ejecuta".to_string(),
                        error_type: crate::domain::notebook::FlawErrorType::ConceptualMisunderstanding,
                    },
                    guiding_questions: vec!["¿Qué falta?".to_string()],
                    model_solution: "La condición del bucle nunca se vuelve falsa".to_string(),
                    visual_aid: None,
                })]
                .into(),
            ),
            grading: Mutex::new(
                vec![GateGradingResult {
                    passed: false,
                    rationale: "no menciona la condición".to_string(),
                    criteria: vec![],
                    scaffold: Some(GateScaffold {
                        scaffold_type: ScaffoldType::SocraticHint,
                        content: "¿Qué tendría que pasar para que el bucle termine?".to_string(),
                    }),
                }]
                .into(),
            ),
            ..Default::default()
        });
        let service = service_with(runner.clone());
        let class_id = first_class_id(&service);
        let payload = service.start_class_notebook(None, &class_id).await.expect("start");
        assert!(payload.blocks[0].content_json.get("modelSolution").is_none(), "modelSolution hidden until passed");
        let block_id = payload.blocks[0].id.clone();

        let result = service
            .submit_gate_response(None, &block_id, GateSubmission::HeuristicErrorAudit { diagnosis_text: "algo está mal".to_string() })
            .await
            .expect("submit ok");
        assert!(!result.passed);
        assert_eq!(*runner.grading_calls.lock().unwrap(), 1);
        let feedback = result.feedback.expect("scaffold feedback present on fail");
        assert!(
            !feedback.to_lowercase().contains("condición del bucle nunca"),
            "scaffold must never quote the model_solution verbatim"
        );
    }

    #[tokio::test]
    async fn a_graded_attempt_stores_the_graders_rubric_judgment_and_the_support_level() {
        use crate::domain::skill_evidence::{EvidenceKind, EvidenceOutcome, RubricCriterion};
        let runner = Arc::new(ScriptedNotebookRunner {
            blocks: Mutex::new(vec![Some(hands_on_mission())].into()),
            grading: Mutex::new(
                vec![GateGradingResult {
                    passed: true,
                    rationale: "cumple".to_string(),
                    criteria: vec![RubricCriterion {
                        criterion: "Entrega un artefacto verificable".to_string(),
                        met: true,
                        evidence: Some("incluye el resultado de la prueba".to_string()),
                    }],
                    scaffold: None,
                }]
                .into(),
            ),
            ..Default::default()
        });
        let service = service_with(runner);
        let class_id = first_class_id(&service);
        let payload = service.start_class_notebook(None, &class_id).await.expect("start");
        let block_id = payload.blocks[0].id.clone();

        service
            .submit_gate_response(None, &block_id, GateSubmission::HandsOnMission { submission_text: "mi solución".to_string() })
            .await
            .expect("submit ok");

        let evidence = service.get_skill_evidence(&class_id).expect("evidence");
        assert_eq!(evidence.len(), 1);
        let row = &evidence[0];
        assert_eq!((row.kind, row.outcome), (EvidenceKind::PracticeGate, EvidenceOutcome::Passed));
        assert_eq!(row.rubric.len(), 1);
        assert!(row.rubric[0].met);
        assert_eq!(row.support_level.as_deref(), Some("full"), "first block of the class: no teaching before it");
        assert_eq!(row.skill_id, class_id);
    }

    /// Persists a retrieval block for `class_id` with one queued concept, the
    /// way a due-retrieval first block would exist after generation.
    fn retrieval_fixture(service: &NotebookService, class_id: &str) -> NotebookBlock {
        let doc = service.store.ensure_document_shell(class_id, "Clase").expect("shell");
        let mut memory = service.store.get_learner_memory(LOCAL_LEARNER_ID).expect("memory");
        memory.queue_retrieval(class_id, "Pilas LIFO", 0);
        service.store.save_learner_memory(&memory).expect("save");
        service
            .store
            .insert_block(
                &doc.id,
                DynamicBlockType::SpacedInterleavedRetrieval,
                &serde_json::json!({"blockType": "spaced_interleaved_retrieval",
                    "items": [{"conceptLabel": "Pilas LIFO", "prompt": "¿Qué sale primero?", "expectedAnswer": "El último en entrar"}]}),
                BlockStatus::Ready,
            )
            .expect("insert")
    }

    #[test]
    fn the_solution_never_reaches_the_client_before_the_student_answers() {
        let service = service_with(Arc::new(ScriptedNotebookRunner::default()));
        let class_id = first_class_id(&service);
        let block = retrieval_fixture(&service, &class_id);

        let before = grading::redact_block(&block);
        assert!(before.content_json["items"][0].get("expectedAnswer").is_none(), "answer must be withheld");
        assert_eq!(before.content_json["items"][0]["prompt"], "¿Qué sale primero?");

        let mut answered = block.clone();
        answered.content_json["items"][0]["reportedOutcome"] = serde_json::json!("correct");
        assert!(grading::redact_block(&answered).content_json["items"][0].get("expectedAnswer").is_some(), "revealed once answered");
    }

    #[tokio::test]
    async fn a_graded_written_answer_is_retrieval_evidence_and_can_only_be_given_once() {
        use crate::domain::skill_evidence::{EvidenceKind, EvidenceOutcome};
        let runner = Arc::new(ScriptedNotebookRunner {
            grading: Mutex::new(vec![GateGradingResult { passed: true, rationale: "ok".to_string(), criteria: vec![], scaffold: None }].into()),
            ..Default::default()
        });
        let service = service_with(runner.clone());
        let class_id = first_class_id(&service);
        let block = retrieval_fixture(&service, &class_id);
        let before = service.get_learner_memory().expect("memory").retrieval_spaced_queue[0].mastery_level;

        // Persisting/showing the block alone changed nothing.
        assert!(service.get_skill_evidence(&class_id).expect("evidence").is_empty());

        let r = service.submit_retrieval_answer(None, &block.id, 0, "el último que entró").await.expect("graded");
        assert!(r.correct);
        assert_eq!(r.expected_answer, "El último en entrar");
        assert_eq!(*runner.grading_calls.lock().unwrap(), 1);

        let evidence = service.get_skill_evidence(&class_id).expect("evidence");
        assert_eq!(evidence.len(), 1);
        assert_eq!((evidence[0].kind, evidence[0].outcome), (EvidenceKind::Retrieval, EvidenceOutcome::Passed));
        assert!(service.get_learner_memory().expect("memory").retrieval_spaced_queue[0].mastery_level > before);

        assert!(service.submit_retrieval_answer(None, &block.id, 0, "otra vez").await.is_err(), "a second answer is rejected");
        assert!(matches!(service.submit_retrieval_answer(None, &block.id, 0, "   ").await, Err(AppError::InvalidInput(_))));
        assert_eq!(service.get_skill_evidence(&class_id).expect("evidence").len(), 1);
    }

    #[test]
    fn giving_up_reveals_the_answer_records_a_miss_once_and_never_counts_as_retention() {
        use crate::domain::skill_evidence::EvidenceOutcome;
        let service = service_with(Arc::new(ScriptedNotebookRunner::default()));
        let class_id = first_class_id(&service);
        let block = retrieval_fixture(&service, &class_id);

        assert_eq!(service.reveal_retrieval_answer(&block.id, 0).expect("reveal"), "El último en entrar");
        assert_eq!(service.reveal_retrieval_answer(&block.id, 0).expect("idempotent"), "El último en entrar");

        let evidence = service.get_skill_evidence(&class_id).expect("evidence");
        assert_eq!(evidence.len(), 1, "revealing twice must not log twice");
        assert_eq!(evidence[0].outcome, EvidenceOutcome::SelfForgot);
        assert!(service.reveal_retrieval_answer(&block.id, 5).is_err(), "out-of-range item");
        let status = service.get_course_skill_status(&service.list_courses().expect("courses")[0].id).expect("status");
        assert!(status.iter().all(|s| !s.retained.achieved));
    }

    #[tokio::test]
    async fn submitting_to_a_stale_or_non_current_block_is_rejected() {
        let runner =
            Arc::new(ScriptedNotebookRunner { blocks: Mutex::new(vec![Some(prediction_gate("B"))].into()), ..Default::default() });
        let service = service_with(runner.clone());
        let class_id = first_class_id(&service);
        service.start_class_notebook(None, &class_id).await.expect("start");

        let err = service
            .submit_gate_response(None, "does-not-exist", GateSubmission::InteractivePredictionGate { selected_option: "B".to_string() })
            .await
            .expect_err("must fail for an unknown block id");
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn never_calling_the_tool_repeatedly_surfaces_an_error_instead_of_fabricating_content() {
        let runner = Arc::new(ScriptedNotebookRunner { blocks: Mutex::new(vec![None, None, None].into()), ..Default::default() });
        let service = service_with(runner.clone());
        let class_id = first_class_id(&service);

        let err = service.start_class_notebook(None, &class_id).await.expect_err("must surface, not fabricate");
        assert!(matches!(err, AppError::AgentExecutionFailed(_)));
        assert_eq!(*runner.block_calls.lock().unwrap(), 3, "used the full retry budget");
        // The document SHELL exists (created up front so a resume/retry has
        // somewhere to attach to — see `ensure_document_shell`), but no
        // fabricated content was ever persisted into it.
        let progress = service.get_class_notebook_progress(&class_id).expect("shell still resolvable");
        assert!(progress.blocks.is_empty(), "nothing was fabricated");
    }

    #[tokio::test]
    async fn a_grounding_violation_is_rejected_then_a_corrected_block_persists_on_retry() {
        let mut bad_gate = prediction_gate("Z"); // Z is not one of the options — grounding rejects it
        if let GeneratedSectionBlock::InteractivePredictionGate { correct_option, .. } = &mut bad_gate {
            *correct_option = "Z".to_string();
        }
        let runner = Arc::new(ScriptedNotebookRunner {
            blocks: Mutex::new(vec![Some(bad_gate), Some(prediction_gate("B"))].into()),
            ..Default::default()
        });
        let service = service_with(runner.clone());
        let class_id = first_class_id(&service);

        let payload = service.start_class_notebook(None, &class_id).await.expect("recovers on retry");
        assert_eq!(payload.blocks[0].block_type, DynamicBlockType::InteractivePredictionGate);
        assert_eq!(*runner.block_calls.lock().unwrap(), 2, "one rejected attempt + one corrected retry");
    }

    #[test]
    fn save_notebook_state_updates_content_without_touching_gating_state() {
        let service = service_with(Arc::new(ScriptedNotebookRunner::default()));
        let class_id = first_class_id(&service);
        let doc = service.store.ensure_document_shell(&class_id, "Clase 1").expect("shell");
        let content = serde_json::to_value(closure()).expect("serializes");
        let block = service.store.insert_block(&doc.id, DynamicBlockType::MetacognitiveClosure, &content, BlockStatus::Ready).expect("insert");

        service
            .save_notebook_state(&doc.id, &[BlockUpdate { id: block.id.clone(), content_json: serde_json::json!({"studentReflection": "listo"}) }])
            .expect("save ok");

        let reloaded = service.get_class_notebook_progress(&class_id).expect("load ok");
        let edited = reloaded.blocks.iter().find(|b| b.id == block.id).expect("block present");
        assert_eq!(edited.content_json, serde_json::json!({"studentReflection": "listo"}));
        assert_eq!(edited.status, BlockStatus::Ready, "content save never changes gating status");
    }

    #[tokio::test]
    async fn a_critic_rejection_retries_with_the_feedback_and_persists_the_corrected_block() {
        let runner = Arc::new(CriticRunner {
            blocks: Mutex::new(vec![Some(micro_theory()), Some(micro_theory())].into()),
            audits: Mutex::new(
                vec![
                    Ok(BlockAuditResult { accepted: false, feedback: vec!["el hook es vago".to_string()] }),
                    Ok(BlockAuditResult { accepted: true, feedback: vec![] }),
                ]
                .into(),
            ),
            ..Default::default()
        });
        let service = service_with_critic(runner.clone());
        let class_id = first_class_id(&service);

        let payload = service.start_class_notebook(None, &class_id).await.expect("recovers on the corrected retry");
        assert_eq!(payload.blocks[0].block_type, DynamicBlockType::AnchoredMicroTheory);
        assert_eq!(*runner.block_calls.lock().unwrap(), 2, "one semantically-rejected attempt + one corrected retry");
        assert_eq!(*runner.audit_calls.lock().unwrap(), 2, "the deterministic-clean block went to the critic both times");
        assert!(
            runner.inputs.lock().unwrap()[1].contains("el hook es vago"),
            "the critic's feedback is fed back to the generator as the corrective note"
        );
    }

    #[tokio::test]
    async fn an_unavailable_critic_fails_open_and_ships_the_block_on_the_first_attempt() {
        let runner = Arc::new(CriticRunner {
            blocks: Mutex::new(vec![Some(micro_theory())].into()),
            audits: Mutex::new(vec![Err(AppError::AgentExecutionFailed("critic is down".to_string()))].into()),
            ..Default::default()
        });
        let service = service_with_critic(runner.clone());
        let class_id = first_class_id(&service);

        let payload = service.start_class_notebook(None, &class_id).await.expect("fails open instead of blocking the class");
        assert_eq!(payload.blocks.len(), 1);
        assert_eq!(*runner.block_calls.lock().unwrap(), 1, "a critic ERROR must not burn a generation retry");
        assert_eq!(*runner.audit_calls.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn a_persistent_semantic_rejection_falls_back_to_the_last_grounded_block_instead_of_dead_ending() {
        let reject = || -> AppResult<BlockAuditResult> { Ok(BlockAuditResult { accepted: false, feedback: vec!["no llega".to_string()] }) };
        let runner = Arc::new(CriticRunner {
            blocks: Mutex::new(vec![Some(micro_theory()), Some(micro_theory()), Some(micro_theory())].into()),
            audits: Mutex::new(vec![reject(), reject(), reject()].into()),
            ..Default::default()
        });
        let service = service_with_critic(runner.clone());
        let class_id = first_class_id(&service);

        let payload = service.start_class_notebook(None, &class_id).await.expect("backstop ships the grounded candidate");
        assert_eq!(payload.blocks[0].block_type, DynamicBlockType::AnchoredMicroTheory);
        assert_eq!(*runner.block_calls.lock().unwrap(), 3, "burned the full attempt budget before the backstop");
        assert_eq!(*runner.audit_calls.lock().unwrap(), 3, "every attempt reached the semantic critic");
    }

    /// The repair path behind the notebook's "Este bloque no se pudo mostrar
    /// (formato inesperado) → Regenerar este bloque" affordance: the broken
    /// block is regenerated ON ITS OWN row — same id, same position, same
    /// type — and the whole class comes back so the UI re-renders in place.
    /// The scripted first block is a GATE so `should_auto_chain` never spawns
    /// a background chain that would eat the replacement scripted below.
    #[tokio::test]
    async fn regenerating_a_broken_block_repairs_it_in_place_without_moving_the_sequence() {
        let store = NotebookStore::open_in_memory().expect("store");
        let runner = Arc::new(ScriptedNotebookRunner {
            blocks: Mutex::new(vec![Some(prediction_gate("B")), Some(prediction_gate("A"))].into()),
            ..Default::default()
        });
        let service = service_with_store(store.clone(), runner.clone());
        let class_id = first_class_id(&service);

        let started = service.start_class_notebook(None, &class_id).await.expect("first block");
        assert_eq!(started.blocks.len(), 1);
        let broken = started.blocks[0].clone();
        assert_eq!(broken.block_type, DynamicBlockType::InteractivePredictionGate);
        let broken_json = serde_json::json!({"blockType": "interactive_prediction_gate", "broken": true});
        store.replace_block_payload(&broken.id, broken.block_type, &broken_json).expect("corrupt it");

        let payload = service.regenerate_block(None, &broken.id).await.expect("regenerated");

        assert_eq!(payload.blocks.len(), 1, "nothing was appended");
        let repaired = &payload.blocks[0];
        assert_eq!(repaired.id, broken.id, "same row");
        assert_eq!(repaired.order_index, broken.order_index, "same position");
        assert_eq!(repaired.block_type, broken.block_type, "same type");
        assert_eq!(repaired.status, BlockStatus::Ready);
        assert!(repaired.content_json.get("broken").is_none(), "the malformed payload is gone");
        assert!(repaired.content_json.get("question").is_some(), "a renderable payload took its place");
        assert_eq!(
            payload.document.current_block_index, started.document.current_block_index,
            "the gate cursor never moved"
        );
        assert_eq!(*runner.block_calls.lock().unwrap(), 2, "one initial generation + one repair");
    }

    /// `required_block_type` is what keeps a repair from silently swapping a
    /// gate for a content block: a candidate of any other type is rejected
    /// deterministically (no critic turn spent) until the right one arrives.
    #[tokio::test]
    async fn regenerating_rejects_a_wrong_block_type_until_the_required_one_arrives() {
        let store = NotebookStore::open_in_memory().expect("store");
        let runner = Arc::new(ScriptedNotebookRunner {
            blocks: Mutex::new(vec![Some(prediction_gate("B")), Some(micro_theory()), Some(prediction_gate("A"))].into()),
            ..Default::default()
        });
        let service = service_with_store(store.clone(), runner.clone());
        let class_id = first_class_id(&service);

        let started = service.start_class_notebook(None, &class_id).await.expect("first block");
        let broken = started.blocks[0].clone();
        store.replace_block_payload(&broken.id, broken.block_type, &serde_json::json!({"broken": true})).expect("corrupt it");

        let payload = service.regenerate_block(None, &broken.id).await.expect("recovers on the required type");

        assert_eq!(payload.blocks[0].block_type, DynamicBlockType::InteractivePredictionGate, "type pinned to the broken one");
        assert_eq!(*runner.block_calls.lock().unwrap(), 3, "initial + rejected wrong type + accepted replacement");
    }

    /// Never producing a valid candidate must NOT leave a half-written block
    /// behind: the broken content stays exactly as it was and the caller gets
    /// an error it can show next to the button.
    #[tokio::test]
    async fn a_regeneration_that_never_produces_a_valid_block_fails_leaving_the_broken_content_untouched() {
        let store = NotebookStore::open_in_memory().expect("store");
        let runner = Arc::new(ScriptedNotebookRunner {
            blocks: Mutex::new(vec![Some(prediction_gate("B"))].into()),
            ..Default::default()
        });
        let service = service_with_store(store.clone(), runner.clone());
        let class_id = first_class_id(&service);

        let started = service.start_class_notebook(None, &class_id).await.expect("first block");
        let broken = started.blocks[0].clone();
        let broken_json = serde_json::json!({"blockType": "interactive_prediction_gate", "broken": true});
        store.replace_block_payload(&broken.id, broken.block_type, &broken_json).expect("corrupt it");

        let err = service.regenerate_block(None, &broken.id).await.expect_err("must surface, not fabricate");
        assert!(matches!(err, AppError::AgentExecutionFailed(_)), "{err:?}");
        assert_eq!(*runner.block_calls.lock().unwrap(), 4, "initial + the full repair retry budget");

        let progress = service.get_class_notebook_progress(&class_id).expect("progress");
        assert_eq!(progress.blocks.len(), 1, "no block was appended or dropped");
        assert_eq!(progress.blocks[0].content_json, broken_json, "the broken payload is untouched until a valid one lands");
    }

    #[tokio::test]
    async fn the_next_class_unlocks_only_once_the_previous_one_is_practiced_and_closed() {
        let store = NotebookStore::open_in_memory().expect("store");
        let course = store.create_course("Dominio del Ciclo de Krebs", "Aprobar el examen", 1).expect("course");
        let milestone = store.create_milestone(&course.id, 1, "Semana 1", "Mapa visual").expect("milestone");
        let first = store.create_class(&milestone.id, 1, "Semana 1: la gran imagen", 0, 3.0).expect("first class");
        let second = store.create_class(&milestone.id, 1, "Semana 1: entradas y salidas", 1, 3.0).expect("second class");
        let runner = Arc::new(ScriptedNotebookRunner {
            blocks: Mutex::new(vec![Some(micro_theory())].into()),
            ..Default::default()
        });
        let service = service_with_store(store, runner.clone());

        let err = service
            .start_class_notebook(None, &second.id)
            .await
            .expect_err("must stay locked while the previous class is incomplete");
        assert!(matches!(err, AppError::InvalidInput(_)), "unexpected error: {err:?}");
        assert!(err.to_string().contains("Semana 1: la gran imagen"), "names the blocking class: {err}");
        assert_eq!(*runner.block_calls.lock().unwrap(), 0, "no generation may run for a locked class");
        let listed = service.list_course_classes(&course.id).expect("list");
        assert!(listed.iter().all(|c| !c.complete), "nothing is complete yet");

        let doc = service.store.ensure_document_shell(&first.id, &first.title).expect("shell");
        service
            .store
            .insert_block(
                &doc.id,
                DynamicBlockType::InteractivePredictionGate,
                &serde_json::json!({ "blockType": "interactive_prediction_gate" }),
                BlockStatus::Passed,
            )
            .expect("gate");
        let mut closed = serde_json::to_value(closure()).expect("closure json");
        closed["studentReflection"] = serde_json::json!("Ya distingo la entrada de la salida");
        service
            .store
            .insert_block(&doc.id, DynamicBlockType::MetacognitiveClosure, &closed, BlockStatus::Passed)
            .expect("closure");
        service.store.set_document_current_index(&doc.id, 2).expect("cursor past the closure");

        let listed = service.list_course_classes(&course.id).expect("list");
        assert!(listed[0].complete, "passed gates + revealed closure + written reflection + approved feedback = complete");
        assert!(!listed[1].complete, "the second class has no notebook yet");

        let payload = service.start_class_notebook(None, &second.id).await.expect("unlocked now");
        assert_eq!(payload.blocks.len(), 1, "the second class generates its first block");
        assert_eq!(*runner.block_calls.lock().unwrap(), 1, "exactly one generation ran");
    }

    #[tokio::test]
    async fn the_closure_feedback_is_the_last_hurdle_that_completes_the_class() {
        // Real arc to the closure: prediction gate -> content -> practice
        // gate -> closure (grounding rejects a closure before mastery).
        let runner = Arc::new(ScriptedNotebookRunner {
            blocks: Mutex::new(
                vec![Some(prediction_gate("B")), Some(micro_theory()), Some(hands_on_mission()), Some(transfer_mission()), Some(closure())].into(),
            ),
            grading: Mutex::new(
                vec![GateGradingResult { passed: true, rationale: "cumple la rúbrica".to_string(), criteria: vec![], scaffold: None }, GateGradingResult { passed: true, rationale: "cumple la rúbrica".to_string(), criteria: vec![], scaffold: None }].into(),
            ),
            closure_feedback: Mutex::new(
                vec![
                    ClosureFeedbackResult {
                        passed: false,
                        feedback: "Menciona qué cambió respecto a tu predicción antes de cerrar.".to_string(),
                    },
                    ClosureFeedbackResult {
                        passed: true,
                        feedback: "Conectaste tu predicción con lo observado — módulo completado.".to_string(),
                    },
                ]
                .into(),
            ),
            ..Default::default()
        });
        let service = service_with(runner.clone());
        let class_id = first_class_id(&service);
        let course_id = service.list_courses().expect("courses")[0].id.clone();

        let payload = service.start_class_notebook(None, &class_id).await.expect("start");
        let gate = &payload.blocks[0];
        service
            .submit_gate_response(None, &gate.id, GateSubmission::InteractivePredictionGate { selected_option: "B".to_string() })
            .await
            .expect("prediction gate passes");
        let_background_chain_settle().await;
        let progress = service.get_class_notebook_progress(&class_id).expect("progress");
        let mission = progress
            .blocks
            .iter()
            .find(|b| b.block_type == DynamicBlockType::HandsOnMission)
            .expect("practice gate buffered after the prediction gate");
        service
            .submit_gate_response(
                None,
                &mission.id,
                GateSubmission::HandsOnMission { submission_text: "mi solución".to_string() },
            )
            .await
            .expect("practice gate passes");
        let_background_chain_settle().await;
        let progress = service.get_class_notebook_progress(&class_id).expect("progress");
        let transfer = progress.blocks.last().expect("transfer mission buffered after mastery").clone();
        service
            .submit_gate_response(None, &transfer.id, GateSubmission::HandsOnMission { submission_text: "caso nuevo".to_string() })
            .await
            .expect("transfer gate passes");
        let_background_chain_settle().await;

        let progress = service.get_class_notebook_progress(&class_id).expect("progress");
        let closure_block = progress
            .blocks
            .iter()
            .find(|b| b.block_type == DynamicBlockType::MetacognitiveClosure)
            .expect("closure revealed after both gates");
        let listed = service.list_course_classes(&course_id).expect("list");
        assert!(!listed[0].complete, "no reflection and no feedback yet");

        let err = service
            .submit_closure_feedback(None, &closure_block.id, "   ")
            .await
            .expect_err("a blank reflection can't be graded");
        assert!(matches!(err, AppError::InvalidInput(_)), "unexpected error: {err:?}");

        let other = service
            .store
            .insert_block(
                &payload.document.id,
                DynamicBlockType::AnchoredMicroTheory,
                &serde_json::to_value(micro_theory()).expect("serializes"),
                BlockStatus::Ready,
            )
            .expect("non-closure block");
        let err = service
            .submit_closure_feedback(None, &other.id, "texto")
            .await
            .expect_err("only the closure takes closure feedback");
        assert!(matches!(err, AppError::InvalidInput(_)), "unexpected error: {err:?}");

        let failed = service
            .submit_closure_feedback(None, &closure_block.id, "Entendí todo.")
            .await
            .expect("graded");
        assert!(!failed.passed, "a vague reflection must not pass");
        assert!(!failed.feedback.is_empty(), "feedback is always student-facing");
        assert_eq!(failed.block.status, BlockStatus::Failed);
        let progress = service.get_class_notebook_progress(&class_id).expect("progress");
        let stored = progress.blocks.iter().find(|b| b.id == closure_block.id).expect("closure present");
        assert_eq!(stored.content_json["studentReflection"], serde_json::json!("Entendí todo."), "the graded text is persisted");
        assert_eq!(stored.status, BlockStatus::Failed);
        let listed = service.list_course_classes(&course_id).expect("list");
        assert!(!listed[0].complete, "a failed closure feedback keeps the class locked");

        let passed = service
            .submit_closure_feedback(None, &closure_block.id, "Predije A pero observé B, y ahora sé que C conecta ambas")
            .await
            .expect("graded");
        assert!(passed.passed, "a substantive reflection passes");
        assert_eq!(passed.block.status, BlockStatus::Passed);
        let listed = service.list_course_classes(&course_id).expect("list");
        assert!(listed[0].complete, "approved closure feedback completes the class");

        let calls_before = *runner.closure_calls.lock().unwrap();
        let again = service
            .submit_closure_feedback(None, &closure_block.id, "intentar de nuevo para borrarlo")
            .await
            .expect("re-grade is a no-op");
        assert!(again.passed, "an approved closure never re-opens");
        assert_eq!(again.feedback, passed.feedback, "the stored verdict stands");
        assert_eq!(*runner.closure_calls.lock().unwrap(), calls_before, "no second grader call after approval");
    }
