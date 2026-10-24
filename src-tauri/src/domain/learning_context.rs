//! What the learner brings: what they already know, what they have to practice
//! with (a camera, an operating system, lab material) and what limits them.
//! A plan that ignores it turns into theory the student cannot practice.
//!
//! The questions are NOT a fixed catalog: they depend on the topic (biology of
//! mutations asks about prior knowledge of DNA, photography asks about the
//! camera), so a model proposes them once per course. What keeps that safe is
//! that the model only SUGGESTS: [`validate_question_set`] checks the shape in
//! code, the validated set is stored on the session as the server's truth, and
//! answers are validated against THAT set — never against what the client
//! claims the questions were. If the model fails, [`fallback_question_set`]
//! builds a generic set in code, so the form can never dead-end.
//!
//! A learner can always say they do not know. Every choice question accepts
//! [`UNKNOWN_ANSWER`] whatever options the model wrote, and a question may carry
//! simpler FOLLOW-UP questions that are asked only after that answer ("¿con qué
//! computadora?" -> "no lo sé" -> "¿qué ves al encender tu computadora?"). Not
//! knowing never blocks the form and never forces a guess.
//!
//! The learner's starting LEVEL is not asked directly either. It is deduced
//! from the answers to the prior-knowledge questions (each option carries a
//! signal 0..=2; "no lo sé" counts as 0), and the diagnostic battery later
//! checks it.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::domain::roadmap::EntryLevel;

pub const MIN_QUESTIONS: usize = 2;
pub const MAX_QUESTIONS: usize = 5;
pub const MAX_OPTIONS: usize = 6;
const MAX_LABEL_CHARS: usize = 140;
const MAX_OPTION_CHARS: usize = 60;
const MAX_ID_CHARS: usize = 24;
/// Longest free-text answer kept: a hint, not an essay.
pub const MAX_TEXT_ANSWER_CHARS: usize = 200;
/// Highest prior-knowledge signal: 0 = nothing, 1 = some, 2 = solid.
const MAX_SIGNAL: u8 = 2;
/// Follow-up questions one question may carry, and the most a whole set may have.
const MAX_FOLLOW_UPS_PER_QUESTION: usize = 2;
const MAX_FOLLOW_UPS_TOTAL: usize = 4;

/// The answer every choice question accepts, whatever options it lists. The form
/// shows it as the last option of each one.
pub const UNKNOWN_ANSWER: &str = "No lo sé";

/// For persisted fields holding a [`LearningContext`]: a value in an older shape
/// (or otherwise unreadable) becomes `None` instead of failing the whole record
/// it lives in — a session or a course must never become unloadable over an
/// optional extra.
pub fn deserialize_lenient<'de, D>(deserializer: D) -> Result<Option<LearningContext>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|v| serde_json::from_value(v).ok()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionKind {
    /// One of `options` (or [`UNKNOWN_ANSWER`]); required.
    Choice,
    /// Free text up to [`MAX_TEXT_ANSWER_CHARS`]; optional.
    Text,
}

/// Why the question is asked: it decides how the answer is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionPurpose {
    /// What the learner already knows. Its options carry a level signal.
    PriorKnowledge,
    /// What they have to practice with (camera, OS, lab material, software).
    Equipment,
    /// A limit that shapes the plan (no internet at home, little free time on weekdays).
    Constraint,
}

// --- What the model returns (lenient on purpose; validated below) -----------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawOption {
    pub label: String,
    /// Only meaningful on prior-knowledge options.
    #[serde(default)]
    pub signal: Option<u8>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawQuestion {
    pub id: String,
    pub label: String,
    pub kind: QuestionKind,
    pub purpose: QuestionPurpose,
    #[serde(default)]
    pub options: Vec<RawOption>,
    /// Simpler questions asked ONLY if the learner answers "no lo sé" to this
    /// one. One level deep: a follow-up cannot carry its own.
    #[serde(default, rename = "followUps", alias = "follow_ups")]
    pub follow_ups: Vec<RawQuestion>,
}

/// `submit_context_questions`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ContextQuestionsArgs {
    pub questions: Vec<RawQuestion>,
}

