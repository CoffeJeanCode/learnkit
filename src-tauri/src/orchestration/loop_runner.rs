use std::time::Duration;

use async_trait::async_trait;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopSource {
    Model,
    Backstop,
}

#[derive(Debug)]
pub struct LoopReport<C> {
    pub value: C,
    pub source: LoopSource,
    pub attempts: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CriticVerdict {
    Accept,
    Reject { feedback: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cause {
    EmptyAttempt,
    OutputBudgetExhausted,
    Rejected { feedback: Vec<String> },
}

#[async_trait]
pub trait GeneratorCriticLoop: Send + Sync {
    type Candidate: Send + Sync;

    async fn generate(&self, note: &str) -> AppResult<Option<Self::Candidate>>;

    async fn critique(&self, candidate: &Self::Candidate) -> CriticVerdict;

    fn corrective_note(&self, cause: &Cause) -> String;

    fn backstop(&self, rejections: &[Cause]) -> Option<Self::Candidate>;

    fn exhausted_error(&self) -> AppError;
}

pub async fn run_generator_critic_loop<L>(job: &L, max_attempts: u8, backoff: Duration) -> AppResult<LoopReport<L::Candidate>>
where
    L: GeneratorCriticLoop,
{
    let budget = max_attempts.max(1);
    let mut note = String::new();
    let mut rejections: Vec<Cause> = Vec::new();
    let mut content_rejected = false;
    let mut last_err: Option<AppError> = None;

    for attempt in 0..budget {
        match job.generate(&note).await {
            Ok(Some(candidate)) => match job.critique(&candidate).await {
                CriticVerdict::Accept => {
                    return Ok(LoopReport { value: candidate, source: LoopSource::Model, attempts: attempt + 1 });
                }
                CriticVerdict::Reject { feedback } => {
                    content_rejected = true;
                    let cause = Cause::Rejected { feedback };
                    note = job.corrective_note(&cause);
                    rejections.push(cause);
                }
            },
            Ok(None) => {
                let cause = Cause::EmptyAttempt;
                note = job.corrective_note(&cause);
                rejections.push(cause);
            }
            Err(e) if e.is_output_budget_exhausted() && attempt + 1 < budget => {
                tracing::warn!(error = %e, attempt = attempt + 1, "generator ran out of output budget, retrying more concisely");
                let cause = Cause::OutputBudgetExhausted;
                note = job.corrective_note(&cause);
                rejections.push(cause);
            }
            Err(e) if e.is_transient() && attempt + 1 < budget => {
                let delay = backoff * (attempt as u32 + 1);
                tracing::warn!(error = %e, attempt = attempt + 1, backoff_ms = delay.as_millis(), "generator failed, retrying after backoff");
                tokio::time::sleep(delay).await;
            }
            Err(e) => {
                tracing::warn!(error = %e, attempt = attempt + 1, "generator failed permanently");
                last_err = Some(e);
                break;
            }
        }
    }

    if let Some(err) = last_err {
        return Err(err);
    }
    if content_rejected {
        if let Some(value) = job.backstop(&rejections) {
            tracing::warn!(attempts = budget, "generator budget exhausted after content rejections, falling back to deterministic backstop");
            return Ok(LoopReport { value, source: LoopSource::Backstop, attempts: budget });
        }
    }
    Err(job.exhausted_error())
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use super::*;

    enum Step {
        Emit(String),
        Empty,
        Fail(AppError),
    }

    struct ScriptedJob {
        steps: Mutex<VecDeque<Step>>,
        verdicts: Mutex<VecDeque<CriticVerdict>>,
        notes: Mutex<Vec<String>>,
        backstop_value: Option<String>,
    }

    impl ScriptedJob {
        fn new(steps: Vec<Step>, verdicts: Vec<CriticVerdict>) -> Self {
            Self {
                steps: Mutex::new(steps.into()),
                verdicts: Mutex::new(verdicts.into()),
                notes: Mutex::new(Vec::new()),
                backstop_value: Some("backstop".to_string()),
            }
        }
    }

    #[async_trait]
    impl GeneratorCriticLoop for ScriptedJob {
        type Candidate = String;

        async fn generate(&self, note: &str) -> AppResult<Option<String>> {
            self.notes.lock().unwrap().push(note.to_string());
            match self.steps.lock().unwrap().pop_front() {
                Some(Step::Emit(candidate)) => Ok(Some(candidate)),
                Some(Step::Empty) => Ok(None),
                Some(Step::Fail(e)) => Err(e),
                None => panic!("generate called past the scripted steps"),
            }
        }

        async fn critique(&self, _candidate: &String) -> CriticVerdict {
            self.verdicts.lock().unwrap().pop_front().unwrap_or(CriticVerdict::Accept)
        }

        fn corrective_note(&self, cause: &Cause) -> String {
            match cause {
                Cause::EmptyAttempt => "empty".to_string(),
                Cause::OutputBudgetExhausted => "budget".to_string(),
                Cause::Rejected { feedback } => format!("rejected: {}", feedback.join("; ")),
            }
        }

        fn backstop(&self, _rejections: &[Cause]) -> Option<String> {
            self.backstop_value.clone()
        }

        fn exhausted_error(&self) -> AppError {
            AppError::AgentExecutionFailed("job produced nothing usable".to_string())
        }
    }

    fn transient() -> AppError {
        AppError::ProviderConnectionFailed("reset".to_string())
    }

    fn budget_exhausted() -> AppError {
        AppError::AgentExecutionFailed("stopped with finish_reason=Length".to_string())
    }

    #[tokio::test]
    async fn accepts_on_the_first_attempt() {
        let job = ScriptedJob::new(vec![Step::Emit("ok".to_string())], vec![]);
        let report = run_generator_critic_loop(&job, 3, Duration::ZERO).await.expect("loop ok");
        assert_eq!(report.value, "ok");
        assert_eq!(report.source, LoopSource::Model);
        assert_eq!(report.attempts, 1);
    }

    #[tokio::test]
    async fn corrective_feedback_from_rejections_feeds_the_next_attempt() {
        let job = ScriptedJob::new(
            vec![Step::Emit("one".to_string()), Step::Emit("two".to_string()), Step::Emit("three".to_string())],
            vec![
                CriticVerdict::Reject { feedback: vec!["sesgo de longitud".to_string()] },
                CriticVerdict::Reject { feedback: vec!["meta-talk".to_string(), "opción corta".to_string()] },
            ],
        );
        let report = run_generator_critic_loop(&job, 3, Duration::ZERO).await.expect("loop ok");
        assert_eq!(report.value, "three");
        assert_eq!(report.source, LoopSource::Model);
        assert_eq!(report.attempts, 3);
        let notes = job.notes.lock().unwrap();
        assert_eq!(notes[0], "", "first attempt gets no corrective note");
        assert_eq!(notes[1], "rejected: sesgo de longitud");
        assert_eq!(notes[2], "rejected: meta-talk; opción corta");
    }

    #[tokio::test]
    async fn exhausted_rejections_fall_back_to_the_backstop() {
        let job = ScriptedJob::new(
            vec![Step::Emit("one".to_string()), Step::Emit("two".to_string()), Step::Emit("three".to_string())],
            vec![
                CriticVerdict::Reject { feedback: vec!["a".to_string()] },
                CriticVerdict::Reject { feedback: vec!["b".to_string()] },
                CriticVerdict::Reject { feedback: vec!["c".to_string()] },
            ],
        );
        let report = run_generator_critic_loop(&job, 3, Duration::ZERO).await.expect("loop ok");
        assert_eq!(report.value, "backstop");
        assert_eq!(report.source, LoopSource::Backstop);
        assert_eq!(report.attempts, 3);
    }

    #[tokio::test]
    async fn empty_attempts_alone_never_trigger_the_backstop() {
        let job = ScriptedJob::new(vec![Step::Empty, Step::Empty, Step::Empty], vec![]);
        let err = run_generator_critic_loop(&job, 3, Duration::ZERO).await.expect_err("must surface, not fabricate");
        assert!(matches!(err, AppError::AgentExecutionFailed(_)));
        assert_eq!(*job.notes.lock().unwrap(), vec!["", "empty", "empty"]);
    }

    #[tokio::test]
    async fn transient_errors_back_off_then_retry_with_unchanged_notes() {
        let job = ScriptedJob::new(vec![Step::Fail(transient()), Step::Fail(transient()), Step::Emit("ok".to_string())], vec![]);
        let report = run_generator_critic_loop(&job, 3, Duration::ZERO).await.expect("recovers");
        assert_eq!(report.value, "ok");
        assert_eq!(report.attempts, 3);
        assert_eq!(*job.notes.lock().unwrap(), vec!["", "", ""]);
    }

    #[tokio::test]
    async fn a_non_transient_error_fails_immediately_without_burning_the_budget() {
        let job = ScriptedJob::new(vec![Step::Fail(AppError::InvalidInput("bad".to_string()))], vec![]);
        let err = run_generator_critic_loop(&job, 3, Duration::ZERO).await.expect_err("must propagate");
        assert!(matches!(err, AppError::InvalidInput(_)));
        assert_eq!(job.notes.lock().unwrap().len(), 1, "no blind resend of a permanent failure");
    }

    #[tokio::test]
    async fn budget_exhaustion_demands_brevity_instead_of_a_blind_backoff() {
        let job = ScriptedJob::new(vec![Step::Fail(budget_exhausted()), Step::Emit("short".to_string())], vec![]);
        let report = run_generator_critic_loop(&job, 3, Duration::ZERO).await.expect("recovers");
        assert_eq!(report.value, "short");
        assert_eq!(report.attempts, 2);
        let notes = job.notes.lock().unwrap();
        assert_eq!(notes[0], "");
        assert_eq!(notes[1], "budget");
    }

    #[tokio::test]
    async fn budget_exhaustion_on_the_last_attempt_propagates_the_error() {
        let job = ScriptedJob::new(vec![Step::Emit("one".to_string()), Step::Fail(budget_exhausted()), Step::Fail(budget_exhausted())], vec![
            CriticVerdict::Reject { feedback: vec!["a".to_string()] },
        ]);
        let err = run_generator_critic_loop(&job, 3, Duration::ZERO).await.expect_err("last attempt error wins");
        assert!(err.is_output_budget_exhausted());
    }
}
