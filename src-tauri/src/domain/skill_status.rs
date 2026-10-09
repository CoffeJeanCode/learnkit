//! What the append-only evidence history (`domain::skill_evidence`) says a
//! student can actually DO with a skill. Pure functions over the rows: the
//! rules live here (and only here), so changing a rule re-derives every
//! student's status from the same history, with no data migration.
//!
//! Three independent achievements — passing an activity does not imply
//! remembering it, and remembering it does not imply applying it:
//!
//! - **solved** ("Lo resolví"): passed a practice gate (mission or audit).
//!   Records whether aids were used (more than one attempt, or hints seen).
//! - **retained** ("Lo recuerdo"): a WRITTEN retrieval answer graded correct,
//!   given at least [`MIN_RETENTION_INTERVAL_MS`] after the skill was first
//!   demonstrated. The solution is withheld from the client until the
//!   student answers (`grading::redact_block`), and self-reports never count.
//! - **applied** ("Lo aplico"): passed a transfer challenge — a new case, no
//!   hints (`support_level == independent`), first attempt, no feedback seen.

use serde::{Deserialize, Serialize};

use crate::domain::skill_evidence::{EvidenceKind, EvidenceOutcome, SkillEvidence};

/// A retrieval answered sooner than this after the first demonstration is
/// still useful practice, but is not evidence of retention. 24 h matches the
/// first interval of the spaced-retrieval schedule.
pub const MIN_RETENTION_INTERVAL_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Achievement {
    pub achieved: bool,
    /// When the FIRST qualifying row happened.
    pub achieved_at_ms: Option<i64>,
    /// Rows that justify it — every achievement points at its evidence.
    pub evidence_ids: Vec<String>,
}

impl Achievement {
    fn none() -> Self {
        Self { achieved: false, achieved_at_ms: None, evidence_ids: Vec::new() }
    }

