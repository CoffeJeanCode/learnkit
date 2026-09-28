use super::*;

    /// One scripted step: either the model asks a question (no tool call)
    /// or calls one/more tools. `assessment`/`diagnostic_battery` are
    /// expected to chain in the SAME step (Gate 1 + Gate 2) — or
    /// `assessment`/`propose_syllabus_plan` chain instead for the
    /// `absolute_zero` skip path. `propose_syllabus_plan` alone is Gate 3a
    /// (triggered by `answer_diagnostic_question`/`skip_diagnostic_battery`,
    /// not a normal chat turn); `confirm_syllabus_plan` alone is Gate 3b
    /// (a normal chat turn, once a proposal is pending).
    #[derive(Clone, Default)]
    pub(super) struct ScriptedStep {
        pub(super) text: String,
        pub(super) assessment: Option<DiagnosticAssessmentArgs>,
        pub(super) diagnostic_battery: Option<DiagnosticBattery>,
        pub(super) propose_syllabus_plan: Option<ProposeSyllabusPlanArgs>,
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
                    micromodules: vec![Micromodule {
                        label: "Módulo 1".to_string(),
                        hours,
                        deliverable: "Artefacto verificable".to_string(),
                        objective: Some("Explicar el concepto central del módulo con un ejemplo propio".to_string()),
                        interactive_blocks: vec![
                            "socratic_prediction".to_string(),
                            "hands_on_mission".to_string(),
                            "metacognitive_closure".to_string(),
                        ],
                    }],
                })
                .collect(),
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

    /// A scripted step carrying ONLY `propose_syllabus_plan` — what Gate 3a
    /// (triggered by `answer_diagnostic_question`/`skip_diagnostic_battery`,
    /// or chained with `assessment` for the `absolute_zero` skip path)
    /// expects.
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
            c.confirm_syllabus_plan = step.confirm_syllabus_plan;
            Ok(PromptOutput { text: step.text, tool_calls: vec![] })
        }
    }
