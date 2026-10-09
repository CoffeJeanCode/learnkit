//! A/B switch for the progress PRESENTATION, plus what is needed to analyse it.
//!
//! The experiment compares two versions that share EXACTLY the same teaching,
//! practice, feedback and assessment (including the transfer challenge and the
//! graded retrieval, which are what the primary metric is measured on). The
//! only difference is how progress is presented:
//!
//! - `gamified`: the capability map, the transfer-challenge framing and the
//!   evidence-backed celebrations.
//! - `plain`: none of those.
//!
//! INVARIANT: the variant is read by the frontend (what to show) and by the
//! store (to stamp evidence rows and events). It must never feed block
//! generation, grading, scheduling or any prompt — otherwise the groups would
//! differ in teaching, not only in presentation, and the comparison would
//! attribute to rewards an effect caused by something else.
//!
//! Each installation is one participant: it exports its own report, and the
//! analysis aggregates the exports across participants.

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::domain::skill_evidence::{EvidenceKind, EvidenceOutcome, SkillEvidence};
use crate::domain::skill_status::{derive_all, MIN_RETENTION_INTERVAL_MS};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StudyVariant {
    Gamified,
    Plain,
}

impl StudyVariant {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gamified => "gamified",
            Self::Plain => "plain",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "gamified" => Some(Self::Gamified),
            "plain" => Some(Self::Plain),
            _ => None,
        }
    }
}

/// Who decides the variant: the random assignment (default, made once and
/// kept) or a manual override by whoever runs the study.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariantMode {
    Random,
    Gamified,
    Plain,
}

impl VariantMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Random => "random",
            Self::Gamified => "gamified",
            Self::Plain => "plain",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "random" => Some(Self::Random),
            "gamified" => Some(Self::Gamified),
            "plain" => Some(Self::Plain),
            _ => None,
        }
    }
}

