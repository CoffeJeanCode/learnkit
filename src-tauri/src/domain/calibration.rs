//! Level calibration: contrasts the level a learner DECLARED on the profile
//! form with what the diagnostic battery actually SHOWED, and turns the result
//! into the signal block generation consumes (`diagnosticProfile` and the
//! scaffolding baseline).
//!
//! Before this existed, the declared `entryLevel` never reached the block
//! generator and `LearnerCognitiveMemory::calibrated_baseline` was never
//! written, so the diagnostic changed the syllabus but not one generated block.
//!
//! The rule is deliberately conservative and fully deterministic — no model in
//! the loop. A battery is 3-4 multiple-choice questions: enough to catch a
//! clear mismatch, nowhere near enough to certify "advanced". So evidence can
//! move the level DOWN freely, but UP only one rung and never past
//! `Intermediate`, and nothing moves with fewer than [`MIN_ANSWERED`] answers.

use serde::{Deserialize, Serialize};

use crate::domain::learner_memory::CalibratedBaseline;
use crate::domain::learning_context::LearningContext;
use crate::domain::notebook::{DiagnosticBatteryState, DiagnosticDimension};
use crate::domain::roadmap::EntryLevel;

/// Fewest answered questions that may move the calibrated level at all.
pub const MIN_ANSWERED: usize = 2;

/// Below this share of correct answers the level steps down one rung.
const LOW_SCORE: f32 = 0.5;

/// How the battery's evidence compares with what the learner declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalibrationFit {
    /// No battery (declared `absolute_zero` skips it) or too few answers.
    NoEvidence,
    Matches,
    /// The battery showed less than the learner declared.
    BelowDeclared,
    /// The battery showed more than the learner declared.
    AboveDeclared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DimensionResult {
    pub dimension: DiagnosticDimension,
    pub correct: u32,
    pub total: u32,
}

/// Course-scoped: the same learner can be a beginner in one topic and
/// intermediate in another, so this lives on the course, not in the
/// cross-course `LearnerCognitiveMemory`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseCalibration {
    pub declared_level: EntryLevel,
    pub weekly_commitment_hours: f32,
    pub calibrated_baseline: CalibratedBaseline,
    pub fit: CalibrationFit,
    /// Share of ANSWERED questions that were correct; `None` without answers.
    pub score: Option<f32>,
    pub dimensions: Vec<DimensionResult>,
    /// What the learner has to practice with (OS, camera, software) — carried
    /// on the course so every block generation can read it. Not part of the
    /// level calibration itself; it lives here because this is the course's one
    /// learner-profile record. `None` for courses sealed before it existed.
    #[serde(default, deserialize_with = "crate::domain::learning_context::deserialize_lenient")]
    pub learner_context: Option<LearningContext>,
}

fn declared_rank(level: EntryLevel) -> i8 {
    match level {
        EntryLevel::AbsoluteZero => 0,
        EntryLevel::TheoreticalFoundations => 1,
        EntryLevel::AppliedIntermediate => 2,
    }
}

/// Highest rung evidence alone can reach: `Intermediate`. `Advanced` exists in
/// the enum but 3-4 multiple-choice answers can never establish it.
const MAX_RANK: i8 = 2;

fn baseline_from_rank(rank: i8) -> CalibratedBaseline {
    match rank.clamp(0, MAX_RANK) {
        0 => CalibratedBaseline::NoviceZero,
        1 => CalibratedBaseline::NoviceIntuitive,
        _ => CalibratedBaseline::Intermediate,
    }
}

/// Answered questions grouped by dimension, in the order each first appears.
fn dimension_results(state: &DiagnosticBatteryState) -> Vec<DimensionResult> {
    let mut out: Vec<DimensionResult> = Vec::new();
    for (i, q) in state.battery.questions.iter().enumerate() {
        let Some(chosen) = state.answers.get(&i.to_string()) else { continue };
        let correct = u32::from(chosen == &q.correct_option);
        match out.iter_mut().find(|d| d.dimension == q.dimension) {
            Some(d) => {
                d.correct += correct;
                d.total += 1;
            }
            None => out.push(DimensionResult { dimension: q.dimension, correct, total: 1 }),
        }
    }
    out
}