// --- Validated: stored on the session and sent to the form ------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextOption {
    pub label: String,
    /// `Some(0..=2)` on prior-knowledge options, `None` elsewhere.
    #[serde(default)]
    pub signal: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextQuestion {
    pub id: String,
    pub label: String,
    pub kind: QuestionKind,
    pub purpose: QuestionPurpose,
    #[serde(default)]
    pub options: Vec<ContextOption>,
    /// Asked only after [`UNKNOWN_ANSWER`] to this question.
    #[serde(default, rename = "followUps")]
    pub follow_ups: Vec<ContextQuestion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextQuestionSet {
    pub questions: Vec<ContextQuestion>,
}

fn clean(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect::<String>().trim_end().to_string()
}

fn valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    id.chars().count() <= MAX_ID_CHARS
        && chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Validates ONE question (and, at top level, its follow-ups). `seen` holds every
/// id used so far across the whole set, follow-ups included, so ids are unique
/// set-wide — an answer names its question by id alone.
fn validate_question(q: &RawQuestion, top_level: bool, seen: &mut HashSet<String>, errors: &mut Vec<String>) -> Option<ContextQuestion> {
    let label = clean(&q.label, MAX_LABEL_CHARS);
    if !valid_id(&q.id) {
        errors.push(format!("id «{}» inválido: usa minúsculas, números y _ (máx. {MAX_ID_CHARS}), empezando por letra", q.id));
        return None;
    }
    if !seen.insert(q.id.clone()) {
        errors.push(format!("id «{}» repetido", q.id));
        return None;
    }
    if label.is_empty() {
        errors.push(format!("la pregunta «{}» no tiene texto", q.id));
        return None;
    }
    if !top_level && q.purpose == QuestionPurpose::PriorKnowledge {
        errors.push(format!("«{}»: una pregunta de seguimiento no puede ser de conocimiento previo", q.id));
        return None;
    }

    let mut question = ContextQuestion { id: q.id.clone(), label, kind: q.kind, purpose: q.purpose, options: vec![], follow_ups: vec![] };
    match q.kind {
        QuestionKind::Text => {
            if q.purpose == QuestionPurpose::PriorKnowledge {
                errors.push(format!("«{}»: una pregunta de conocimiento previo debe ser de opciones, no de texto", q.id));
                return None;
            }
        }
        QuestionKind::Choice => {
            let mut options: Vec<ContextOption> = Vec::new();
            for o in &q.options {
                let text = clean(&o.label, MAX_OPTION_CHARS);
                // The form adds "No lo sé" itself; one the model wrote would show twice.
                if text.is_empty() || text.eq_ignore_ascii_case(UNKNOWN_ANSWER) || options.iter().any(|x| x.label.eq_ignore_ascii_case(&text)) {
                    continue;
                }
                options.push(ContextOption { label: text, signal: (q.purpose == QuestionPurpose::PriorKnowledge).then_some(o.signal).flatten() });
            }
            if !(2..=MAX_OPTIONS).contains(&options.len()) {
                errors.push(format!("«{}»: necesita entre 2 y {MAX_OPTIONS} opciones distintas (sin contar «No lo sé»), tiene {}", q.id, options.len()));
                return None;
            }
            if q.purpose == QuestionPurpose::PriorKnowledge {
                let signals: Vec<u8> = options.iter().filter_map(|o| o.signal).collect();
                if signals.len() != options.len() || signals.iter().any(|s| *s > MAX_SIGNAL) {
                    errors.push(format!("«{}»: cada opción de conocimiento previo necesita un signal entre 0 y {MAX_SIGNAL}", q.id));
                    return None;
                }
                let distinct: std::collections::BTreeSet<u8> = signals.iter().copied().collect();
                if !distinct.contains(&0) || distinct.len() < 2 {
                    errors.push(format!("«{}»: sus opciones deben incluir «nada todavía» (signal 0) y al menos otro nivel distinto", q.id));
                    return None;
                }
            }
            question.options = options;
            if top_level {
                for follow_up in q.follow_ups.iter().take(MAX_FOLLOW_UPS_PER_QUESTION) {
                    if let Some(valid) = validate_question(follow_up, false, seen, errors) {
                        question.follow_ups.push(valid);
                    }
                }
            }
        }
    }
    Some(question)
}

/// Checks what the model proposed and returns every problem at once (for the
/// corrective retry). Rules:
/// - between [`MIN_QUESTIONS`] and [`MAX_QUESTIONS`] top-level questions, unique
///   `snake_case` ids across the whole set;
/// - labels non-empty and capped; choices have 2..=[`MAX_OPTIONS`] distinct
///   options (not counting "No lo sé", which the form always adds); a text
///   question has none;
/// - at least ONE prior-knowledge question (the level is deduced from it), and
///   every one of them is a choice whose options carry signals 0..=2 that
///   include a 0 ("nothing yet") and at least one other value — a question
///   that cannot tell a beginner from an expert deduces nothing;
/// - follow-ups hang only off choice questions, one level deep, never about
///   prior knowledge, at most [`MAX_FOLLOW_UPS_TOTAL`] in all;
/// - signals on any other purpose are dropped.
pub fn validate_question_set(args: &ContextQuestionsArgs) -> Result<ContextQuestionSet, Vec<String>> {
    let mut errors = Vec::new();
    if !(MIN_QUESTIONS..=MAX_QUESTIONS).contains(&args.questions.len()) {
        errors.push(format!("debe haber entre {MIN_QUESTIONS} y {MAX_QUESTIONS} preguntas, no {}", args.questions.len()));
    }
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<ContextQuestion> = Vec::new();
    for q in args.questions.iter().take(MAX_QUESTIONS) {
        if let Some(valid) = validate_question(q, true, &mut seen, &mut errors) {
            out.push(valid);
        }
    }
    let follow_ups: usize = out.iter().map(|q| q.follow_ups.len()).sum();
    if follow_ups > MAX_FOLLOW_UPS_TOTAL {
        errors.push(format!("demasiadas preguntas de seguimiento ({follow_ups}); máximo {MAX_FOLLOW_UPS_TOTAL} en total"));
    }
    if !out.iter().any(|q| q.purpose == QuestionPurpose::PriorKnowledge) && errors.iter().all(|e| !e.contains("conocimiento previo")) {
        errors.push("falta al menos una pregunta de conocimiento previo (purpose = prior_knowledge)".to_string());
    }
    if errors.is_empty() { Ok(ContextQuestionSet { questions: out }) } else { Err(errors) }
}

/// A generic, always-valid set: how much they already know about the topic and
/// what they have to practice with. Used when the model could not produce a
/// valid set, so the form never dead-ends. The topic is learner text and only
/// ever lands inside a label, never an instruction.
pub fn fallback_question_set(topic: &str) -> ContextQuestionSet {
    let topic = clean(topic, 60);
    let about = if topic.is_empty() { "este tema".to_string() } else { topic };
    ContextQuestionSet {
        questions: vec![
            ContextQuestion {
                id: "prior".to_string(),
                label: format!("¿Qué tanto sabes ya sobre {about}?"),
                kind: QuestionKind::Choice,
                purpose: QuestionPurpose::PriorKnowledge,
                options: vec![
                    ContextOption { label: "Nada, empiezo de cero".to_string(), signal: Some(0) },
                    ContextOption { label: "Conozco algo, pero casi no lo he aplicado".to_string(), signal: Some(1) },
                    ContextOption { label: "Ya lo he aplicado y quiero profundizar".to_string(), signal: Some(2) },
                ],
                follow_ups: vec![],
            },
            ContextQuestion {
                id: "tools".to_string(),
                label: "¿Con qué herramientas o material cuentas para practicar?".to_string(),
                kind: QuestionKind::Text,
                purpose: QuestionPurpose::Equipment,
                options: vec![],
                follow_ups: vec![],
            },
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextAnswer {
    pub question_id: String,
    pub answer: String,
}

/// One answered question, with the question's text frozen next to the answer so
/// nothing later has to look the question up again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnsweredQuestion {
    pub question_id: String,
    pub label: String,
    pub purpose: QuestionPurpose,
    pub answer: String,
}

/// The validated, saved answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningContext {
    pub answers: Vec<AnsweredQuestion>,
}

impl LearningContext {
    /// Validates `answers` against the server's own copy of the questions:
    /// - each answer must belong to a question of the set, once;
    /// - a choice must be one of its options or [`UNKNOWN_ANSWER`], and every
    ///   choice that is currently ASKED must be answered;
    /// - a follow-up is asked only when its parent was answered "no lo sé":
    ///   answering one otherwise is refused, and when its parent was "no lo sé"
    ///   its choices are required;
    /// - free text is trimmed, capped and may be blank.
    /// Returns every problem at once.
    pub fn validate(set: &ContextQuestionSet, answers: &[ContextAnswer]) -> Result<Self, Vec<String>> {
        let mut index: HashMap<&str, (&ContextQuestion, Option<&str>)> = HashMap::new();
        for q in &set.questions {
            index.insert(q.id.as_str(), (q, None));
            for f in &q.follow_ups {
                index.insert(f.id.as_str(), (f, Some(q.id.as_str())));
            }
        }
        // Which top-level questions were answered "no lo sé" (activates their follow-ups).
        let unknown_parents: HashSet<&str> = answers
            .iter()
            .filter(|a| a.answer.trim() == UNKNOWN_ANSWER)
            .filter(|a| index.get(a.question_id.as_str()).is_some_and(|(_, parent)| parent.is_none()))
            .map(|a| a.question_id.as_str())
            .collect();

        let mut errors = Vec::new();
        let mut kept: Vec<AnsweredQuestion> = Vec::new();
        for a in answers {
            let Some((q, parent)) = index.get(a.question_id.as_str()).copied() else {
                errors.push(format!("la pregunta «{}» no existe", a.question_id));
                continue;
            };
            if kept.iter().any(|k| k.question_id == q.id) {
                errors.push(format!("la pregunta «{}» está repetida", q.id));
                continue;
            }
            if parent.is_some_and(|p| !unknown_parents.contains(p)) {
                errors.push(format!("«{}» solo se pregunta si la anterior fue «{UNKNOWN_ANSWER}»", q.label));
                continue;
            }
            let value = a.answer.trim();
            match q.kind {
                QuestionKind::Choice if value != UNKNOWN_ANSWER && !q.options.iter().any(|o| o.label == value) => {
                    errors.push(format!("«{value}» no es una opción de «{}»", q.label));
                }
                QuestionKind::Text if value.chars().count() > MAX_TEXT_ANSWER_CHARS => {
                    errors.push(format!("la respuesta a «{}» supera {MAX_TEXT_ANSWER_CHARS} caracteres", q.label));
                }
                _ if value.is_empty() => {} // blank free text = unanswered
                _ => kept.push(AnsweredQuestion { question_id: q.id.clone(), label: q.label.clone(), purpose: q.purpose, answer: value.to_string() }),
            }
        }

        // Every choice that is being asked must be answered: top-level ones always,
        // follow-ups only under a parent answered "no lo sé".
        let asked = set.questions.iter().flat_map(|q| {
            let active_follow_ups: &[ContextQuestion] = if unknown_parents.contains(q.id.as_str()) { &q.follow_ups } else { &[] };
            std::iter::once(q).chain(active_follow_ups.iter())
        });
        for q in asked.filter(|q| q.kind == QuestionKind::Choice) {
            let answered = kept.iter().any(|k| k.question_id == q.id);
            let already_reported = errors.iter().any(|e| e.contains(q.label.as_str()));
            if !answered && !already_reported {
                errors.push(format!("falta responder «{}»", q.label));
            }
        }
        if errors.is_empty() { Ok(Self { answers: kept }) } else { Err(errors) }
    }

    /// Human-readable form handed to the model: each answer next to the QUESTION
    /// it answers. Free text is data, never an instruction. A "No lo sé" is kept
    /// as written: it means the learner does not know, NOT that the answer is
    /// anything in particular.
    pub fn describe(&self) -> serde_json::Value {
        let answers: Vec<serde_json::Value> =
            self.answers.iter().map(|a| serde_json::json!({ "question": a.label, "purpose": a.purpose, "answer": a.answer })).collect();
        serde_json::json!({ "answers": answers })
    }

    pub fn answer(&self, question_id: &str) -> Option<&str> {
        self.answers.iter().find(|a| a.question_id == question_id).map(|a| a.answer.as_str())
    }
}

/// The starting level the prior-knowledge answers indicate. Nothing but zeros is
/// `AbsoluteZero` (which skips the battery) — and "no lo sé" counts as zero; an
/// average of 1.5 or more is `AppliedIntermediate`; anything between is
/// `TheoreticalFoundations`. Only top-level prior-knowledge CHOICE questions
/// count; with none answered it errs on the side of a battery
/// (`TheoreticalFoundations`), never on skipping it.
pub fn derive_entry_level(set: &ContextQuestionSet, context: &LearningContext) -> EntryLevel {
    let signals: Vec<u32> = set
        .questions
        .iter()
        .filter(|q| q.purpose == QuestionPurpose::PriorKnowledge && q.kind == QuestionKind::Choice)
        .filter_map(|q| {
            let answer = context.answer(&q.id)?;
            if answer == UNKNOWN_ANSWER {
                return Some(0);
            }
            q.options.iter().find(|o| o.label == answer).and_then(|o| o.signal).map(u32::from)
        })
        .collect();
    if signals.is_empty() {
        return EntryLevel::TheoreticalFoundations;
    }
    let sum: u32 = signals.iter().sum();
    if sum == 0 {
        EntryLevel::AbsoluteZero
    } else if sum * 2 >= 3 * signals.len() as u32 {
        EntryLevel::AppliedIntermediate
    } else {
        EntryLevel::TheoreticalFoundations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opt(label: &str, signal: Option<u8>) -> RawOption {
        RawOption { label: label.to_string(), signal }
    }

    fn prior(id: &str) -> RawQuestion {
        RawQuestion {
            id: id.to_string(),
            label: "¿Qué sabes de ADN?".to_string(),
            kind: QuestionKind::Choice,
            purpose: QuestionPurpose::PriorKnowledge,
            options: vec![opt("Nada todavía", Some(0)), opt("Lo básico", Some(1)), opt("Lo manejo bien", Some(2))],
            follow_ups: vec![],
        }
    }

    fn text(id: &str, purpose: QuestionPurpose) -> RawQuestion {
        RawQuestion { id: id.to_string(), label: "¿Con qué material cuentas?".to_string(), kind: QuestionKind::Text, purpose, options: vec![], follow_ups: vec![] }
    }

    /// A choice about equipment, e.g. "¿Con qué computadora vas a practicar?".
    fn equipment(id: &str, follow_ups: Vec<RawQuestion>) -> RawQuestion {
        RawQuestion {
            id: id.to_string(),
            label: format!("¿Con qué equipo practicas ({id})?"),
            kind: QuestionKind::Choice,
            purpose: QuestionPurpose::Equipment,
            options: vec![opt("Windows", None), opt("Mac", None), opt("Linux", None)],
            follow_ups,
        }
    }

    fn simple_follow_up(id: &str) -> RawQuestion {
        RawQuestion {
            id: id.to_string(),
            label: "¿Qué ves al encender tu computadora?".to_string(),
            kind: QuestionKind::Choice,
            purpose: QuestionPurpose::Equipment,
            options: vec![opt("El escritorio de Windows", None), opt("Una manzana al arrancar", None), opt("Otra cosa", None)],
            follow_ups: vec![],
        }
    }

    fn ans(q: &str, a: &str) -> ContextAnswer {
        ContextAnswer { question_id: q.to_string(), answer: a.to_string() }
    }

    fn set_of(questions: Vec<RawQuestion>) -> ContextQuestionSet {
        validate_question_set(&ContextQuestionsArgs { questions }).expect("valid set")
    }

    // --- Validating what the model proposes ---------------------------------------

    #[test]
    fn a_topic_specific_set_is_accepted() {
        // Biology of mutations: no equipment at all, only knowledge.
        let set = set_of(vec![
            prior("dna"),
            RawQuestion { id: "mutations".into(), label: "¿Has oído hablar de mutaciones?".into(), ..prior("x") },
            text("goal_context", QuestionPurpose::Constraint),
        ]);
        assert_eq!(set.questions.len(), 3);
        assert!(set.questions[0].options.iter().all(|o| o.signal.is_some()), "prior-knowledge options keep their signals");
    }

    #[test]
    fn signals_on_other_purposes_are_dropped() {
        let mut equipment = prior("camera");
        equipment.purpose = QuestionPurpose::Equipment;
        let set = set_of(vec![prior("p"), equipment]);
        assert!(set.questions[1].options.iter().all(|o| o.signal.is_none()), "only prior knowledge feeds the level");
    }

    #[test]
    fn every_problem_is_reported_at_once() {
        let bad = ContextQuestionsArgs {
            questions: vec![
                RawQuestion { id: "Bad Id".into(), ..prior("x") },
                prior("dup"),
                prior("dup"),
                RawQuestion { id: "one_option".into(), label: "x".into(), kind: QuestionKind::Choice, purpose: QuestionPurpose::Equipment, options: vec![opt("solo una", None)], follow_ups: vec![] },
                RawQuestion { id: "t".into(), ..text("t", QuestionPurpose::PriorKnowledge) },
            ],
        };
        let errors = validate_question_set(&bad).expect_err("invalid").join(" | ");
        for expected in ["inválido", "repetido", "necesita entre 2", "debe ser de opciones"] {
            assert!(errors.contains(expected), "{expected:?} missing from: {errors}");
        }
    }

    #[test]
    fn a_question_without_text_and_a_wrong_number_of_questions_are_refused() {
        let blank = RawQuestion { id: "blank".into(), label: "  ".into(), ..text("blank", QuestionPurpose::Equipment) };
        let errors = validate_question_set(&ContextQuestionsArgs { questions: vec![prior("a"), blank] }).expect_err("blank label").join(" | ");
        assert!(errors.contains("no tiene texto"), "{errors}");

        let many = ContextQuestionsArgs { questions: (0..6).map(|i| prior(&format!("q{i}"))).collect() };
        let errors = validate_question_set(&many).expect_err("too many").join(" | ");
        assert!(errors.contains("entre 2 y 5 preguntas, no 6"), "{errors}");
        assert!(validate_question_set(&ContextQuestionsArgs { questions: vec![prior("solo")] }).is_err(), "too few");
    }

    #[test]
    fn the_set_must_contain_a_prior_knowledge_question_because_the_level_is_deduced_from_it() {
        let errors = validate_question_set(&ContextQuestionsArgs { questions: vec![text("a", QuestionPurpose::Equipment), text("b", QuestionPurpose::Constraint)] }).expect_err("none");
        assert!(errors.iter().any(|e| e.contains("falta al menos una pregunta de conocimiento previo")), "{errors:?}");
    }

    #[test]
    fn a_prior_knowledge_question_that_cannot_tell_levels_apart_is_refused() {
        let flat = RawQuestion { options: vec![opt("A", Some(1)), opt("B", Some(1))], ..prior("flat") };
        let no_zero = RawQuestion { options: vec![opt("A", Some(1)), opt("B", Some(2))], ..prior("nozero") };
        let missing = RawQuestion { options: vec![opt("A", Some(0)), opt("B", None)], ..prior("missing") };
        let out_of_range = RawQuestion { options: vec![opt("A", Some(0)), opt("B", Some(9))], ..prior("range") };
        for (q, why) in [(flat, "all the same"), (no_zero, "no 'nothing yet' option"), (missing, "an option without a signal"), (out_of_range, "signal beyond 2")] {
            let r = validate_question_set(&ContextQuestionsArgs { questions: vec![q, text("t", QuestionPurpose::Equipment)] });
            assert!(r.is_err(), "{why} must be refused");
        }
    }

    #[test]
    fn text_is_cut_and_duplicate_options_collapse() {
        let long = "x".repeat(500);
        let q = RawQuestion { label: long.clone(), options: vec![opt("Nada", Some(0)), opt("nada", Some(0)), opt(&long, Some(1))], ..prior("long") };
        let set = set_of(vec![q, text("t", QuestionPurpose::Equipment)]);
        assert_eq!(set.questions[0].label.chars().count(), MAX_LABEL_CHARS);
        assert_eq!(set.questions[0].options.len(), 2, "a case-insensitive duplicate collapsed");
        assert_eq!(set.questions[0].options[1].label.chars().count(), MAX_OPTION_CHARS);
    }

    #[test]
    fn the_fallback_set_is_always_valid_and_safe() {
        for topic in ["Mutaciones genéticas", "", "   ", &"z".repeat(300)] {
            let set = fallback_question_set(topic);
            let to_raw = |q: &ContextQuestion| RawQuestion {
                id: q.id.clone(),
                label: q.label.clone(),
                kind: q.kind,
                purpose: q.purpose,
                options: q.options.iter().map(|o| RawOption { label: o.label.clone(), signal: o.signal }).collect(),
                follow_ups: vec![],
            };
            let raw = ContextQuestionsArgs { questions: set.questions.iter().map(to_raw).collect() };
            assert!(validate_question_set(&raw).is_ok(), "the fallback must pass its own validation for {topic:?}");
        }
        assert!(fallback_question_set("Mutaciones genéticas").questions[0].label.contains("Mutaciones genéticas"));
    }

    // --- "No lo sé" and follow-ups: not knowing must never block the form ----------

    #[test]
    fn a_no_lo_se_written_by_the_model_is_not_duplicated() {
        let mut q = prior("dna");
        q.options.push(opt("no lo sé", Some(0)));
        let set = set_of(vec![q, text("t", QuestionPurpose::Equipment)]);
        assert_eq!(set.questions[0].options.len(), 3, "the form always adds its own «No lo sé»");
    }

    #[test]
    fn follow_ups_are_kept_and_their_ids_are_unique_across_the_whole_set() {
        let set = set_of(vec![prior("dna"), equipment("computer", vec![simple_follow_up("see_screen")])]);
        assert_eq!(set.questions[1].follow_ups.len(), 1);

        // The same id on a follow-up and a top-level question would make an answer ambiguous.
        let errors = validate_question_set(&ContextQuestionsArgs { questions: vec![prior("dna"), equipment("computer", vec![simple_follow_up("dna")])] }).expect_err("collision");
        assert!(errors.join(" | ").contains("repetido"), "{errors:?}");
    }

    #[test]
    fn follow_ups_cannot_be_nested_prior_knowledge_or_too_many() {
        // Nesting: a follow-up's own follow-ups are ignored (one level deep).
        let nested = RawQuestion { follow_ups: vec![simple_follow_up("deep")], ..simple_follow_up("mid") };
        let set = set_of(vec![prior("dna"), equipment("computer", vec![nested])]);
        assert!(set.questions[1].follow_ups[0].follow_ups.is_empty(), "no nesting");

        // A follow-up may not be about prior knowledge (the level comes from top-level ones).
        let about_knowledge = RawQuestion { purpose: QuestionPurpose::PriorKnowledge, ..simple_follow_up("fk") };
        let errors = validate_question_set(&ContextQuestionsArgs { questions: vec![prior("dna"), equipment("computer", vec![about_knowledge])] }).expect_err("refused");
        assert!(errors.join(" | ").contains("seguimiento no puede ser de conocimiento previo"), "{errors:?}");

        // Per question at most 2 are kept; in total at most 4 are allowed.
        let three = equipment("c1", vec![simple_follow_up("a"), simple_follow_up("b"), simple_follow_up("c")]);
        assert_eq!(set_of(vec![prior("dna"), three]).questions[1].follow_ups.len(), MAX_FOLLOW_UPS_PER_QUESTION);
        let qs = vec![
            prior("dna"),
            equipment("c1", vec![simple_follow_up("a"), simple_follow_up("b")]),
            equipment("c2", vec![simple_follow_up("c"), simple_follow_up("d")]),
            equipment("c3", vec![simple_follow_up("e")]),
        ];
        let errors = validate_question_set(&ContextQuestionsArgs { questions: qs }).expect_err("too many follow-ups");
        assert!(errors.join(" | ").contains("demasiadas preguntas de seguimiento"), "{errors:?}");
    }

    #[test]
    fn follow_ups_only_hang_off_choice_questions() {
        let mut free_text = text("notes", QuestionPurpose::Equipment);
        free_text.follow_ups = vec![simple_follow_up("orphan")];
        let set = set_of(vec![prior("dna"), free_text]);
        assert!(set.questions[1].follow_ups.is_empty(), "a text question has nothing to be 'unknown' about");
    }

    #[test]
    fn every_choice_accepts_no_lo_se_even_if_it_is_not_among_the_options() {
        let set = set_of(vec![prior("dna"), equipment("computer", vec![])]);
        let ctx = LearningContext::validate(&set, &[ans("dna", UNKNOWN_ANSWER), ans("computer", UNKNOWN_ANSWER)]).expect("not knowing is a valid answer");
        assert_eq!(ctx.answer("computer"), Some(UNKNOWN_ANSWER));
    }

    #[test]
    fn a_follow_up_is_asked_only_after_no_lo_se_and_is_required_then() {
        let set = set_of(vec![prior("dna"), equipment("computer", vec![simple_follow_up("see_screen")])]);

        // The parent answered normally: the follow-up is not asked, and answering it is refused.
        let ok = LearningContext::validate(&set, &[ans("dna", "Lo básico"), ans("computer", "Linux")]).expect("no follow-up needed");
        assert_eq!(ok.answers.len(), 2);
        let refused = LearningContext::validate(&set, &[ans("dna", "Lo básico"), ans("computer", "Linux"), ans("see_screen", "Otra cosa")]).expect_err("not asked");
        assert!(refused.join(" | ").contains("solo se pregunta si la anterior fue"), "{refused:?}");

        // The parent was "no lo sé": the follow-up is now REQUIRED...
        let missing = LearningContext::validate(&set, &[ans("dna", "Lo básico"), ans("computer", UNKNOWN_ANSWER)]).expect_err("follow-up required");
        assert!(missing.join(" | ").contains("falta responder «¿Qué ves al encender tu computadora?»"), "{missing:?}");
        // ...and may itself be "no lo sé", so nothing forces a guess.
        let both_unknown = LearningContext::validate(&set, &[ans("dna", "Lo básico"), ans("computer", UNKNOWN_ANSWER), ans("see_screen", UNKNOWN_ANSWER)]).expect("still valid");
        assert_eq!(both_unknown.answers.len(), 3);
        let answered = LearningContext::validate(&set, &[ans("dna", "Lo básico"), ans("computer", UNKNOWN_ANSWER), ans("see_screen", "El escritorio de Windows")]).expect("valid");
        assert_eq!(answered.answer("see_screen"), Some("El escritorio de Windows"));
    }

    // --- Answers ------------------------------------------------------------------

    #[test]
    fn answers_are_validated_against_the_servers_own_questions() {
        let set = set_of(vec![prior("dna"), text("material", QuestionPurpose::Equipment)]);
        let ok = LearningContext::validate(&set, &[ans("dna", "Lo básico"), ans("material", "  un microscopio  ")]).expect("valid");
        assert_eq!(ok.answer("material"), Some("un microscopio"));
        assert_eq!(ok.answers[0].label, "¿Qué sabes de ADN?", "the question text is frozen next to the answer");

        let errors = LearningContext::validate(&set, &[ans("dna", "Soy experto absoluto"), ans("ghost", "x"), ans("material", &"y".repeat(300))]).expect_err("invalid").join(" | ");
        assert!(errors.contains("no es una opción") && errors.contains("no existe") && errors.contains("supera"), "{errors}");
    }

    #[test]
    fn every_choice_must_be_answered_but_free_text_may_be_blank() {
        let set = set_of(vec![prior("dna"), text("material", QuestionPurpose::Equipment)]);
        let errors = LearningContext::validate(&set, &[ans("material", "")]).expect_err("choice unanswered");
        assert!(errors.iter().any(|e| e.contains("falta responder")), "{errors:?}");
        let ok = LearningContext::validate(&set, &[ans("dna", "Nada todavía"), ans("material", "   ")]).expect("blank text is fine");
        assert_eq!(ok.answer("material"), None);
    }

    #[test]
    fn a_repeated_answer_is_refused() {
        let set = set_of(vec![prior("dna"), text("material", QuestionPurpose::Equipment)]);
        let errors = LearningContext::validate(&set, &[ans("dna", "Nada todavía"), ans("dna", "Lo básico")]).expect_err("repeat");
        assert!(errors.iter().any(|e| e.contains("repetida")), "{errors:?}");
    }

    #[test]
    fn the_model_reads_each_answer_next_to_its_question() {
        let set = set_of(vec![prior("dna"), text("material", QuestionPurpose::Equipment)]);
        let ctx = LearningContext::validate(&set, &[ans("dna", "Nada todavía")]).expect("valid");
        let json = ctx.describe();
        assert_eq!(json["answers"][0], serde_json::json!({ "question": "¿Qué sabes de ADN?", "purpose": "prior_knowledge", "answer": "Nada todavía" }));
    }

    // --- Deducing the level -----------------------------------------------------------

    fn level_for(answers: &[(&str, &str)]) -> EntryLevel {
        let set = set_of(vec![prior("dna"), prior("mutations"), text("material", QuestionPurpose::Equipment)]);
        let given: Vec<ContextAnswer> = answers.iter().map(|(q, a)| ans(q, a)).collect();
        let ctx = LearningContext {
            answers: given
                .iter()
                .map(|a| AnsweredQuestion { question_id: a.question_id.clone(), label: String::new(), purpose: QuestionPurpose::PriorKnowledge, answer: a.answer.clone() })
                .collect(),
        };
        derive_entry_level(&set, &ctx)
    }

    #[test]
    fn nothing_known_is_absolute_zero_and_a_little_knowledge_is_not() {
        assert_eq!(level_for(&[("dna", "Nada todavía"), ("mutations", "Nada todavía")]), EntryLevel::AbsoluteZero);
        assert_eq!(level_for(&[("dna", "Lo básico"), ("mutations", "Nada todavía")]), EntryLevel::TheoreticalFoundations, "one rung of knowledge is enough to run the battery");
        assert_eq!(level_for(&[("dna", "Lo básico"), ("mutations", "Lo básico")]), EntryLevel::TheoreticalFoundations);
    }

    #[test]
    fn not_knowing_counts_as_knowing_nothing_not_as_missing_information() {
        assert_eq!(level_for(&[("dna", UNKNOWN_ANSWER), ("mutations", UNKNOWN_ANSWER)]), EntryLevel::AbsoluteZero);
        assert_eq!(level_for(&[("dna", "Lo básico"), ("mutations", UNKNOWN_ANSWER)]), EntryLevel::TheoreticalFoundations);
        assert_eq!(level_for(&[("dna", "Lo manejo bien"), ("mutations", UNKNOWN_ANSWER)]), EntryLevel::TheoreticalFoundations, "2 + 0 averages 1, below the intermediate threshold");
    }

    #[test]
    fn solid_knowledge_across_the_board_is_applied_intermediate() {
        assert_eq!(level_for(&[("dna", "Lo manejo bien"), ("mutations", "Lo manejo bien")]), EntryLevel::AppliedIntermediate);
        // 2 + 1 over two questions is 1.5: the threshold is inclusive.
        assert_eq!(level_for(&[("dna", "Lo manejo bien"), ("mutations", "Lo básico")]), EntryLevel::AppliedIntermediate);
    }

    #[test]
    fn with_no_usable_signal_it_errs_toward_running_the_battery() {
        assert_eq!(level_for(&[]), EntryLevel::TheoreticalFoundations, "never skip the diagnostic on missing information");
    }

    #[test]
    fn a_set_with_follow_ups_round_trips_through_json_under_the_camel_case_name() {
        let set = set_of(vec![prior("dna"), equipment("computer", vec![simple_follow_up("see_screen")])]);
        let json = serde_json::to_value(&set).expect("serialize");
        assert!(json["questions"][1]["followUps"].is_array(), "the UI reads `followUps`: {json}");
        let back: ContextQuestionSet = serde_json::from_value(json).expect("round trip");
        assert_eq!(back, set);
        // A set stored before follow-ups existed still loads.
        let old: ContextQuestionSet =
            serde_json::from_str(r#"{"questions":[{"id":"a","label":"x","kind":"text","purpose":"equipment"}]}"#).expect("old shape");
        assert!(old.questions[0].follow_ups.is_empty());
    }
}
