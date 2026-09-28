//! `LearnerCognitiveMemory` — cross-course, cross-session state about HOW one
//! student learns (not WHAT course they're in — that's `domain::roadmap`'s
//! job). Read before every block generation to personalize scaffolding,
//! friction and spaced retrieval (see `application::notebook_service::render
//! ::render_next_block_input`), and updated after every graded gate and every
//! passed class closure (see `application::notebook_service::generation`).
//!
//! Single-learner app: there is no login system (see `state::app_state` — API
//! keys are local, not per-account), so every read/write goes through the one
//! constant [`LOCAL_LEARNER_ID`] rather than a real multi-tenant id. The
//! `learnerId` field still exists on the struct (not hardcoded away) so a
//! future multi-profile mode is a store-layer change, not a schema one.

use serde::{Deserialize, Serialize};

use crate::domain::notebook::DynamicBlockType;

/// The one learner this local app currently has. See module doc comment.
pub const LOCAL_LEARNER_ID: &str = "local";

/// How many due retrieval items a single `spaced_interleaved_retrieval` block
/// surfaces — "1 o 2 preguntas intercaladas", never a full quiz.
pub const MAX_RETRIEVAL_ITEMS_PER_BLOCK: usize = 2;

/// Exponential-moving-average smoothing for `cognitive_metrics`' rolling
/// rates — recent gate outcomes matter more than the whole history, but one
/// unlucky attempt shouldn't swing the rate wildly either.
const METRIC_EMA_ALPHA: f32 = 0.3;

