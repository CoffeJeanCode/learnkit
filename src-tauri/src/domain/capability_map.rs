//! The capability map: what the student can demonstrably DO, one entry per
//! skill (class), each backed by the evidence rows that justify it.
//!
//! Nothing here is a score. An entry reports three independent achievements
//! (`domain::skill_status`) and carries the evidence they were derived from,
//! so every reward the UI shows can link to a concrete attempt. "The class
//! was completed" (the student could continue) is reported separately from
//! "the skill was demonstrated".

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::domain::learner_memory::LearnerCognitiveMemory;
use crate::domain::notebook::ClassRecord;
use crate::domain::skill_evidence::SkillEvidence;
use crate::domain::skill_status::{derive_status, SkillStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityEntry {
    pub skill_id: String,
    pub title: String,
    /// "Qué sabrás hacer" — the capability statement, when the class has one.
    pub objective: Option<String>,
    pub week_number: u16,
    pub milestone_title: String,
    /// The notebook was finished (could continue) — NOT a demonstration.
    pub class_complete: bool,
    pub status: SkillStatus,
    /// When the next spaced retrieval of this skill becomes available.
    pub next_retrieval_at_ms: Option<i64>,
    /// Every row behind the status, oldest first.
    pub evidence: Vec<SkillEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityTotals {
    pub skills: usize,
    pub solved: usize,
    pub retained: usize,
    pub applied: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityMap {
    pub course_id: String,
    pub entries: Vec<CapabilityEntry>,
    pub totals: CapabilityTotals,
}

/// `milestones`: milestone id -> (week number, title). Entries keep the
/// syllabus order of `classes`.
pub fn build(
    course_id: &str,
    classes: &[ClassRecord],
    milestones: &HashMap<String, (u16, String)>,
    evidence: &[SkillEvidence],
    memory: &LearnerCognitiveMemory,
) -> CapabilityMap {
    let entries: Vec<CapabilityEntry> = classes
        .iter()
        .map(|c| {
            let (week_number, milestone_title) = milestones.get(&c.milestone_id).cloned().unwrap_or((0, String::new()));
            let mut rows: Vec<SkillEvidence> = evidence.iter().filter(|e| e.skill_id == c.id).cloned().collect();
            rows.sort_by_key(|e| e.created_at_ms);
            CapabilityEntry {
                skill_id: c.id.clone(),
                title: c.title.clone(),
                objective: c.objective.clone(),
                week_number,
                milestone_title,
                class_complete: c.complete,
                status: derive_status(&c.id, &rows),
                next_retrieval_at_ms: memory.retrieval_spaced_queue.iter().find(|i| i.concept_id == c.id).map(|i| i.next_due_at_ms),
                evidence: rows,
            }
        })
        .collect();
    let totals = CapabilityTotals {
        skills: entries.len(),
        solved: entries.iter().filter(|e| e.status.solved.achieved).count(),
        retained: entries.iter().filter(|e| e.status.retained.achieved).count(),
        applied: entries.iter().filter(|e| e.status.applied.achieved).count(),
    };
    CapabilityMap { course_id: course_id.to_string(), entries, totals }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::skill_evidence::{EvidenceKind, EvidenceOutcome};
    use crate::domain::skill_status::MIN_RETENTION_INTERVAL_MS;

    fn class(id: &str, order: u32, complete: bool) -> ClassRecord {
        ClassRecord {
            id: id.to_string(),
            milestone_id: "m1".to_string(),
            class_number: order as u16,
            title: format!("Clase {id}"),
            order_index: order,
            hours: 1.0,
            objective: Some(format!("Hacer {id}")),
            complete,
        }
    }

    fn row(skill: &str, kind: EvidenceKind, outcome: EvidenceOutcome, at: i64) -> SkillEvidence {
        let mut e = SkillEvidence::new(skill, "course", kind, outcome);
        e.created_at_ms = at;
        e
    }

    #[test]
    fn every_entry_links_its_status_to_the_rows_behind_it_and_keeps_syllabus_order() {
        let classes = [class("a", 1, true), class("b", 2, false), class("c", 3, false)];
        let milestones: HashMap<String, (u16, String)> = [("m1".to_string(), (1u16, "Semana 1".to_string()))].into_iter().collect();
        let solved = row("a", EvidenceKind::PracticeGate, EvidenceOutcome::Passed, 1000);
        let recalled = row("a", EvidenceKind::Retrieval, EvidenceOutcome::Passed, 1000 + MIN_RETENTION_INTERVAL_MS * 3);
        let attempt = row("b", EvidenceKind::ConceptualGate, EvidenceOutcome::Failed, 5);
        let evidence = [recalled.clone(), solved.clone(), attempt.clone()];

        let mut memory = LearnerCognitiveMemory::new("local");
        memory.queue_retrieval("a", "Clase a", 42);

        let map = build("course", &classes, &milestones, &evidence, &memory);

        assert_eq!(map.entries.iter().map(|e| e.skill_id.as_str()).collect::<Vec<_>>(), vec!["a", "b", "c"]);
        let a = &map.entries[0];
        assert!(a.status.solved.achieved && a.status.retained.achieved && !a.status.applied.achieved);
        assert_eq!(a.status.retained_after_days, Some(3));
        assert_eq!(a.evidence.iter().map(|e| e.id.clone()).collect::<Vec<_>>(), vec![solved.id.clone(), recalled.id.clone()], "oldest first");
        // Every achievement id points at a row that is actually in the entry.
        for id in a.status.solved.evidence_ids.iter().chain(&a.status.retained.evidence_ids) {
            assert!(a.evidence.iter().any(|e| &e.id == id));
        }
        assert_eq!((a.week_number, a.milestone_title.as_str()), (1, "Semana 1"));
        assert_eq!(a.next_retrieval_at_ms, Some(42));
        assert!(a.class_complete);

        let b = &map.entries[1];
        assert!(!b.status.solved.achieved, "an attempt is not a demonstration");
        assert_eq!(b.evidence.len(), 1);
        assert!(map.entries[2].evidence.is_empty());
        assert_eq!(map.totals, CapabilityTotals { skills: 3, solved: 1, retained: 1, applied: 0 });
    }

    #[test]
    fn finishing_a_class_does_not_count_as_demonstrating_the_skill() {
        let classes = [class("a", 1, true)];
        let map = build("c", &classes, &HashMap::new(), &[], &LearnerCognitiveMemory::new("local"));
        assert!(map.entries[0].class_complete);
        assert!(!map.entries[0].status.solved.achieved);
        assert_eq!(map.totals.solved, 0);
    }
}
