//! Hybrid generation: for the SIMPLE block kinds the model picks a template and
//! fills only its parameters; Rust expands it into the full
//! [`GeneratedSectionBlock`] the rest of the pipeline (grounding, critic,
//! persistence, rendering) already understands.
//!
//! Why: the free-form schema is one huge `oneOf` the model has to hold in its
//! head, and a quiz-like block only carries a handful of facts. Templates cut
//! output tokens (fewer `finish_reason=Length` retries) and remove whole error
//! classes BY CONSTRUCTION — the answer key is derived, never typed twice
//! (`correctOption` can't miss `options`, exactly one branch is optimal,
//! `correctOrder` always matches `shuffledSteps`), and the position of the
//! right answer is shuffled here instead of being left to the model's bias
//! (the "correct option is always B / longest" failure).
//!
//! Template tags carry the `tpl_` prefix so they can never collide with a
//! free-form `blockType`. Expansion targets existing block types only, so no
//! frontend change is needed.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::notebook::{BoundaryCase, GeneratedSectionBlock, InteractionMode, RetrievalPrompt, ScenarioBranch};

pub const TEMPLATE_PREFIX: &str = "tpl_";

/// Word ceilings for `anchored_micro_theory`. Shared with the post-generation
/// review (`notebook_service::grounding`) so the template rejects, inside the
/// same tool call, exactly what that review would reject a whole generation later.
pub const THEORY_MAX_SYSTEM_RULE_WORDS: usize = 240;
/// Combined ceiling for `intuitiveHook` + `analogyBoundary` + `frequentError`.
pub const THEORY_MAX_OTHER_LAYERS_WORDS: usize = 100;

/// One answer option with its feedback — shown to the student right after they
/// pick it (the feedback for the correct option confirms WHY it is right).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionSpec {
    pub text: String,
    pub feedback: String,
}

/// One branch of a scenario decision (consequence visible after choosing).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchSpec {
    pub choice: String,
    pub consequence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "blockType")]
pub enum BlockTemplate {
    /// → `interactive_prediction_gate` (`multiple_choice`).
    #[serde(rename = "tpl_multiple_choice", rename_all = "camelCase")]
    MultipleChoice { question: String, correct: OptionSpec, distractors: Vec<OptionSpec> },
    /// → `interactive_prediction_gate` (`multiple_choice`, Verdadero/Falso).
    #[serde(rename = "tpl_true_false", rename_all = "camelCase")]
    TrueFalse { statement: String, is_true: bool, explanation: String },
    /// → `interactive_prediction_gate` (`causal_chain_order`).
    #[serde(rename = "tpl_ordering", rename_all = "camelCase")]
    Ordering { question: String, steps_in_order: Vec<String> },
    /// → `branching_scenario_challenge`.
    #[serde(rename = "tpl_scenario_choice", rename_all = "camelCase")]
    ScenarioChoice { scenario: String, decision_point: String, optimal: BranchSpec, alternatives: Vec<BranchSpec> },
    /// → `spaced_interleaved_retrieval`.
    #[serde(rename = "tpl_quick_recall", rename_all = "camelCase")]
    QuickRecall { items: Vec<RetrievalPrompt> },
    /// → `anchored_micro_theory`. Flat params with the word caps checked at
    /// call time, so an over-long layer is corrected in the same turn.
    #[serde(rename = "tpl_theory", rename_all = "camelCase")]
    Theory { title: String, intuitive_hook: String, analogy_boundary: String, system_rule: String, frequent_error: String },
    /// → `interactive_prediction_gate` (`boundary_case_audit`).
    #[serde(rename = "tpl_boundary_cases", rename_all = "camelCase")]
    BoundaryCases { question: String, cases: Vec<BoundaryCase> },
}

fn blank(s: &str) -> bool {
    s.trim().is_empty()
}

fn norm(s: &str) -> String {
    s.trim().to_lowercase()
}

fn all_unique<'a>(items: impl IntoIterator<Item = &'a str>) -> bool {
    let mut seen = HashSet::new();
    items.into_iter().all(|s| seen.insert(norm(s)))
}

impl BlockTemplate {
    /// Whether `block_type` names a template (vs a free-form block).
    pub fn is_template_tag(block_type: &str) -> bool {
        block_type.starts_with(TEMPLATE_PREFIX)
    }