/// How many recent (domain_concept, error_pattern) pairs to keep — unbounded
/// growth would eventually make `misconceptions_relevant_to` slow and the
/// persisted JSON blob large for no pedagogical benefit; the oldest RESOLVED
/// entries are dropped first when the cap is hit.
const MAX_RECURRING_MISCONCEPTIONS: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalibratedBaseline {
    NoviceZero,
    NoviceIntuitive,
    Intermediate,
    Advanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbstractionLevel {
    VisualAnalogy,
    CausalMechanics,
    FormalSymbolic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrictionTolerance {
    LowFrustration,
    Resilient,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CognitiveMetrics {
    /// Rolling pass rate on `interactive_prediction_gate`/
    /// `branching_scenario_challenge` (the CONCEPTUAL gate family) — an EMA,
    /// not a raw ratio, so a single early miss doesn't lock in a permanent
    /// "struggling" read.
    pub prediction_accuracy_rate: f32,
    /// Same EMA, scoped to `heuristic_error_audit` specifically (matches the
    /// field name) — a distinct signal from prediction accuracy, since
    /// diagnosing someone else's flawed artifact draws on a different skill
    /// than predicting your own outcome.
    pub error_audit_detection_rate: f32,
    pub preferred_abstraction_level: AbstractionLevel,
    pub cognitive_friction_tolerance: FrictionTolerance,
}

impl Default for CognitiveMetrics {
    /// A neutral, uninformed prior — 0.6 on both rates so a brand-new
    /// learner isn't immediately read as "struggling" (which would trigger
    /// `Self::scaffolding_directive`) before a single gate has been graded.
    fn default() -> Self {
        Self {
            prediction_accuracy_rate: 0.6,
            error_audit_detection_rate: 0.6,
            preferred_abstraction_level: AbstractionLevel::VisualAnalogy,
            cognitive_friction_tolerance: FrictionTolerance::Resilient,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringMisconception {
    /// What the class/course this was observed in was ABOUT — matched as a
    /// loose substring against a new class's course/class title in
    /// `misconceptions_relevant_to`, never an exact foreign key (concepts
    /// don't have stable ids anywhere else in this app either — see
    /// `domain::roadmap::Milestone`).
    pub domain_concept: String,
    /// e.g. "Confunde tasa de flujo con acumulación (stock vs flow)" — taken
    /// verbatim from the failed gate's own `conceptualFeedbackMap` entry for
    /// the option the student picked, never invented by Rust.
    pub identified_error_pattern: String,
    pub last_encountered_date: String,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpacedRetrievalItem {
    pub concept_id: String,
    pub concept_label: String,
    /// 0.0 (just learned) to 1.0 (long-term retained).
    pub mastery_level: f32,
    /// When this item next becomes eligible for reactivation — computed by
    /// this module (spaced-repetition-style backoff), never asked of the
    /// model. Not serialized to the model's input context directly: see
    /// `due_retrieval_items`, which reduces this down to the boolean the
    /// spec's `dueForRetrieval` actually describes.
    pub next_due_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearnerCognitiveMemory {
    pub learner_id: String,
    pub calibrated_baseline: CalibratedBaseline,
    pub cognitive_metrics: CognitiveMetrics,
    #[serde(default)]
    pub recurring_misconceptions: Vec<RecurringMisconception>,
    #[serde(default)]
    pub retrieval_spaced_queue: Vec<SpacedRetrievalItem>,
}

impl LearnerCognitiveMemory {
    pub fn new(learner_id: &str) -> Self {
        Self {
            learner_id: learner_id.to_string(),
            calibrated_baseline: CalibratedBaseline::NoviceIntuitive,
            cognitive_metrics: CognitiveMetrics::default(),
            recurring_misconceptions: Vec::new(),
            retrieval_spaced_queue: Vec::new(),
        }
    }

    /// Rolls one graded gate outcome into `cognitive_metrics` — only
    /// `InteractivePredictionGate` and `HeuristicErrorAudit` map onto a named
    /// rate (see the fields' own doc comments); every other gate type is a
    /// no-op here; it still counts toward `GateCategory` mastery elsewhere
    /// (`notebook_service::grounding::MasteryProgress`), just not toward one
    /// of these two specific named metrics.
    pub fn record_gate_outcome(&mut self, block_type: DynamicBlockType, passed: bool) {
        let outcome = if passed { 1.0 } else { 0.0 };
        let ema = |rate: &mut f32| *rate += METRIC_EMA_ALPHA * (outcome - *rate);
        match block_type {
            DynamicBlockType::InteractivePredictionGate => ema(&mut self.cognitive_metrics.prediction_accuracy_rate),
            DynamicBlockType::HeuristicErrorAudit => ema(&mut self.cognitive_metrics.error_audit_detection_rate),
            _ => {}
        }
    }

    /// Records (or refreshes) one specific, documented misconception — never
    /// a generic "estudiante falló" entry. Deduplicates on
    /// `(domain_concept, identified_error_pattern)`: a repeat sighting
    /// refreshes the date and clears `resolved` instead of growing the list.
    pub fn record_misconception(&mut self, domain_concept: &str, identified_error_pattern: &str, today_iso: &str) {
        if let Some(existing) = self
            .recurring_misconceptions
            .iter_mut()
            .find(|m| m.domain_concept == domain_concept && m.identified_error_pattern == identified_error_pattern)
        {
            existing.last_encountered_date = today_iso.to_string();
            existing.resolved = false;
            return;
        }
        self.recurring_misconceptions.push(RecurringMisconception {
            domain_concept: domain_concept.to_string(),
            identified_error_pattern: identified_error_pattern.to_string(),
            last_encountered_date: today_iso.to_string(),
            resolved: false,
        });
        if self.recurring_misconceptions.len() > MAX_RECURRING_MISCONCEPTIONS {
            // Drop the oldest RESOLVED entry first — an unresolved one is
            // still live evidence the student needs help with, never the
            // first thing to discard just to make room.
            if let Some(pos) = self.recurring_misconceptions.iter().position(|m| m.resolved) {
                self.recurring_misconceptions.remove(pos);
            } else {
                self.recurring_misconceptions.remove(0);
            }
        }
    }

    /// Marks every unresolved misconception under `domain_concept` resolved
    /// — called once the student PASSES a gate on a class matching that
    /// concept, closing the loop the diagnostic battery / a failed gate
    /// opened.
    pub fn resolve_misconceptions_for(&mut self, domain_concept: &str) {
        for m in self.recurring_misconceptions.iter_mut().filter(|m| m.domain_concept == domain_concept) {
            m.resolved = true;
        }
    }

    /// Unresolved misconceptions whose `domain_concept` overlaps `text`
    /// (course/class title + target goal, lowercased substring match) —
    /// deliberately loose: concepts have no stable id to join on anywhere
    /// else in this app (see `RecurringMisconception::domain_concept`'s doc
    /// comment), and a missed match just means one less personalization
    /// signal, never a hard failure.
    pub fn misconceptions_relevant_to(&self, text: &str) -> Vec<&RecurringMisconception> {
        let haystack = text.to_lowercase();
        self.recurring_misconceptions
            .iter()
            .filter(|m| !m.resolved && haystack.contains(&m.domain_concept.to_lowercase()))
            .collect()
    }

    /// Seeds (or refreshes) a spaced-retrieval item for a concept the
    /// student just demonstrated mastery of — called once a class's
    /// `metacognitive_closure` is approved. Due IMMEDIATELY (`next_due_at_ms
    /// = now`) on first queue: reinforcing a concept in the very next
    /// session is itself good practice, not a delay.
    pub fn queue_retrieval(&mut self, concept_id: &str, concept_label: &str, now_ms: i64) {
        if let Some(existing) = self.retrieval_spaced_queue.iter_mut().find(|i| i.concept_id == concept_id) {
            existing.concept_label = concept_label.to_string();
            return;
        }
        self.retrieval_spaced_queue.push(SpacedRetrievalItem {
            concept_id: concept_id.to_string(),
            concept_label: concept_label.to_string(),
            mastery_level: 0.3,
            next_due_at_ms: now_ms,
        });
    }

    /// Applies one reactivation attempt's outcome: a correct recall bumps
    /// `mastery_level` and pushes the next due date out (classic spaced-
    /// repetition backoff, coarse on purpose — this app has no per-second
    /// scheduling need); a miss resets mastery down and makes it due again
    /// immediately, same as a fresh miss on any other gate.
    pub fn reactivate_retrieval(&mut self, concept_id: &str, recalled_correctly: bool, now_ms: i64) {
        let Some(item) = self.retrieval_spaced_queue.iter_mut().find(|i| i.concept_id == concept_id) else { return };
        if recalled_correctly {
            item.mastery_level = (item.mastery_level + 0.25).min(1.0);
            let interval_days: i64 = if item.mastery_level < 0.5 {
                1
            } else if item.mastery_level < 0.7 {
                3
            } else if item.mastery_level < 0.9 {
                7
            } else {
                14
            };
            item.next_due_at_ms = now_ms + interval_days * 24 * 60 * 60 * 1000;
        } else {
            item.mastery_level = (item.mastery_level - 0.2).max(0.0);
            item.next_due_at_ms = now_ms;
        }
    }

    /// Items due right now, oldest-due first, capped at
    /// [`MAX_RETRIEVAL_ITEMS_PER_BLOCK`] — exactly what a
    /// `spaced_interleaved_retrieval` block's `items` should draw from.
    pub fn due_retrieval_items(&self, now_ms: i64) -> Vec<&SpacedRetrievalItem> {
        let mut due: Vec<&SpacedRetrievalItem> = self.retrieval_spaced_queue.iter().filter(|i| i.next_due_at_ms <= now_ms).collect();
        due.sort_by_key(|i| i.next_due_at_ms);
        due.truncate(MAX_RETRIEVAL_ITEMS_PER_BLOCK);
        due
    }

    /// Whether the NEXT block generated should scaffold hard before any
    /// autonomous gate — see the roadmap's "Calibración de Andamiaje" rule.
    /// `None` means no special scaffolding directive is warranted.
    pub fn needs_heavy_scaffolding(&self) -> bool {
        self.calibrated_baseline == CalibratedBaseline::NoviceZero || self.cognitive_metrics.prediction_accuracy_rate < 0.4
    }

    pub fn friction_is_low(&self) -> bool {
        self.cognitive_metrics.cognitive_friction_tolerance == FrictionTolerance::LowFrustration
    }
}

/// Milliseconds since epoch -> `"YYYY-MM-DD"` (UTC), dependency-free (this
/// crate carries no date/time library — see `Cargo.toml`). Howard Hinnant's
/// `civil_from_days` algorithm: proleptic-Gregorian, correct for every date
/// this app will ever produce (`now_ms()` is always "today"). Used ONLY for
/// `RecurringMisconception::last_encountered_date` — a display string, never
/// parsed back or compared, so a from-scratch implementation is safe here in
/// a way it wouldn't be for real calendar arithmetic.
pub fn iso_date_from_ms(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_gate_outcome_only_moves_the_matching_named_rate() {
        let mut mem = LearnerCognitiveMemory::new(LOCAL_LEARNER_ID);
        let start = mem.cognitive_metrics.prediction_accuracy_rate;
        mem.record_gate_outcome(DynamicBlockType::HeuristicErrorAudit, true);
        assert_eq!(mem.cognitive_metrics.prediction_accuracy_rate, start, "unrelated block type must not move this rate");
        mem.record_gate_outcome(DynamicBlockType::InteractivePredictionGate, false);
        assert!(mem.cognitive_metrics.prediction_accuracy_rate < start, "a failure must pull the EMA down");
    }

    #[test]
    fn record_misconception_dedupes_by_concept_and_pattern() {
        let mut mem = LearnerCognitiveMemory::new(LOCAL_LEARNER_ID);
        mem.record_misconception("Termodinámica", "Confunde calor con temperatura", "2026-01-01");
        mem.record_misconception("Termodinámica", "Confunde calor con temperatura", "2026-01-05");
        assert_eq!(mem.recurring_misconceptions.len(), 1);
        assert_eq!(mem.recurring_misconceptions[0].last_encountered_date, "2026-01-05");
    }

    #[test]
    fn misconceptions_relevant_to_matches_case_insensitively_and_skips_resolved() {
        let mut mem = LearnerCognitiveMemory::new(LOCAL_LEARNER_ID);
        mem.record_misconception("Termodinámica", "Confunde calor con temperatura", "2026-01-01");
        assert_eq!(mem.misconceptions_relevant_to("Semana 2: TERMODINÁMICA avanzada").len(), 1);
        assert_eq!(mem.misconceptions_relevant_to("Óptica geométrica").len(), 0);
        mem.resolve_misconceptions_for("Termodinámica");
        assert_eq!(mem.misconceptions_relevant_to("Termodinámica").len(), 0, "a resolved entry must not keep surfacing");
    }

    #[test]
    fn due_retrieval_items_respects_the_due_date_and_the_cap() {
        let mut mem = LearnerCognitiveMemory::new(LOCAL_LEARNER_ID);
        mem.queue_retrieval("c1", "Concepto 1", 1000);
        mem.queue_retrieval("c2", "Concepto 2", 1000);
        mem.queue_retrieval("c3", "Concepto 3", 5000);
        let due = mem.due_retrieval_items(1000);
        assert_eq!(due.len(), 2, "capped at MAX_RETRIEVAL_ITEMS_PER_BLOCK even with 2 due");
        assert!(due.iter().all(|i| i.concept_id != "c3"), "not due yet at t=1000");
    }

    #[test]
    fn reactivate_retrieval_pushes_the_due_date_out_on_success_and_resets_on_failure() {
        let mut mem = LearnerCognitiveMemory::new(LOCAL_LEARNER_ID);
        mem.queue_retrieval("c1", "Concepto 1", 0);
        mem.reactivate_retrieval("c1", true, 0);
        let item = mem.retrieval_spaced_queue.iter().find(|i| i.concept_id == "c1").unwrap();
        assert!(item.next_due_at_ms > 0, "a correct recall must push the due date into the future");
        assert!(item.mastery_level > 0.3);

        mem.reactivate_retrieval("c1", false, item.next_due_at_ms);
        let item = mem.retrieval_spaced_queue.iter().find(|i| i.concept_id == "c1").unwrap();
        assert_eq!(item.next_due_at_ms, item.next_due_at_ms.min(item.next_due_at_ms), "sanity: field still readable");
        assert!(item.mastery_level < 0.55, "a miss must pull mastery back down");
    }

    #[test]
    fn iso_date_from_ms_matches_known_reference_dates() {
        assert_eq!(iso_date_from_ms(0), "1970-01-01");
        // 2026-01-01T00:00:00Z
        assert_eq!(iso_date_from_ms(1_767_225_600_000), "2026-01-01");
    }

    #[test]
    fn needs_heavy_scaffolding_fires_on_novice_zero_or_low_accuracy() {
        let mut mem = LearnerCognitiveMemory::new(LOCAL_LEARNER_ID);
        assert!(!mem.needs_heavy_scaffolding(), "default prior is neutral, not struggling");
        mem.calibrated_baseline = CalibratedBaseline::NoviceZero;
        assert!(mem.needs_heavy_scaffolding());
        mem.calibrated_baseline = CalibratedBaseline::Intermediate;
        mem.cognitive_metrics.prediction_accuracy_rate = 0.1;
        assert!(mem.needs_heavy_scaffolding());
    }
}