/// The calibration for a course. `state` is the battery with whatever has been
/// answered so far (`None` when it was skipped).
pub fn calibrate(declared: EntryLevel, weekly_commitment_hours: f32, state: Option<&DiagnosticBatteryState>) -> CourseCalibration {
    let rank = declared_rank(declared);
    let dimensions = state.map(dimension_results).unwrap_or_default();
    let answered: u32 = dimensions.iter().map(|d| d.total).sum();
    let correct: u32 = dimensions.iter().map(|d| d.correct).sum();
    let score = (answered > 0).then(|| correct as f32 / answered as f32);

    let (rank_after, fit) = match score {
        Some(s) if answered as usize >= MIN_ANSWERED => {
            let delta: i8 = if s >= 1.0 {
                1
            } else if s <= 0.0 {
                -2
            } else if s < LOW_SCORE {
                -1
            } else {
                0
            };
            let moved = (rank + delta).clamp(0, MAX_RANK);
            let fit = match moved.cmp(&rank) {
                std::cmp::Ordering::Greater => CalibrationFit::AboveDeclared,
                std::cmp::Ordering::Less => CalibrationFit::BelowDeclared,
                std::cmp::Ordering::Equal => CalibrationFit::Matches,
            };
            (moved, fit)
        }
        _ => (rank, CalibrationFit::NoEvidence),
    };

    CourseCalibration {
        declared_level: declared,
        weekly_commitment_hours,
        calibrated_baseline: baseline_from_rank(rank_after),
        fit,
        score,
        dimensions,
        learner_context: None,
    }
}

impl CourseCalibration {
    /// Dimensions answered correctly in full — what the plain `weakPoints`
    /// summary never said, and what lets the generator go faster where the
    /// learner is already solid instead of only reinforcing gaps.
    pub fn strengths(&self) -> Vec<DiagnosticDimension> {
        self.dimensions.iter().filter(|d| d.total > 0 && d.correct == d.total).map(|d| d.dimension).collect()
    }
}