    /// Whether expansion fully determines this template's grading shape: the
    /// answer key, the option set and the ordering are fixed by construction
    /// (see [`expand`](Self::expand)), so no LLM judgment is left for the
    /// pedagogical critic to audit — it would only cost an extra round-trip
    /// on top of the deterministic guardrails that always run. `false` for
    /// the content-heavy templates (`tpl_multiple_choice`,
    /// `tpl_scenario_choice`) whose distractors / branches still warrant a
    /// semantic read, and for `tpl_quick_recall` (already critic-free by
    /// block type).
    pub fn is_mechanical(&self) -> bool {
        matches!(self, Self::TrueFalse { .. } | Self::Ordering { .. })
    }

    /// Shape checks the type system can't express. A failure here is reported
    /// to the model like any malformed tool call, naming the exact problem.
    pub fn validate(&self) -> Result<(), String> {
        let err = |tpl: &str, msg: &str| Err(format!("{tpl}: {msg}"));
        match self {
            Self::MultipleChoice { question, correct, distractors } => {
                let t = "tpl_multiple_choice";
                if blank(question) || blank(&correct.text) || blank(&correct.feedback) {
                    return err(t, "question, correct.text y correct.feedback no pueden estar vacíos");
                }
                if !(1..=3).contains(&distractors.len()) {
                    return err(t, &format!("distractors tiene {} elementos, debe tener 1-3 (2-4 opciones en total)", distractors.len()));
                }
                if distractors.iter().any(|d| blank(&d.text) || blank(&d.feedback)) {
                    return err(t, "cada distractor necesita text y feedback (por qué es incorrecto)");
                }
                let texts = std::iter::once(correct.text.as_str()).chain(distractors.iter().map(|d| d.text.as_str()));
                if !all_unique(texts) {
                    return err(t, "las opciones deben ser distintas entre sí (un distractor repite otra opción)");
                }
                Ok(())
            }
            Self::TrueFalse { statement, explanation, .. } => {
                if blank(statement) || blank(explanation) {
                    return err("tpl_true_false", "statement y explanation no pueden estar vacíos");
                }
                Ok(())
            }
            Self::Ordering { question, steps_in_order } => {
                let t = "tpl_ordering";
                if blank(question) {
                    return err(t, "question no puede estar vacío");
                }
                if !(3..=4).contains(&steps_in_order.len()) {
                    return err(t, &format!("stepsInOrder tiene {} eslabones, debe tener 3-4", steps_in_order.len()));
                }
                if steps_in_order.iter().any(|s| blank(s)) || !all_unique(steps_in_order.iter().map(String::as_str)) {
                    return err(t, "stepsInOrder tiene eslabones vacíos o repetidos");
                }
                Ok(())
            }
            Self::ScenarioChoice { scenario, decision_point, optimal, alternatives } => {
                let t = "tpl_scenario_choice";
                if blank(scenario) || blank(decision_point) {
                    return err(t, "scenario y decisionPoint no pueden estar vacíos");
                }
                if !(1..=3).contains(&alternatives.len()) {
                    return err(t, &format!("alternatives tiene {} elementos, debe tener 1-3 (2-4 ramas en total)", alternatives.len()));
                }
                let all = std::iter::once(optimal).chain(alternatives.iter());
                if all.clone().any(|b| blank(&b.choice) || blank(&b.consequence)) {
                    return err(t, "cada rama necesita choice y consequence");
                }
                if !all_unique(all.map(|b| b.choice.as_str())) {
                    return err(t, "las decisiones (choice) deben ser distintas entre sí");
                }
                Ok(())
            }
            Self::Theory { title, intuitive_hook, analogy_boundary, system_rule, frequent_error } => {
                let t = "tpl_theory";
                if [title, intuitive_hook, analogy_boundary, system_rule, frequent_error].iter().any(|f| blank(f)) {
                    return err(t, "title, intuitiveHook, analogyBoundary, systemRule y frequentError no pueden estar vacíos");
                }
                let words = |s: &str| s.split_whitespace().count();
                let (hook, boundary, error) = (words(intuitive_hook), words(analogy_boundary), words(frequent_error));
                if hook + boundary + error > THEORY_MAX_OTHER_LAYERS_WORDS {
                    return err(
                        t,
                        &format!(
                            "intuitiveHook ({hook}) + analogyBoundary ({boundary}) + frequentError ({error}) suman {} palabras, máximo \
                             {THEORY_MAX_OTHER_LAYERS_WORDS} (≤30/≤25/≤40 por campo): recorta el campo más largo",
                            hook + boundary + error
                        ),
                    );
                }
                let rule = words(system_rule);
                if rule > THEORY_MAX_SYSTEM_RULE_WORDS {
                    return err(t, &format!("systemRule tiene {rule} palabras, máximo {THEORY_MAX_SYSTEM_RULE_WORDS}"));
                }
                Ok(())
            }
            Self::BoundaryCases { question, cases } => {
                let t = "tpl_boundary_cases";
                if blank(question) {
                    return err(t, "question no puede estar vacío");
                }
                if !(2..=3).contains(&cases.len()) {
                    return err(t, &format!("cases tiene {} elementos, debe tener 2-3", cases.len()));
                }
                if cases.iter().any(|c| blank(&c.statement)) || !all_unique(cases.iter().map(|c| c.statement.as_str())) {
                    return err(t, "cada caso necesita un statement distinto");
                }
                if !cases.iter().any(|c| c.follows_rule) || cases.iter().all(|c| c.follows_rule) {
                    return err(t, "debe haber al menos un caso que cumple la regla y uno que la rompe (followsRule)");
                }
                Ok(())
            }
            Self::QuickRecall { items } => {
                if !(1..=2).contains(&items.len()) {
                    return err("tpl_quick_recall", &format!("items tiene {} elementos, debe tener 1-2", items.len()));
                }
                if items.iter().any(|i| blank(&i.concept_label) || blank(&i.prompt) || blank(&i.expected_answer)) {
                    return err("tpl_quick_recall", "conceptLabel, prompt y expectedAnswer no pueden estar vacíos");
                }
                Ok(())
            }
        }
    }