    fn from_rows(rows: &[&SkillEvidence]) -> Self {
        match rows.first() {
            Some(first) => Self {
                achieved: true,
                achieved_at_ms: Some(first.created_at_ms),
                evidence_ids: rows.iter().map(|r| r.id.clone()).collect(),
            },
            None => Self::none(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillStatus {
    pub skill_id: String,
    pub solved: Achievement,
    /// `Some(true)` when the first solving attempt needed a retry or hints.
    pub solved_with_aids: Option<bool>,
    pub retained: Achievement,
    /// Whole days between the first demonstration and the first qualifying
    /// retrieval — what "recordaste el concepto después de N días" says.
    pub retained_after_days: Option<i64>,
    pub applied: Achievement,
    /// Evidence that moved nothing yet: attempts and retrievals that did not
    /// qualify. Lets a UI say "intentado" without claiming mastery.
    pub attempts_recorded: usize,
}

pub fn is_solved_row(e: &SkillEvidence) -> bool {
    e.kind == EvidenceKind::PracticeGate && e.outcome == EvidenceOutcome::Passed
}

pub fn is_applied_row(e: &SkillEvidence) -> bool {
    is_solved_row(e)
        && e.is_transfer
        && e.support_level.as_deref() == Some("independent")
        && e.attempt_number == 1
        && e.hints_shown == 0
}

/// First moment the student PASSED anything for this skill — the anchor the
/// retention interval is counted from.
fn first_demonstration_ms(rows: &[SkillEvidence]) -> Option<i64> {
    rows.iter()
        .filter(|e| e.outcome == EvidenceOutcome::Passed && e.kind != EvidenceKind::Retrieval)
        .map(|e| e.created_at_ms)
        .min()
}

/// Derives one skill's status from its evidence (any order).
pub fn derive_status(skill_id: &str, evidence: &[SkillEvidence]) -> SkillStatus {
    let mut rows: Vec<SkillEvidence> = evidence.iter().filter(|e| e.skill_id == skill_id).cloned().collect();
    rows.sort_by_key(|e| e.created_at_ms);

    let solved_rows: Vec<&SkillEvidence> = rows.iter().filter(|e| is_solved_row(e)).collect();
    let applied_rows: Vec<&SkillEvidence> = rows.iter().filter(|e| is_applied_row(e)).collect();
    let anchor = first_demonstration_ms(&rows);
    let retained_rows: Vec<&SkillEvidence> = rows
        .iter()
        .filter(|e| e.kind == EvidenceKind::Retrieval && e.outcome == EvidenceOutcome::Passed)
        .filter(|e| anchor.is_some_and(|a| e.created_at_ms - a >= MIN_RETENTION_INTERVAL_MS))
        .collect();

    SkillStatus {
        skill_id: skill_id.to_string(),
        solved_with_aids: solved_rows.first().map(|r| r.attempt_number > 1 || r.hints_shown > 0),
        solved: Achievement::from_rows(&solved_rows),
        retained_after_days: retained_rows.first().zip(anchor).map(|(r, a)| (r.created_at_ms - a) / MIN_RETENTION_INTERVAL_MS),
        retained: Achievement::from_rows(&retained_rows),
        applied: Achievement::from_rows(&applied_rows),
        attempts_recorded: rows.len(),
    }
}

/// Status of every skill that has at least one evidence row, by first
/// appearance.
pub fn derive_all(evidence: &[SkillEvidence]) -> Vec<SkillStatus> {
    let mut ids: Vec<&str> = Vec::new();
    for e in evidence {
        if !ids.contains(&e.skill_id.as_str()) {
            ids.push(&e.skill_id);
        }
    }
    ids.into_iter().map(|id| derive_status(id, evidence)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = MIN_RETENTION_INTERVAL_MS;

    fn row(kind: EvidenceKind, outcome: EvidenceOutcome, at: i64) -> SkillEvidence {
        let mut e = SkillEvidence::new("s", "c", kind, outcome);
        e.created_at_ms = at;
        e
    }

    fn transfer(at: i64, attempt: u32, hints: u32, level: &str) -> SkillEvidence {
        let mut e = row(EvidenceKind::PracticeGate, EvidenceOutcome::Passed, at).with_attempt(attempt, hints).with_transfer(true);
        e.support_level = Some(level.to_string());
        e
    }

    #[test]
    fn no_evidence_means_nothing_is_achieved() {
        let s = derive_status("s", &[]);
        assert!(!s.solved.achieved && !s.retained.achieved && !s.applied.achieved);
        assert_eq!((s.solved_with_aids, s.attempts_recorded), (None, 0));
    }

    #[test]
    fn solving_requires_a_passed_practice_gate_and_records_whether_aids_were_used() {
        let conceptual_only = [row(EvidenceKind::ConceptualGate, EvidenceOutcome::Passed, 0)];
        assert!(!derive_status("s", &conceptual_only).solved.achieved);

        let escalated = [row(EvidenceKind::PracticeGate, EvidenceOutcome::Escalated, 0)];
        assert!(!derive_status("s", &escalated).solved.achieved, "continuing is not demonstrating");

        let clean = [row(EvidenceKind::PracticeGate, EvidenceOutcome::Passed, 5)];
        let s = derive_status("s", &clean);
        assert!(s.solved.achieved);
        assert_eq!(s.solved_with_aids, Some(false));

        let aided = [
            row(EvidenceKind::PracticeGate, EvidenceOutcome::Failed, 1).with_attempt(1, 0),
            row(EvidenceKind::PracticeGate, EvidenceOutcome::Passed, 2).with_attempt(2, 1),
        ];
        let s = derive_status("s", &aided);
        assert_eq!((s.solved.achieved, s.solved_with_aids), (true, Some(true)));
    }

    #[test]
    fn retention_needs_a_graded_correct_answer_at_least_a_day_after_the_first_demonstration() {
        let passed = row(EvidenceKind::PracticeGate, EvidenceOutcome::Passed, 1000);
        let too_soon = row(EvidenceKind::Retrieval, EvidenceOutcome::Passed, 1000 + DAY - 1);
        assert!(!derive_status("s", &[passed.clone(), too_soon]).retained.achieved);

        let on_time = row(EvidenceKind::Retrieval, EvidenceOutcome::Passed, 1000 + DAY);
        let s = derive_status("s", &[passed.clone(), on_time.clone()]);
        assert!(s.retained.achieved);
        assert_eq!(s.retained.evidence_ids, vec![on_time.id.clone()]);
        assert_eq!(s.retained_after_days, Some(1));

        let wrong = row(EvidenceKind::Retrieval, EvidenceOutcome::Failed, 1000 + 2 * DAY);
        assert!(!derive_status("s", &[passed.clone(), wrong]).retained.achieved);

        let self_report = row(EvidenceKind::Retrieval, EvidenceOutcome::SelfRecalled, 1000 + 2 * DAY);
        assert!(!derive_status("s", &[passed, self_report]).retained.achieved, "self-reports never count");
    }

    #[test]
    fn retention_without_any_prior_demonstration_does_not_count() {
        let lonely = row(EvidenceKind::Retrieval, EvidenceOutcome::Passed, 10 * DAY);
        assert!(!derive_status("s", &[lonely]).retained.achieved);
    }

    #[test]
    fn applying_requires_a_clean_first_attempt_transfer_at_independent_support() {
        assert!(derive_status("s", &[transfer(1, 1, 0, "independent")]).applied.achieved);
        assert!(!derive_status("s", &[transfer(1, 2, 1, "independent")]).applied.achieved, "needed a retry/hints");
        assert!(!derive_status("s", &[transfer(1, 1, 0, "faded")]).applied.achieved, "not at independent support");
        let not_transfer = row(EvidenceKind::PracticeGate, EvidenceOutcome::Passed, 1).with_attempt(1, 0);
        assert!(!derive_status("s", &[not_transfer]).applied.achieved, "an ordinary mission is not transfer");
    }

    #[test]
    fn the_three_achievements_are_independent_and_other_skills_are_ignored() {
        let mut other = row(EvidenceKind::PracticeGate, EvidenceOutcome::Passed, 1);
        other.skill_id = "other".to_string();
        let s = derive_status("s", &[other.clone()]);
        assert_eq!(s.attempts_recorded, 0);

        let solved_only = [row(EvidenceKind::PracticeGate, EvidenceOutcome::Passed, 1)];
        let s = derive_status("s", &solved_only);
        assert!(s.solved.achieved && !s.retained.achieved && !s.applied.achieved);

        let all = derive_all(&[solved_only[0].clone(), other]);
        assert_eq!(all.iter().map(|s| s.skill_id.as_str()).collect::<Vec<_>>(), vec!["s", "other"]);
    }
}