/// The `diagnosticProfile` handed to the block generator: the battery's
/// `weakPoints` summary (unchanged) enriched with the calibration. `None` only
/// for a course with neither a battery nor a calibration (it predates both).
pub fn diagnostic_profile_json(state: Option<&DiagnosticBatteryState>, calibration: Option<&CourseCalibration>) -> Option<serde_json::Value> {
    let mut base = match (state, calibration) {
        (None, None) => return None,
        (Some(s), _) => s.profile_summary(),
        (None, Some(_)) => serde_json::json!({ "answered": false }),
    };
    if let (Some(c), Some(obj)) = (calibration, base.as_object_mut()) {
        obj.insert("declaredLevel".to_string(), serde_json::json!(c.declared_level));
        obj.insert("calibratedLevel".to_string(), serde_json::json!(c.calibrated_baseline));
        obj.insert("levelFit".to_string(), serde_json::json!(c.fit));
        obj.insert("weeklyCommitmentHours".to_string(), serde_json::json!(c.weekly_commitment_hours));
        if let Some(score) = c.score {
            // Rounded in f64: an f32 0.67 would serialize as 0.6700000166893005.
            obj.insert("batteryScore".to_string(), serde_json::json!((f64::from(score) * 100.0).round() / 100.0));
        }
        let strengths = c.strengths();
        if !strengths.is_empty() {
            obj.insert("strengths".to_string(), serde_json::json!(strengths));
        }
        if let Some(context) = &c.learner_context {
            obj.insert("learnerContext".to_string(), context.describe());
        }
    }
    Some(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::notebook::{DiagnosticBattery, DiagnosticQuestion};
    use std::collections::HashMap;

    fn question(dimension: DiagnosticDimension) -> DiagnosticQuestion {
        DiagnosticQuestion {
            dimension,
            prompt: "p".to_string(),
            options: vec!["A".to_string(), "B".to_string()],
            correct_option: "A".to_string(),
            diagnostic_insight: format!("insight {dimension:?}"),
        }
    }

    /// Battery of `dims` answered with `answers[i]` = right ("A") / wrong ("B");
    /// `None` leaves that question unanswered.
    fn state(dims: &[DiagnosticDimension], answers: &[Option<bool>]) -> DiagnosticBatteryState {
        let mut map = HashMap::new();
        for (i, a) in answers.iter().enumerate() {
            if let Some(right) = a {
                map.insert(i.to_string(), if *right { "A" } else { "B" }.to_string());
            }
        }
        DiagnosticBatteryState {
            battery: DiagnosticBattery { goal_alignment: "g".to_string(), questions: dims.iter().map(|d| question(*d)).collect() },
            answers: map,
        }
    }

    use DiagnosticDimension::{CriticalCase, Intuition, Mechanics};
    const DIMS: [DiagnosticDimension; 3] = [Intuition, Mechanics, CriticalCase];

    #[test]
    fn declared_absolute_zero_has_no_battery_and_calibrates_to_novice_zero() {
        let c = calibrate(EntryLevel::AbsoluteZero, 3.0, None);
        assert_eq!(c.calibrated_baseline, CalibratedBaseline::NoviceZero);
        assert_eq!(c.fit, CalibrationFit::NoEvidence);
        assert_eq!(c.score, None);
        assert!(c.dimensions.is_empty());
    }

    #[test]
    fn a_battery_that_matches_the_declaration_keeps_the_declared_level() {
        let two_of_three = state(&DIMS, &[Some(true), Some(true), Some(false)]);
        let c = calibrate(EntryLevel::TheoreticalFoundations, 5.0, Some(&two_of_three));
        assert_eq!(c.calibrated_baseline, CalibratedBaseline::NoviceIntuitive);
        assert_eq!(c.fit, CalibrationFit::Matches);
        assert_eq!(c.score.map(|s| (s * 100.0).round()), Some(67.0));
    }

    #[test]
    fn a_flawless_battery_raises_a_theoretical_learner_one_rung_and_never_past_intermediate() {
        let perfect = state(&DIMS, &[Some(true), Some(true), Some(true)]);
        let up = calibrate(EntryLevel::TheoreticalFoundations, 4.0, Some(&perfect));
        assert_eq!((up.calibrated_baseline, up.fit), (CalibratedBaseline::Intermediate, CalibrationFit::AboveDeclared));
        // Already intermediate: a perfect score cannot claim "advanced".
        let capped = calibrate(EntryLevel::AppliedIntermediate, 4.0, Some(&perfect));
        assert_eq!((capped.calibrated_baseline, capped.fit), (CalibratedBaseline::Intermediate, CalibrationFit::Matches));
    }

    #[test]
    fn a_weak_battery_lowers_the_level_and_all_wrong_lowers_it_two_rungs() {
        let one_of_three = state(&DIMS, &[Some(true), Some(false), Some(false)]);
        let down = calibrate(EntryLevel::AppliedIntermediate, 4.0, Some(&one_of_three));
        assert_eq!((down.calibrated_baseline, down.fit), (CalibratedBaseline::NoviceIntuitive, CalibrationFit::BelowDeclared));

        let none_right = state(&DIMS, &[Some(false), Some(false), Some(false)]);
        let way_down = calibrate(EntryLevel::AppliedIntermediate, 4.0, Some(&none_right));
        assert_eq!((way_down.calibrated_baseline, way_down.fit), (CalibratedBaseline::NoviceZero, CalibrationFit::BelowDeclared));
        // A theoretical learner who knows nothing is a novice from zero.
        let theoretical = calibrate(EntryLevel::TheoreticalFoundations, 4.0, Some(&none_right));
        assert_eq!(theoretical.calibrated_baseline, CalibratedBaseline::NoviceZero);
    }

    #[test]
    fn the_level_never_goes_below_novice_zero() {
        let none_right = state(&DIMS, &[Some(false), Some(false), Some(false)]);
        let c = calibrate(EntryLevel::AbsoluteZero, 2.0, Some(&none_right));
        assert_eq!(c.calibrated_baseline, CalibratedBaseline::NoviceZero);
        assert_eq!(c.fit, CalibrationFit::Matches, "already at the floor: nothing to lower");
    }

    #[test]
    fn too_few_answers_never_move_the_level() {
        let one = state(&DIMS, &[Some(false), None, None]);
        let c = calibrate(EntryLevel::AppliedIntermediate, 4.0, Some(&one));
        assert_eq!(c.calibrated_baseline, CalibratedBaseline::Intermediate);
        assert_eq!(c.fit, CalibrationFit::NoEvidence);
        assert_eq!(c.score, Some(0.0), "the evidence is still reported, it just moves nothing");
    }

    #[test]
    fn dimension_results_and_strengths_only_count_answered_questions() {
        let st = state(&[Intuition, Intuition, Mechanics, CriticalCase], &[Some(true), Some(true), Some(false), None]);
        let c = calibrate(EntryLevel::TheoreticalFoundations, 4.0, Some(&st));
        assert_eq!(
            c.dimensions,
            vec![
                DimensionResult { dimension: Intuition, correct: 2, total: 2 },
                DimensionResult { dimension: Mechanics, correct: 0, total: 1 },
            ]
        );
        assert_eq!(c.strengths(), vec![Intuition]);
    }

    #[test]
    fn the_generator_profile_adds_the_calibration_without_losing_the_weak_points() {
        let st = state(&DIMS, &[Some(true), Some(false), Some(true)]);
        let c = calibrate(EntryLevel::TheoreticalFoundations, 6.0, Some(&st));
        let json = diagnostic_profile_json(Some(&st), Some(&c)).expect("profile");
        assert_eq!(json["answered"], true);
        assert_eq!(json["weakPoints"].as_array().map(Vec::len), Some(1), "the existing weakPoints summary is intact");
        assert_eq!(json["declaredLevel"], "theoretical_foundations");
        assert_eq!(json["calibratedLevel"], "novice_intuitive");
        assert_eq!(json["levelFit"], "matches");
        assert_eq!(json["weeklyCommitmentHours"], 6.0);
        assert_eq!(json["batteryScore"], 0.67);
        assert_eq!(json["strengths"], serde_json::json!(["intuition", "critical_case"]));
    }

    #[test]
    fn an_absolute_zero_course_now_has_a_profile_even_though_it_has_no_battery() {
        let c = calibrate(EntryLevel::AbsoluteZero, 3.0, None);
        let json = diagnostic_profile_json(None, Some(&c)).expect("a profile exists now");
        assert_eq!(json["answered"], false);
        assert_eq!(json["calibratedLevel"], "novice_zero");
        assert!(json.get("strengths").is_none() && json.get("batteryScore").is_none());
    }

    #[test]
    fn the_learner_context_reaches_the_generator_profile_when_the_course_has_one() {
        use crate::domain::learning_context::{ContextAnswer, LearningContext, fallback_question_set};
        let mut c = calibrate(EntryLevel::AbsoluteZero, 3.0, None);
        assert!(diagnostic_profile_json(None, Some(&c)).expect("profile").get("learnerContext").is_none(), "no context, no key");
        c.learner_context =
            Some(
                LearningContext::validate(
                    &fallback_question_set("Fotografía"),
                    &[ContextAnswer { question_id: "prior".into(), answer: "Nada, empiezo de cero".into() }, ContextAnswer { question_id: "tools".into(), answer: "Un celular".into() }],
                )
                .expect("valid"),
            );
        let json = diagnostic_profile_json(None, Some(&c)).expect("profile");
        assert_eq!(json["learnerContext"]["answers"][1]["answer"], "Un celular");
        assert_eq!(json["learnerContext"]["answers"][0]["question"], "¿Qué tanto sabes ya sobre Fotografía?", "answers travel next to their questions");
        let back: CourseCalibration = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back, c, "the context survives storage");
    }

    #[test]
    fn a_calibration_stored_before_the_context_existed_still_loads() {
        let old = r#"{"declaredLevel":"absolute_zero","weeklyCommitmentHours":3.0,"calibratedBaseline":"novice_zero","fit":"no_evidence","score":null,"dimensions":[]}"#;
        let c: CourseCalibration = serde_json::from_str(old).expect("old rows keep loading");
        assert_eq!(c.learner_context, None);
    }

    #[test]
    fn a_course_with_neither_battery_nor_calibration_has_no_profile() {
        assert!(diagnostic_profile_json(None, None).is_none(), "old courses keep their old behaviour");
    }

    #[test]
    fn the_calibration_round_trips_through_json() {
        let st = state(&DIMS, &[Some(true), Some(true), Some(false)]);
        let c = calibrate(EntryLevel::TheoreticalFoundations, 5.0, Some(&st));
        let back: CourseCalibration = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back, c);
    }
}