    /// Expands into the full block. Assumes [`validate`](Self::validate)
    /// passed; never fails, and is deterministic (same params → same block).
    pub fn expand(self) -> GeneratedSectionBlock {
        match self {
            Self::MultipleChoice { question, correct, distractors } => {
                let seed = fnv(&format!("{question}|{}", correct.text));
                let mut specs: Vec<OptionSpec> = std::iter::once(correct.clone()).chain(distractors).collect();
                shuffle(&mut specs, seed);
                multiple_choice(question, specs.iter().map(|s| s.text.clone()).collect(), correct.text.clone(), specs)
            }
            Self::TrueFalse { statement, is_true, explanation } => {
                let right = if is_true { "Verdadero" } else { "Falso" };
                let specs = ["Verdadero", "Falso"]
                    .into_iter()
                    .map(|text| OptionSpec {
                        text: text.to_string(),
                        feedback: if text == right { format!("Correcto. {explanation}") } else { format!("No: {explanation}") },
                    })
                    .collect::<Vec<_>>();
                multiple_choice(format!("Verdadero o falso: {statement}"), vec!["Verdadero".to_string(), "Falso".to_string()], right.to_string(), specs)
            }
            Self::Ordering { question, steps_in_order } => {
                let mut shuffled = steps_in_order.clone();
                shuffle(&mut shuffled, fnv(&format!("{question}|{}", steps_in_order.join("|"))));
                if shuffled == steps_in_order {
                    // The scrambled list must differ from the answer, or the
                    // block is already solved.
                    shuffled.rotate_left(1);
                }
                GeneratedSectionBlock::InteractivePredictionGate {
                    question,
                    options: Vec::new(),
                    conceptual_feedback_map: HashMap::new(),
                    correct_option: String::new(),
                    interaction_mode: Some(InteractionMode::CausalChainOrder),
                    shuffled_steps: Some(shuffled),
                    correct_order: Some(steps_in_order),
                    boundary_cases: None,
                    visual_aid: None,
                }
            }
            Self::ScenarioChoice { scenario, decision_point, optimal, alternatives } => {
                let seed = fnv(&format!("{scenario}|{}", optimal.choice));
                let mut branches: Vec<ScenarioBranch> = std::iter::once(ScenarioBranch { choice: optimal.choice, consequence: optimal.consequence, is_optimal: true })
                    .chain(alternatives.into_iter().map(|a| ScenarioBranch { choice: a.choice, consequence: a.consequence, is_optimal: false }))
                    .collect();
                shuffle(&mut branches, seed);
                GeneratedSectionBlock::BranchingScenarioChallenge { scenario, decision_point, branches, visual_aid: None }
            }
            Self::QuickRecall { items } => GeneratedSectionBlock::SpacedInterleavedRetrieval { items },
            Self::Theory { title, intuitive_hook, analogy_boundary, system_rule, frequent_error } => {
                GeneratedSectionBlock::AnchoredMicroTheory { title, intuitive_hook, analogy_boundary: Some(analogy_boundary), system_rule, frequent_error }
            }
            Self::BoundaryCases { question, mut cases } => {
                let seed = fnv(&format!("{question}|{}", cases.iter().map(|c| c.statement.as_str()).collect::<Vec<_>>().join("|")));
                shuffle(&mut cases, seed);
                GeneratedSectionBlock::InteractivePredictionGate {
                    question,
                    options: Vec::new(),
                    conceptual_feedback_map: HashMap::new(),
                    correct_option: String::new(),
                    interaction_mode: Some(InteractionMode::BoundaryCaseAudit),
                    shuffled_steps: None,
                    correct_order: None,
                    boundary_cases: Some(cases),
                    visual_aid: None,
                }
            }
        }
    }
}