/// The variant in force. `coin` is only consulted when the mode is `random`
/// and no assignment was ever made; an existing random assignment is NEVER
/// re-flipped, so toggling the mode back and forth can't re-roll a participant.
/// Returns `(variant, newly_assigned_random)`.
pub fn resolve_variant(mode: VariantMode, stored_random: Option<StudyVariant>, coin: bool) -> (StudyVariant, Option<StudyVariant>) {
    match mode {
        VariantMode::Gamified => (StudyVariant::Gamified, None),
        VariantMode::Plain => (StudyVariant::Plain, None),
        VariantMode::Random => match stored_random {
            Some(v) => (v, None),
            None => {
                let v = if coin { StudyVariant::Gamified } else { StudyVariant::Plain };
                (v, Some(v))
            }
        },
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudySettings {
    pub participant_id: String,
    pub mode: VariantMode,
    /// The variant actually in force right now.
    pub variant: StudyVariant,
    /// The random assignment (kept even while a manual override is active).
    pub random_assignment: Option<StudyVariant>,
    pub assigned_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StudyEventKind {
    AppOpened,
    ClassOpened,
    CapabilityMapOpened,
}

impl StudyEventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AppOpened => "app_opened",
            Self::ClassOpened => "class_opened",
            Self::CapabilityMapOpened => "capability_map_opened",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "app_opened" => Some(Self::AppOpened),
            "class_opened" => Some(Self::ClassOpened),
            "capability_map_opened" => Some(Self::CapabilityMapOpened),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudyEvent {
    pub kind: StudyEventKind,
    pub class_id: Option<String>,
    /// The variant in force WHEN it happened.
    pub variant: Option<String>,
    pub at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudyMetrics {
    pub skills_with_evidence: usize,
    pub solved: usize,
    pub retained: usize,
    pub applied: usize,
    /// Skills where a transfer was passed cleanly AND a written retrieval was
    /// passed at least `MIN_RETENTION_INTERVAL_MS` AFTER that transfer.
    pub applied_and_retained: usize,
    /// PRIMARY outcome: at least one skill in `applied_and_retained` — "solved
    /// a new case without aids and kept the performance on a later assessment".
    pub primary_outcome: bool,
    pub classes_opened: usize,
    pub classes_completed: usize,
    /// Distinct UTC days with any activity; `return_days` is the days after the first.
    pub active_days: usize,
    pub return_days: usize,
}

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

pub fn compute_metrics(evidence: &[SkillEvidence], events: &[StudyEvent]) -> StudyMetrics {
    let statuses = derive_all(evidence);

    let mut by_skill: HashMap<&str, Vec<&SkillEvidence>> = HashMap::new();
    for e in evidence {
        by_skill.entry(&e.skill_id).or_default().push(e);
    }
    let applied_and_retained = by_skill
        .values()
        .filter(|rows| {
            let applied_at = rows.iter().filter(|e| crate::domain::skill_status::is_applied_row(e)).map(|e| e.created_at_ms).min();
            applied_at.is_some_and(|a| {
                rows.iter().any(|r| {
                    r.kind == EvidenceKind::Retrieval && r.outcome == EvidenceOutcome::Passed && r.created_at_ms - a >= MIN_RETENTION_INTERVAL_MS
                })
            })
        })
        .count();

    let classes_opened: BTreeSet<&str> =
        events.iter().filter(|e| e.kind == StudyEventKind::ClassOpened).filter_map(|e| e.class_id.as_deref()).collect();
    let classes_completed: BTreeSet<&str> = evidence
        .iter()
        .filter(|e| e.kind == EvidenceKind::Closure && e.outcome == EvidenceOutcome::Passed)
        .map(|e| e.skill_id.as_str())
        .collect();
    let days: BTreeSet<i64> = events.iter().map(|e| e.at_ms.div_euclid(DAY_MS)).chain(evidence.iter().map(|e| e.created_at_ms.div_euclid(DAY_MS))).collect();

    StudyMetrics {
        skills_with_evidence: statuses.len(),
        solved: statuses.iter().filter(|s| s.solved.achieved).count(),
        retained: statuses.iter().filter(|s| s.retained.achieved).count(),
        applied: statuses.iter().filter(|s| s.applied.achieved).count(),
        applied_and_retained,
        primary_outcome: applied_and_retained > 0,
        classes_opened: classes_opened.len(),
        classes_completed: classes_completed.len(),
        active_days: days.len(),
        return_days: days.len().saturating_sub(1),
    }
}

/// What one installation exports for the analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudyReport {
    pub schema_version: u32,
    pub generated_at_ms: i64,
    pub settings: StudySettings,
    /// Variants that stamped any row/event. More than one means the version
    /// changed mid-study: the analysis should treat that participant apart.
    pub variants_seen: Vec<String>,
    pub metrics: StudyMetrics,
    /// Not measured: this build records no session durations.
    pub time_of_use_measured: bool,
    pub evidence: Vec<SkillEvidence>,
    pub events: Vec<StudyEvent>,
}

pub fn build_report(settings: StudySettings, evidence: Vec<SkillEvidence>, events: Vec<StudyEvent>, now_ms: i64) -> StudyReport {
    let variants_seen: BTreeSet<String> =
        evidence.iter().filter_map(|e| e.variant.clone()).chain(events.iter().filter_map(|e| e.variant.clone())).collect();
    StudyReport {
        schema_version: 1,
        generated_at_ms: now_ms,
        metrics: compute_metrics(&evidence, &events),
        settings,
        variants_seen: variants_seen.into_iter().collect(),
        time_of_use_measured: false,
        evidence,
        events,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_random_assignment_is_made_once_and_never_rerolled() {
        let (v, fresh) = resolve_variant(VariantMode::Random, None, true);
        assert_eq!((v, fresh), (StudyVariant::Gamified, Some(StudyVariant::Gamified)));
        let (v, fresh) = resolve_variant(VariantMode::Random, None, false);
        assert_eq!((v, fresh), (StudyVariant::Plain, Some(StudyVariant::Plain)));
        // Once stored, the coin is ignored.
        assert_eq!(resolve_variant(VariantMode::Random, Some(StudyVariant::Plain), true), (StudyVariant::Plain, None));
    }

    #[test]
    fn a_manual_override_wins_without_touching_the_stored_random_assignment() {
        assert_eq!(resolve_variant(VariantMode::Gamified, Some(StudyVariant::Plain), false), (StudyVariant::Gamified, None));
        assert_eq!(resolve_variant(VariantMode::Plain, None, true), (StudyVariant::Plain, None));
    }

    #[test]
    fn variants_modes_and_events_round_trip_through_their_storage_strings() {
        for v in [StudyVariant::Gamified, StudyVariant::Plain] {
            assert_eq!(StudyVariant::parse(v.as_str()), Some(v));
        }
        for m in [VariantMode::Random, VariantMode::Gamified, VariantMode::Plain] {
            assert_eq!(VariantMode::parse(m.as_str()), Some(m));
        }
        for k in [StudyEventKind::AppOpened, StudyEventKind::ClassOpened, StudyEventKind::CapabilityMapOpened] {
            assert_eq!(StudyEventKind::parse(k.as_str()), Some(k));
        }
        assert_eq!(StudyVariant::parse("x"), None);
    }

    fn row(skill: &str, kind: EvidenceKind, outcome: EvidenceOutcome, at: i64) -> SkillEvidence {
        let mut e = SkillEvidence::new(skill, "c", kind, outcome);
        e.created_at_ms = at;
        e
    }

    fn clean_transfer(skill: &str, at: i64) -> SkillEvidence {
        let mut e = row(skill, EvidenceKind::PracticeGate, EvidenceOutcome::Passed, at).with_transfer(true);
        e.support_level = Some("independent".to_string());
        e
    }

    #[test]
    fn the_primary_outcome_needs_a_clean_transfer_and_a_later_written_retrieval() {
        let t = 1_000;
        let retrieval = |at| row("s", EvidenceKind::Retrieval, EvidenceOutcome::Passed, at);

        let only_transfer = [clean_transfer("s", t)];
        assert!(!compute_metrics(&only_transfer, &[]).primary_outcome);

        let too_soon = [clean_transfer("s", t), retrieval(t + MIN_RETENTION_INTERVAL_MS - 1)];
        assert!(!compute_metrics(&too_soon, &[]).primary_outcome, "retrieval must come a full interval after the transfer");

        let ok = [clean_transfer("s", t), retrieval(t + MIN_RETENTION_INTERVAL_MS)];
        let m = compute_metrics(&ok, &[]);
        assert!(m.primary_outcome);
        assert_eq!((m.applied, m.applied_and_retained), (1, 1));

        // A retrieval BEFORE the transfer does not "maintain" anything.
        let before = [retrieval(t), clean_transfer("s", t + 3 * MIN_RETENTION_INTERVAL_MS)];
        assert!(!compute_metrics(&before, &[]).primary_outcome);

        // A self-reported recall is never retention.
        let selfie = [clean_transfer("s", t), row("s", EvidenceKind::Retrieval, EvidenceOutcome::SelfRecalled, t + 2 * MIN_RETENTION_INTERVAL_MS)];
        assert!(!compute_metrics(&selfie, &[]).primary_outcome);
    }

    #[test]
    fn engagement_metrics_count_distinct_classes_and_active_days() {
        let ev = |kind, class: Option<&str>, at| StudyEvent { kind, class_id: class.map(str::to_string), variant: None, at_ms: at };
        let events = [
            ev(StudyEventKind::AppOpened, None, 10),
            ev(StudyEventKind::ClassOpened, Some("a"), 20),
            ev(StudyEventKind::ClassOpened, Some("a"), 30), // reopened: still one class
            ev(StudyEventKind::ClassOpened, Some("b"), DAY_MS + 5),
            ev(StudyEventKind::AppOpened, None, 3 * DAY_MS),
        ];
        let evidence = [row("a", EvidenceKind::Closure, EvidenceOutcome::Passed, 40), row("b", EvidenceKind::Closure, EvidenceOutcome::Failed, 50)];
        let m = compute_metrics(&evidence, &events);
        assert_eq!((m.classes_opened, m.classes_completed), (2, 1));
        assert_eq!((m.active_days, m.return_days), (3, 2));
        assert_eq!(compute_metrics(&[], &[]).active_days, 0);
    }

    #[test]
    fn the_report_flags_a_mid_study_version_change() {
        let settings = StudySettings {
            participant_id: "p".into(),
            mode: VariantMode::Random,
            variant: StudyVariant::Plain,
            random_assignment: Some(StudyVariant::Plain),
            assigned_at_ms: Some(1),
        };
        let mut a = row("s", EvidenceKind::PracticeGate, EvidenceOutcome::Passed, 1);
        a.variant = Some("plain".into());
        let mut b = row("s", EvidenceKind::PracticeGate, EvidenceOutcome::Passed, 2);
        b.variant = Some("gamified".into());
        let r = build_report(settings, vec![a, b], vec![], 99);
        assert_eq!(r.variants_seen, vec!["gamified".to_string(), "plain".to_string()]);
        assert!(!r.time_of_use_measured);
        assert_eq!(r.schema_version, 1);
    }
}
