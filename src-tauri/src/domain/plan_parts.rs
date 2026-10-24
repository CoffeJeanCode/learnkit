//! The syllabus proposal written IN PARTS instead of one giant tool call.
//!
//! One turn that writes every week and every session (8 weeks at 12 h/week is
//! 24 sessions) takes minutes and shows the learner nothing until the end. So it
//! is split in two kinds of small calls:
//!
//! 1. the OUTLINE — course title, diagnostic summary, one line per week, the
//!    capstone and the closing question (small, fast: it is what the learner
//!    sees first);
//! 2. the SESSIONS of ONE week each, run in parallel, shown as each arrives.
//!
//! The model never writes what code can compute: how many sessions a week has
//! and how many hours each one lasts follow from the weekly hours
//! (see [`session_hours`]), so the arithmetic that used to fail grounding most
//! often is simply right. The parts are assembled into the SAME
//! [`ProposeSyllabusPlanArgs`](crate::domain::roadmap::ProposeSyllabusPlanArgs)
//! the single-call flow produces and go through the same grounding.

use serde::{Deserialize, Serialize};

use crate::domain::roadmap::{CapstoneProject, Deliverable, Micromodule, Milestone};

/// One line of the outline: what a week is about, before its sessions exist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineWeek {
    pub week: u16,
    pub title: String,
    pub deliverable: String,
    #[serde(default)]
    pub weekly_goal: Option<String>,
}

/// `submit_plan_outline`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanOutlineArgs {
    pub course_title: String,
    pub core_focus: String,
    pub identified_needs: Vec<String>,
    pub learning_strategy: String,
    pub weeks: Vec<OutlineWeek>,
    pub capstone_project: CapstoneProject,
    pub closing_question: String,
}

/// One session as the model writes it. No `hours`: code decides them.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDraft {
    pub label: String,
    #[serde(default)]
    pub focus: Option<String>,
    pub deliverable: Deliverable,
    #[serde(default)]
    pub objective: Option<String>,
    pub interactive_blocks: Vec<String>,
}

/// `submit_week_sessions`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekSessionsArgs {
    pub sessions: Vec<SessionDraft>,
}

/// How long each session of a week lasts, for `sessions` sessions and
/// `pace_hours_per_week` hours: equal shares rounded to 2 decimals, the last one
/// absorbing the rounding so the total is exactly the weekly hours. Equal by
/// construction, so the "homogeneous" and "at most 4 h" rules cannot be broken
/// by the model — the caller picks `sessions` large enough for the 4 h cap.
pub fn session_hours(pace_hours_per_week: f32, sessions: usize) -> Vec<f32> {
    if sessions == 0 {
        return vec![];
    }
    let share = ((pace_hours_per_week as f64 / sessions as f64) * 100.0).floor() / 100.0;
    let mut hours = vec![share as f32; sessions];
    let last = pace_hours_per_week as f64 - share * (sessions as f64 - 1.0);
    hours[sessions - 1] = ((last * 100.0).round() / 100.0) as f32;
    hours
}

/// A week of the final syllabus: the outline line plus its sessions with the
/// hours code assigned. `sessions.len()` must equal `hours.len()` (checked by the
/// caller before assembling).
pub fn assemble_week(outline: &OutlineWeek, sessions: Vec<SessionDraft>, hours: &[f32]) -> Milestone {
    Milestone {
        week: outline.week,
        title: outline.title.clone(),
        deliverable: outline.deliverable.clone(),
        weekly_goal: outline.weekly_goal.clone(),
        micromodules: sessions
            .into_iter()
            .zip(hours.iter().copied())
            .map(|(s, hours)| Micromodule {
                label: s.label,
                hours,
                deliverable: s.deliverable,
                focus: s.focus,
                objective: s.objective,
                interactive_blocks: s.interactive_blocks,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hours_split_equally_and_always_add_up_exactly() {
        assert_eq!(session_hours(12.0, 3), vec![4.0, 4.0, 4.0]);
        assert_eq!(session_hours(5.0, 2), vec![2.5, 2.5]);
        // 10 h over 3 sessions does not divide: the last one absorbs the rounding.
        let h = session_hours(10.0, 3);
        assert_eq!(h, vec![3.33, 3.33, 3.34]);
        for (pace, n) in [(0.5f32, 2usize), (7.0, 2), (14.0, 4), (80.0, 20), (3.7, 2)] {
            let hours = session_hours(pace, n);
            assert_eq!(hours.len(), n);
            let total: f32 = hours.iter().sum();
            assert!((total - pace).abs() < 0.011, "{pace} over {n} summed to {total}");
        }
        assert!(session_hours(5.0, 0).is_empty());
    }

    #[test]
    fn a_week_is_the_outline_line_plus_its_sessions_with_code_assigned_hours() {
        let outline = OutlineWeek { week: 3, title: "Permisos".into(), deliverable: "Un script".into(), weekly_goal: Some("Cambiar permisos".into()) };
        let draft = |label: &str| SessionDraft {
            label: label.into(),
            focus: Some("foco".into()),
            deliverable: Deliverable { artifact_type: crate::domain::roadmap::DeliverableArtifactType::FunctionalCli, description: "un script".into() },
            objective: Some("Cambiar permisos".into()),
            interactive_blocks: vec!["hands_on_mission".into(), "socratic_prediction".into(), "metacognitive_closure".into()],
        };
        let m = assemble_week(&outline, vec![draft("Días 1-2"), draft("Días 3-4")], &[2.5, 2.5]);
        assert_eq!((m.week, m.title.as_str()), (3, "Permisos"));
        assert_eq!(m.micromodules.iter().map(|s| s.hours).collect::<Vec<_>>(), vec![2.5, 2.5]);
        assert_eq!(m.micromodules[1].label, "Días 3-4");
    }

    #[test]
    fn the_wire_format_the_model_produces_deserializes_without_hours() {
        let week: WeekSessionsArgs = serde_json::from_value(serde_json::json!({
            "sessions": [{
                "label": "Días 1-2 — Abrir la terminal",
                "focus": "Qué es un comando",
                "deliverable": { "artifactType": "functional_cli", "description": "un script que lista archivos" },
                "objective": "Abrir una terminal y listar archivos",
                "interactiveBlocks": ["hands_on_mission", "socratic_prediction", "metacognitive_closure"]
            }]
        }))
        .expect("a week without hours");
        assert_eq!(week.sessions.len(), 1);

        let outline: PlanOutlineArgs = serde_json::from_value(serde_json::json!({
            "courseTitle": "Linux desde cero", "coreFocus": "Terminal", "identifiedNeeds": ["práctica"], "learningStrategy": "guiado",
            "weeks": [{ "week": 1, "title": "Primeros pasos", "deliverable": "Navegar carpetas", "weeklyGoal": "Moverse sin ratón" }],
            "capstoneProject": { "title": "Mi servidor", "description": "Montar un servidor", "verifiableEvidence": "Un servidor corriendo" },
            "closingQuestion": "¿Te parece bien?"
        }))
        .expect("outline");
        assert_eq!(outline.capstone_project.title, "Mi servidor");
    }
}