fn multiple_choice(question: String, options: Vec<String>, correct_option: String, specs: Vec<OptionSpec>) -> GeneratedSectionBlock {
    GeneratedSectionBlock::InteractivePredictionGate {
        question,
        options,
        conceptual_feedback_map: specs.into_iter().map(|s| (s.text, s.feedback)).collect(),
        correct_option,
        interaction_mode: Some(InteractionMode::MultipleChoice),
        shuffled_steps: None,
        correct_order: None,
        boundary_cases: None,
        visual_aid: None,
    }
}

/// FNV-1a: stable across runs and platforms (unlike `DefaultHasher`), so the
/// same template always expands to the same block.
pub(crate) fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

/// Deterministic Fisher–Yates driven by an LCG seeded from the content.
pub(crate) fn shuffle<T>(items: &mut [T], seed: u64) {
    let mut state = seed | 1;
    for i in (1..items.len()).rev() {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let j = ((state >> 33) as usize) % (i + 1);
        items.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opt(t: &str) -> OptionSpec {
        OptionSpec { text: t.to_string(), feedback: format!("fb {t}") }
    }

    fn mc() -> BlockTemplate {
        BlockTemplate::MultipleChoice { question: "¿Qué pasa?".to_string(), correct: opt("Correcta"), distractors: vec![opt("Mala 1"), opt("Mala 2")] }
    }

    #[test]
    fn multiple_choice_derives_a_consistent_answer_key() {
        let GeneratedSectionBlock::InteractivePredictionGate { options, conceptual_feedback_map, correct_option, interaction_mode, .. } = mc().expand() else {
            panic!("expands to a prediction gate");
        };
        assert_eq!(interaction_mode, Some(InteractionMode::MultipleChoice));
        assert_eq!(options.len(), 3);
        assert!(options.contains(&correct_option), "correctOption is always one of the options");
        assert_eq!(correct_option, "Correcta");
        assert!(options.iter().all(|o| conceptual_feedback_map.contains_key(o)), "every option has feedback");
        assert_eq!(conceptual_feedback_map["Mala 1"], "fb Mala 1");
    }

    #[test]
    fn expansion_is_deterministic_and_does_not_always_put_the_answer_first() {
        let first = |t: BlockTemplate| match t.expand() {
            GeneratedSectionBlock::InteractivePredictionGate { options, .. } => options,
            _ => panic!(),
        };
        assert_eq!(first(mc()), first(mc()));
        let positions: HashSet<usize> = (0..40)
            .map(|n| {
                let t = BlockTemplate::MultipleChoice {
                    question: format!("pregunta {n}"),
                    correct: opt("Correcta"),
                    distractors: vec![opt("Mala 1"), opt("Mala 2"), opt("Mala 3")],
                };
                first(t).iter().position(|o| o == "Correcta").unwrap()
            })
            .collect();
        assert!(positions.len() >= 3, "the right answer lands in varied positions, got {positions:?}");
    }

    #[test]
    fn theory_template_checks_word_caps_at_call_time_and_expands_to_the_free_form_block() {
        let ok = |hook: &str, rule: &str| BlockTemplate::Theory {
            title: "t".to_string(),
            intuitive_hook: hook.to_string(),
            analogy_boundary: "no cubre X".to_string(),
            system_rule: rule.to_string(),
            frequent_error: "esto SÍ a / esto NO b, porque c".to_string(),
        };
        assert!(ok("gancho corto", "1 ➔ 2 ➔ 3").validate().is_ok());
        let long_hook = "palabra ".repeat(101);
        let e = ok(&long_hook, "r").validate().unwrap_err();
        assert!(e.contains("tpl_theory") && e.contains("suman"), "{e}");
        let long_rule = "palabra ".repeat(THEORY_MAX_SYSTEM_RULE_WORDS + 1);
        assert!(ok("h", &long_rule).validate().unwrap_err().contains("systemRule"));
        assert!(matches!(
            ok("h", "r").expand(),
            GeneratedSectionBlock::AnchoredMicroTheory { analogy_boundary: Some(_), .. }
        ));
    }

    #[test]
    fn boundary_cases_template_needs_a_contrast_and_keeps_the_key_on_each_case() {
        let case = |s: &str, f: bool| BoundaryCase { statement: s.to_string(), follows_rule: f };
        let tpl = |cases| BlockTemplate::BoundaryCases { question: "q".to_string(), cases };
        assert!(tpl(vec![case("a", true), case("b", false)]).validate().is_ok());
        assert!(tpl(vec![case("a", true), case("b", true)]).validate().is_err());
        assert!(tpl(vec![case("a", false)]).validate().is_err());
        let GeneratedSectionBlock::InteractivePredictionGate { interaction_mode, boundary_cases, options, .. } =
            tpl(vec![case("a", true), case("b", false), case("c", true)]).expand()
        else {
            panic!("expands to a prediction gate");
        };
        assert_eq!(interaction_mode, Some(InteractionMode::BoundaryCaseAudit));
        assert!(options.is_empty());
        let by_text: HashMap<_, _> = boundary_cases.unwrap().into_iter().map(|c| (c.statement, c.follows_rule)).collect();
        assert_eq!((by_text["a"], by_text["b"], by_text["c"]), (true, false, true));
    }

    #[test]
    fn diagnostic_battery_shuffle_keeps_the_key_valid_and_moves_the_answer() {
        use super::super::notebook::{DiagnosticBattery, DiagnosticDimension, DiagnosticQuestion};
        let question = |n: usize| DiagnosticQuestion {
            dimension: DiagnosticDimension::Intuition,
            prompt: format!("pregunta {n}"),
            options: vec!["Correcta".to_string(), "Mala 1".to_string(), "Mala 2".to_string(), "Mala 3".to_string()],
            correct_option: "Correcta".to_string(),
            diagnostic_insight: "i".to_string(),
        };
        let mut battery = DiagnosticBattery { goal_alignment: "g".to_string(), questions: (0..40).map(question).collect() };
        battery.shuffle_options();
        let mut positions = HashSet::new();
        for q in &battery.questions {
            assert_eq!(q.options.len(), 4);
            let pos = q.options.iter().position(|o| *o == q.correct_option).expect("key still one of the options");
            positions.insert(pos);
        }
        assert!(positions.len() >= 3, "the correct option lands in varied positions, got {positions:?}");
        let mut again = DiagnosticBattery { goal_alignment: "g".to_string(), questions: (0..40).map(question).collect() };
        again.shuffle_options();
        assert_eq!(again.questions[7].options, battery.questions[7].options, "deterministic");
    }

    #[test]
    fn true_false_maps_to_two_fixed_options_with_the_right_key() {
        let t = BlockTemplate::TrueFalse { statement: "El cielo es verde".to_string(), is_true: false, explanation: "Es azul por dispersión".to_string() };
        let GeneratedSectionBlock::InteractivePredictionGate { question, options, conceptual_feedback_map, correct_option, .. } = t.expand() else { panic!() };
        assert!(question.starts_with("Verdadero o falso:"));
        assert_eq!(options, vec!["Verdadero", "Falso"]);
        assert_eq!(correct_option, "Falso");
        assert!(conceptual_feedback_map["Falso"].starts_with("Correcto."));
        assert!(conceptual_feedback_map["Verdadero"].starts_with("No:"));
    }

    #[test]
    fn ordering_scrambles_but_keeps_the_same_steps() {
        let steps: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| s.to_string()).collect();
        for n in 0..30 {
            let t = BlockTemplate::Ordering { question: format!("q{n}"), steps_in_order: steps.clone() };
            let GeneratedSectionBlock::InteractivePredictionGate { shuffled_steps, correct_order, interaction_mode, options, .. } = t.expand() else { panic!() };
            let (shuffled, key) = (shuffled_steps.unwrap(), correct_order.unwrap());
            assert_eq!(interaction_mode, Some(InteractionMode::CausalChainOrder));
            assert!(options.is_empty());
            assert_eq!(key, steps);
            assert_ne!(shuffled, key, "never ships an already-solved ordering");
            let (mut a, mut b) = (shuffled.clone(), key.clone());
            a.sort();
            b.sort();
            assert_eq!(a, b);
        }
    }

    #[test]
    fn scenario_has_exactly_one_optimal_branch() {
        let b = |c: &str| BranchSpec { choice: c.to_string(), consequence: format!("cons {c}") };
        let t = BlockTemplate::ScenarioChoice { scenario: "s".to_string(), decision_point: "d".to_string(), optimal: b("óptima"), alternatives: vec![b("x"), b("y")] };
        let GeneratedSectionBlock::BranchingScenarioChallenge { branches, .. } = t.expand() else { panic!() };
        assert_eq!(branches.len(), 3);
        assert_eq!(branches.iter().filter(|b| b.is_optimal).count(), 1);
        assert_eq!(branches.iter().find(|b| b.is_optimal).unwrap().choice, "óptima");
    }

    #[test]
    fn quick_recall_passes_items_through() {
        let t = BlockTemplate::QuickRecall { items: vec![RetrievalPrompt { concept_label: "c".into(), prompt: "p".into(), expected_answer: "e".into() }] };
        assert!(matches!(t.expand(), GeneratedSectionBlock::SpacedInterleavedRetrieval { items } if items.len() == 1));
    }

    #[test]
    fn validation_names_the_exact_problem() {
        let dup = BlockTemplate::MultipleChoice { question: "q".into(), correct: opt("A"), distractors: vec![opt(" a ")] };
        assert!(dup.validate().unwrap_err().contains("distintas"));
        let none = BlockTemplate::MultipleChoice { question: "q".into(), correct: opt("A"), distractors: vec![] };
        assert!(none.validate().unwrap_err().contains("distractors"));
        let few = BlockTemplate::Ordering { question: "q".into(), steps_in_order: vec!["a".into(), "b".into()] };
        assert!(few.validate().unwrap_err().contains("3-4"));
        let repeated = BlockTemplate::Ordering { question: "q".into(), steps_in_order: vec!["a".into(), "A".into(), "c".into()] };
        assert!(repeated.validate().unwrap_err().contains("repetidos"));
        let empty_tf = BlockTemplate::TrueFalse { statement: " ".into(), is_true: true, explanation: "x".into() };
        assert!(empty_tf.validate().is_err());
        assert!(mc().validate().is_ok());
    }

    #[test]
    fn template_tags_are_recognised_and_parse_from_tool_json() {
        assert!(BlockTemplate::is_template_tag("tpl_ordering"));
        assert!(!BlockTemplate::is_template_tag("interactive_prediction_gate"));
        let json = serde_json::json!({
            "blockType": "tpl_multiple_choice",
            "question": "q",
            "correct": {"text": "A", "feedback": "f"},
            "distractors": [{"text": "B", "feedback": "g"}]
        });
        let t: BlockTemplate = serde_json::from_value(json).expect("parses");
        assert!(t.validate().is_ok());
    }
}
