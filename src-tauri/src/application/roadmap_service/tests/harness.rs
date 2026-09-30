use super::*;

    /// One scripted step: either the model asks a question (no tool call)
    /// or calls one/more tools. `assessment`/`diagnostic_battery` are
    /// expected to chain in the SAME step (Gate 1 + Gate 2) — or
    /// `assessment`/`propose_syllabus_plan` chain instead for the
    /// `absolute_zero` skip path. `propose_syllabus_plan` always expects
    /// `propose_capstone_project` alongside it in the SAME step (see
    /// `tools::roadmap_tools::ProposeCapstoneProjectTool` — split apart only
    /// to keep each completion small, both still fire together); a step
    /// missing one while carrying the other is exactly how the
    /// "capstone omitted" rejection test drives that scenario.
    /// `confirm_syllabus_plan` alone is Gate 3b (a normal chat turn, once a
    /// proposal is pending).
    #[derive(Clone, Default)]
    pub(super) struct ScriptedStep {
        pub(super) text: String,
        pub(super) assessment: Option<DiagnosticAssessmentArgs>,
        pub(super) diagnostic_battery: Option<DiagnosticBattery>,
        pub(super) propose_syllabus_plan: Option<ProposeSyllabusPlanArgs>,
        pub(super) propose_capstone_project: Option<CapstoneProjectArgs>,
        pub(super) confirm_syllabus_plan: Option<ConfirmSyllabusPlanArgs>,
    }

    pub(super) struct ScriptedRunner {
        steps: Mutex<VecDeque<ScriptedStep>>,
        calls: Mutex<usize>,
    }

    impl ScriptedRunner {
        pub(super) fn new(steps: Vec<ScriptedStep>) -> Self {
            Self { steps: Mutex::new(steps.into_iter().collect()), calls: Mutex::new(0) }
        }
    }

    #[async_trait]
    impl PromptRunner for ScriptedRunner {
        async fn run(
            &self,
            _provider_id: &str,
            _model: &str,
            _system_prompt: &str,
            _input: &str,
            _tools: &[String],
        ) -> AppResult<PromptOutput> {
            unreachable!("roadmap service only drives run_diagnostic_execution now")
        }

        async fn run_diagnostic_execution(
            &self,
            _provider_id: &str,
            _model: &str,
            _system_prompt: &str,
            _input: &str,
            capture: SharedRoadmapCapture,
            _scope: DiagnosticToolScope,
        ) -> AppResult<PromptOutput> {
            *self.calls.lock().unwrap() += 1;
            let step = self.steps.lock().unwrap().pop_front().expect("scripted step available");
            let mut c = capture.lock().unwrap();
            c.assessment = step.assessment;
            c.diagnostic_battery = step.diagnostic_battery;
            c.propose_syllabus_plan = step.propose_syllabus_plan;
            c.propose_capstone_project = step.propose_capstone_project;
            c.confirm_syllabus_plan = step.confirm_syllabus_plan;
            Ok(PromptOutput { text: step.text, tool_calls: vec![] })
        }
    }

    pub(super) fn service_with_runner(runner: Arc<dyn PromptRunner>) -> RoadmapServiceHarness {
        let agents = Arc::new(AgentRegistry::new());
        agents.register(crate::agents::roadmap_agent::definition()).expect("register roadmap agent");
        agents.register(crate::agents::analyst_agent::definition()).expect("register analyst agent");
        let orchestrator = Arc::new(Orchestrator::new(agents, runner));
        let dir = std::env::temp_dir().join(format!("learnkit-roadmap-{}", Uuid::new_v4()));
        let store = FileStore::new(dir.clone());
        let notebook_store = NotebookStore::open_in_memory().expect("open notebook store");
        RoadmapServiceHarness { service: RoadmapService::new(orchestrator, store, notebook_store), dir }
    }

    pub(super) struct RoadmapServiceHarness {
        pub(super) service: RoadmapService,
        pub(super) dir: std::path::PathBuf,
    }

    pub(super) fn full_assessment(topic: &str, weeks: u16, hours: f32, entry_level: EntryLevel) -> DiagnosticAssessmentArgs {
        DiagnosticAssessmentArgs {
            topic: topic.to_string(),
            target_goal: "Aprobar el examen".to_string(),
            entry_level,
            timeframe_weeks: weeks,
            weekly_commitment_hours: hours,
        }
    }

    pub(super) fn full_syllabus(weeks: u16, hours: f32, topic: &str) -> RoadmapSyllabusPackage {
        RoadmapSyllabusPackage {
            course_title: topic.to_string(),
            total_weeks: weeks,
            pace_hours_per_week: hours,
            milestones: (1..=weeks)
                .map(|week| Milestone {
                    week,
                    title: format!("Semana {week}"),
                    deliverable: "Entregable de la semana".to_string(),
                    weekly_goal: Some("Avanzar un paso concreto hacia la meta declarada".to_string()),
                    micromodules: vec![
                        Micromodule {
                            label: "Días 1-2".to_string(),
                            hours: hours / 2.0,
                            focus: Some("Conceptos centrales de la primera sesión".to_string()),
                            deliverable: Deliverable {
                                artifact_type: DeliverableArtifactType::TestsPassing,
                                description: "Artefacto verificable".to_string(),
                            },
                            objective: Some("Explicar el concepto central de la sesión con un ejemplo propio".to_string()),
                            interactive_blocks: vec![
                                "socratic_prediction".to_string(),
                                "hands_on_mission".to_string(),
                                "metacognitive_closure".to_string(),
                            ],
                        },
                        Micromodule {
                            label: "Días 3-4".to_string(),
                            hours: hours / 2.0,
                            focus: Some("Profundización y casos límite".to_string()),
                            deliverable: Deliverable {
                                artifact_type: DeliverableArtifactType::TestsPassing,
                                description: "Artefacto verificable 2".to_string(),
                            },
                            objective: Some("Resolver un caso límite del concepto central".to_string()),
                            interactive_blocks: vec![
                                "error_audit_challenge".to_string(),
                                "hands_on_mission".to_string(),
                                "metacognitive_closure".to_string(),
                            ],
                        },
                    ],
                })
                .collect(),
            capstone_project: CapstoneProject {
                title: "Proyecto terminal".to_string(),
                description: "Integrar lo aprendido en un escenario real de transferencia".to_string(),
                verifiable_evidence: "Repositorio con la app corriendo + demo grabada".to_string(),
            },
        }
    }

    pub(super) fn full_battery() -> DiagnosticBattery {
        DiagnosticBattery {
            goal_alignment: "Mide la brecha".to_string(),
            questions: vec![
                DiagnosticQuestion {
                    dimension: DiagnosticDimension::Intuition,
                    prompt: "p1".to_string(),
                    options: vec!["A".to_string(), "B".to_string()],
                    correct_option: "A".to_string(),
                    diagnostic_insight: "insight 1".to_string(),
                },
                DiagnosticQuestion {
                    dimension: DiagnosticDimension::Mechanics,
                    prompt: "p2".to_string(),
                    options: vec!["A".to_string(), "B".to_string()],
                    correct_option: "B".to_string(),
                    diagnostic_insight: "insight 2".to_string(),
                },
                DiagnosticQuestion {
                    dimension: DiagnosticDimension::CriticalCase,
                    prompt: "p3".to_string(),
                    options: vec!["A".to_string(), "B".to_string()],
                    correct_option: "A".to_string(),
                    diagnostic_insight: "insight 3".to_string(),
                },
            ],
        }
    }

    /// A valid capstone args value — what `propose_capstone_project` is
    /// expected to carry alongside `propose_syllabus_plan` in the SAME step
    /// (see `ScriptedStep`'s doc comment).
    pub(super) fn full_capstone_args() -> CapstoneProjectArgs {
        CapstoneProjectArgs {
            capstone_project: CapstoneProject {
                title: "Proyecto terminal".to_string(),
                description: "Integrar lo aprendido en un escenario real de transferencia".to_string(),
                verifiable_evidence: "Repositorio con la app corriendo + demo grabada".to_string(),
            },
        }
    }

    /// A scripted step carrying `propose_syllabus_plan` + `propose_capstone_project`
    /// together — what Gate 3a (triggered by
    /// `answer_diagnostic_question`/`skip_diagnostic_battery`, or chained
    /// with `assessment` for the `absolute_zero` skip path) expects.
    pub(super) fn full_propose_step(text: &str, weeks: u16, hours: f32, topic: &str) -> ScriptedStep {
        ScriptedStep {
            text: text.to_string(),
            assessment: None,
            diagnostic_battery: None,
            propose_syllabus_plan: Some(ProposeSyllabusPlanArgs {
                core_focus: "Lo esencial".to_string(),
                identified_needs: vec!["Entender el flujo".to_string()],
                learning_strategy: "Guiado".to_string(),
                syllabus: full_syllabus(weeks, hours, topic),
                closing_question: "¿Te parece adecuada esta distribución?".to_string(),
            }),
            propose_capstone_project: Some(full_capstone_args()),
            confirm_syllabus_plan: None,
        }
    }

    /// A scripted step carrying ONLY `confirm_syllabus_plan` — Gate 3b, a
    /// normal chat turn once a proposal is pending.
    pub(super) fn full_confirm_step(text: &str) -> ScriptedStep {
        ScriptedStep {
            text: text.to_string(),
            assessment: None,
            diagnostic_battery: None,
            propose_syllabus_plan: None,
            propose_capstone_project: None,
            confirm_syllabus_plan: Some(ConfirmSyllabusPlanArgs {}),
        }
    }

    /// Drives the Diagnostic stage to completion by answering every question
    /// with its own `correctOption` — returns the LAST answer's result,
    /// which (once the battery is fully answered) is Gate 3a's (propose)
    /// result — the session has a `proposed_plan` pending, NOT sealed yet;
    /// a separate `confirm_syllabus_plan`-scripted `send_message` call seals
    /// it.
    pub(super) async fn answer_all_questions(harness: &RoadmapServiceHarness, session_id: &str, battery: &DiagnosticBattery) -> RoadmapTurnResult {
        let mut result = None;
        for (i, q) in battery.questions.iter().enumerate() {
            result = Some(
                harness
                    .service
                    .answer_diagnostic_question(None, session_id, i, &q.correct_option)
                    .await
                    .expect("answer ok"),
            );
        }
        result.expect("battery must have at least one question")
    }


    /// Succeeds exactly once, returning the given step, then fails hard
    /// (non-transient — no retry budget spent) on every call after that.
    /// For proving a checkpoint saved right after the first gate lands is
    /// durable even when the very NEXT model call — chained in the SAME
    /// `advance()`/`send_message()` invocation, like the `absolute_zero`
    /// assessment-then-propose hand-off — dies for good.
    pub(super) struct SucceedOnceThenFailRunner {
        pub(super) first_step: Mutex<Option<ScriptedStep>>,
        pub(super) calls: Mutex<usize>,
    }

    #[async_trait]
    impl PromptRunner for SucceedOnceThenFailRunner {
        async fn run(&self, _: &str, _: &str, _: &str, _: &str, _: &[String]) -> AppResult<PromptOutput> {
            unreachable!()
        }

        async fn run_diagnostic_execution(
            &self,
            _provider_id: &str,
            _model: &str,
            _system_prompt: &str,
            _input: &str,
            capture: SharedRoadmapCapture,
            _scope: DiagnosticToolScope,
        ) -> AppResult<PromptOutput> {
            *self.calls.lock().unwrap() += 1;
            let mut first = self.first_step.lock().unwrap();
            if let Some(step) = first.take() {
                let mut c = capture.lock().unwrap();
                c.assessment = step.assessment;
                c.diagnostic_battery = step.diagnostic_battery;
                c.propose_syllabus_plan = step.propose_syllabus_plan;
                c.propose_capstone_project = step.propose_capstone_project;
                c.confirm_syllabus_plan = step.confirm_syllabus_plan;
                return Ok(PromptOutput { text: step.text, tool_calls: vec![] });
            }
            Err(AppError::ProviderKeyMissing("openai".to_string()))
        }
    }

    pub(super) struct FlakyRunner {
        pub(super) failures_left: Mutex<usize>,
        pub(super) transient: bool,
        pub(super) step: ScriptedStep,
        pub(super) calls: Mutex<usize>,
    }

    #[async_trait]
    impl PromptRunner for FlakyRunner {
        async fn run(&self, _: &str, _: &str, _: &str, _: &str, _: &[String]) -> AppResult<PromptOutput> {
            unreachable!()
        }

        async fn run_diagnostic_execution(
            &self,
            _provider_id: &str,
            _model: &str,
            _system_prompt: &str,
            _input: &str,
            capture: SharedRoadmapCapture,
            _scope: DiagnosticToolScope,
        ) -> AppResult<PromptOutput> {
            *self.calls.lock().unwrap() += 1;
            let mut left = self.failures_left.lock().unwrap();
            if *left > 0 {
                *left -= 1;
                return Err(if self.transient {
                    AppError::AgentExecutionFailed("simulated tool call failure".to_string())
                } else {
                    AppError::ProviderKeyMissing("openai".to_string())
                });
            }
            let mut c = capture.lock().unwrap();
            c.assessment = self.step.assessment.clone();
            c.diagnostic_battery = self.step.diagnostic_battery.clone();
            c.propose_syllabus_plan = self.step.propose_syllabus_plan.clone();
            c.propose_capstone_project = self.step.propose_capstone_project.clone();
            c.confirm_syllabus_plan = self.step.confirm_syllabus_plan.clone();
            Ok(PromptOutput { text: self.step.text.clone(), tool_calls: vec![] })
        }
    }

    /// Scripted steps WITH a transient failure window that can be opened and
    /// closed mid-test: the next `fail_next` calls die exactly like a network
    /// blip (`HttpError: error sending request`), every later call pops the
    /// next step in order. `FlakyRunner` is all-or-nothing from construction,
    /// so it can't express "the model died precisely at the final battery
    /// answer, then recovered" — this can.
    pub(super) struct SeqRunner {
        pub(super) fail_next: Mutex<usize>,
        pub(super) steps: Mutex<VecDeque<ScriptedStep>>,
        pub(super) calls: Mutex<usize>,
    }

    impl SeqRunner {
        pub(super) fn new(steps: Vec<ScriptedStep>) -> Self {
            Self { fail_next: Mutex::new(0), steps: Mutex::new(steps.into_iter().collect()), calls: Mutex::new(0) }
        }

        /// Arms (or clears, with `0`) the next N calls to fail transiently.
        pub(super) fn fail_next(&self, n: usize) {
            *self.fail_next.lock().unwrap() = n;
        }
    }

    #[async_trait]
    impl PromptRunner for SeqRunner {
        async fn run(&self, _: &str, _: &str, _: &str, _: &str, _: &[String]) -> AppResult<PromptOutput> {
            unreachable!()
        }

        async fn run_diagnostic_execution(
            &self,
            _provider_id: &str,
            _model: &str,
            _system_prompt: &str,
            _input: &str,
            capture: SharedRoadmapCapture,
            _scope: DiagnosticToolScope,
        ) -> AppResult<PromptOutput> {
            *self.calls.lock().unwrap() += 1;
            let mut left = self.fail_next.lock().unwrap();
            if *left > 0 {
                *left -= 1;
                return Err(AppError::AgentExecutionFailed(
                    "CompletionError: HttpError: Http client error: error sending request for url (https://api.deepseek.com/chat/completions)"
                        .to_string(),
                ));
            }
            drop(left);
            let step = self.steps.lock().unwrap().pop_front().expect("scripted step available");
            let mut c = capture.lock().unwrap();
            c.assessment = step.assessment;
            c.diagnostic_battery = step.diagnostic_battery;
            c.propose_syllabus_plan = step.propose_syllabus_plan;
            c.propose_capstone_project = step.propose_capstone_project;
            c.confirm_syllabus_plan = step.confirm_syllabus_plan;
            Ok(PromptOutput { text: step.text, tool_calls: vec![] })
        }
    }
