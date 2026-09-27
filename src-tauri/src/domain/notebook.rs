use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// The 7 pedagogical block types a dynamically-composed class notebook can
/// draw from (3-5 per class — see `GeneratedDynamicNotebook`). No mandatory
/// sequence and no fixed template: `notebook_agent` (and `roadmap_agent`'s
/// embedded first-class generation) choose which blocks to use, how many,
/// and in what order, based on the class's epistemological shape — see
/// `agents::notebook_agent`'s system prompt. Renamed/restructured from the
/// earlier 6-type catalog to fix a confirmed monotony problem (the same 6
/// names in practice collapsing into one repeated mold) — see
/// `application::notebook_service::notebook_grounding_violations`'s
/// anti-repetition check, which is what actually enforces this now instead
/// of just prompt wording.
/// The two mastery dimensions a notebook must demonstrate before it's
/// allowed to close — understanding a concept (a prediction/decision gate)
/// is not the same as being able to apply it (an audit/mission gate), and a
/// class that only ever exercised one of the two hasn't actually finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateCategory {
    Conceptual,
    Practice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicBlockType {
    AnchoredMicroTheory,
    DeclarativeVisualDiagram,
    BranchingScenarioChallenge,
    HeuristicErrorAudit,
    InteractivePredictionGate,
    HandsOnMission,
    MetacognitiveClosure,
}

impl DynamicBlockType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AnchoredMicroTheory => "anchored_micro_theory",
            Self::DeclarativeVisualDiagram => "declarative_visual_diagram",
            Self::BranchingScenarioChallenge => "branching_scenario_challenge",
            Self::HeuristicErrorAudit => "heuristic_error_audit",
            Self::InteractivePredictionGate => "interactive_prediction_gate",
            Self::HandsOnMission => "hands_on_mission",
            Self::MetacognitiveClosure => "metacognitive_closure",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "anchored_micro_theory" => Some(Self::AnchoredMicroTheory),
            "declarative_visual_diagram" => Some(Self::DeclarativeVisualDiagram),
            "branching_scenario_challenge" => Some(Self::BranchingScenarioChallenge),
            "heuristic_error_audit" => Some(Self::HeuristicErrorAudit),
            "interactive_prediction_gate" => Some(Self::InteractivePredictionGate),
            "hands_on_mission" => Some(Self::HandsOnMission),
            "metacognitive_closure" => Some(Self::MetacognitiveClosure),
            _ => None,
        }
    }

    /// Whether this block type is a mastery GATE — the student must submit
    /// a graded response (`GateSubmission`) before the notebook advances
    /// past it. The other 3 types (`anchored_micro_theory`,
    /// `declarative_visual_diagram`, `metacognitive_closure`) are read-only
    /// content: reaching them IS the interaction, no grading needed, and
    /// the next block starts generating in the background immediately.
    pub fn is_gate(self) -> bool {
        matches!(
            self,
            Self::InteractivePredictionGate | Self::BranchingScenarioChallenge | Self::HeuristicErrorAudit | Self::HandsOnMission
        )
    }

    /// Which mastery dimension this gate tests, if any — the closing
    /// `metacognitive_closure` block is only allowed once the student has
    /// PASSED at least one gate of EACH category (see
    /// `notebook_service::grounding`'s closure-eligibility check), not
    /// after any fixed block count. `None` for non-gate types.
    pub fn gate_category(self) -> Option<GateCategory> {
        match self {
            Self::InteractivePredictionGate | Self::BranchingScenarioChallenge => Some(GateCategory::Conceptual),
            Self::HeuristicErrorAudit | Self::HandsOnMission => Some(GateCategory::Practice),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticDimension {
    /// Anclaje físico/cotidiano: fenómeno causal o espacial básico, sin
    /// tecnicismos ni fórmulas.
    Intuition,
    /// Mecánica relacional: causa-efecto entre variables interdependientes.
    Mechanics,
    /// Auditoría de mito/error crítico: escenario límite que revela si el
    /// estudiante cae en el malentendido más frecuente del dominio.
    CriticalCase,
    /// Límite y transferencia: qué ocurre en casos nulos, paralelos o
    /// asintóticos — opcional, la 4ª dimensión.
    Boundary,
}

/// One question in a `DiagnosticBattery` — always multiple choice so the
/// gap it reveals is machine-checkable (`correct_option`), never a free-text
/// question graded by vibes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticQuestion {
    pub dimension: DiagnosticDimension,
    pub prompt: String,
    pub options: Vec<String>,
    pub correct_option: String,
    /// What choosing the WRONG option(s) reveals about the student's mental
    /// model — shown back to them after they answer, framing the miss as a
    /// specific misconception rather than just "incorrecto".
    pub diagnostic_insight: String,
}

/// A calibration battery generated ONCE, alongside the syllabus itself (see
/// `SyllabusExecutionArgs` in `domain::roadmap`) — NOT a notebook block, and
/// never regenerated per class. It belongs to the ROADMAP/course, not to any
/// one class, because its purpose is to let the diagnostic (plus the
/// learner's goal, timeframe and hours) shape both the syllabus and every
/// class generated within it, not to be content a student reads inside a
/// lesson.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticBattery {
    /// Justifies how these questions measure the gap to the course's
    /// target goal — not decoration, ties the battery to the real terminal
    /// objective instead of testing arbitrary trivia.
    pub goal_alignment: String,
    pub questions: Vec<DiagnosticQuestion>,
}

/// The persisted, course-scoped shape: the generated battery plus whatever
/// the student has answered so far (`answers`, keyed by question index as a
/// string) — mutated in place by `save_diagnostic_battery_answers`,
/// re-read by later class generations to personalize on it (see
/// `NotebookService::generate_class_notebook`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticBatteryState {
    #[serde(flatten)]
    pub battery: DiagnosticBattery,
    #[serde(default)]
    pub answers: HashMap<String, String>,
}

impl DiagnosticBatteryState {
    /// `true` once every question has an answer recorded — the signal that
    /// the Diagnostic stage is actually done, not just started (see
    /// `RoadmapService::answer_diagnostic_question`).
    pub fn fully_answered(&self) -> bool {
        (0..self.battery.questions.len()).all(|i| self.answers.contains_key(&i.to_string()))
    }

    /// Reduces this state down to what a class generation (or the syllabus
    /// generation right after the Diagnostic stage) actually needs: whether
    /// it's been answered yet, and which dimensions revealed a gap — never
    /// the full question bank, which the model already saw once and doesn't
    /// need again. Used by both `NotebookService::generate_class_notebook`
    /// (course-scoped, later classes) and `RoadmapService::
    /// answer_diagnostic_question` (session-scoped, right before sealing).
    pub fn profile_summary(&self) -> serde_json::Value {
        if self.answers.is_empty() {
            return serde_json::json!({ "answered": false });
        }
        let weak_points: Vec<serde_json::Value> = self
            .battery
            .questions
            .iter()
            .enumerate()
            .filter_map(|(i, q)| {
                let chosen = self.answers.get(&i.to_string())?;
                if chosen == &q.correct_option {
                    return None;
                }
                Some(serde_json::json!({
                    "dimension": q.dimension,
                    "insight": q.diagnostic_insight,
                }))
            })
            .collect();
        serde_json::json!({ "answered": true, "weakPoints": weak_points })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MermaidChartType {
    Flowchart,
    SequenceDiagram,
    ClassDiagram,
    StateDiagram,
    ErDiagram,
}

/// One shape in a `declarative_svg` visual — a hand-specified drawing
/// primitive, not a canvas/WebGL scene: static, inspectable, and cheap to
/// render as literal SVG markup on the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SvgTag {
    Rect,
    Circle,
    Line,
    Path,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvgElement {
    pub tag: SvgTag,
    /// Raw SVG attribute map (e.g. `{"x": 10, "y": 20, "width": 100}`) —
    /// deliberately untyped: the attribute set is genuinely different per
    /// tag, and re-encoding SVG's own attribute vocabulary as Rust structs
    /// buys nothing the frontend's `<rect {...props} />` spread doesn't
    /// already give for free.
    pub props: serde_json::Value,
    #[serde(default)]
    pub label: Option<String>,
}

/// A semantic cluster of elements within a `declarative_svg` scene — e.g.
/// "extracellular_space" or "protein_channel" in a cell-transport diagram.
/// Grouping is what lets a scientific illustration stay spatially coherent
/// (bilayer, channel, gradient all rendered as one scene) instead of the
/// flat, ungrouped element soup `elements` alone allows.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SvgGroup {
    pub group_id: String,
    /// The group's pedagogical function in the scene (e.g.
    /// "high_concentration_zone", "selective_barrier") — informational, read
    /// by nothing in Rust, but keeps the model honest about why each cluster
    /// of shapes exists rather than emitting decoration.
    pub pedagogical_role: String,
    pub elements: Vec<SvgElement>,
}

/// Static, declarative visual — explicitly NOT a dynamic simulator or
/// executable sandbox (out of scope for now, see the caller's system
/// prompt): one of 3 stable rendering engines, chosen for the concept's
/// shape (process -> `mermaid` flowchart, entities -> `mermaid` class/ER
/// diagram or `declarative_svg` boxes, comparison -> `conceptual_matrix`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "renderEngine")]
pub enum StaticVisualSpec {
    #[serde(rename = "mermaid")]
    Mermaid {
        #[serde(rename = "chartType")]
        chart_type: MermaidChartType,
        /// Must be syntactically valid Mermaid source for `chartType` — the
        /// frontend renders it directly via `mermaid.render`, no server-side
        /// validation, so a malformed diagram fails visibly in the browser
        /// console, not silently.
        code: String,
        caption: String,
    },
    #[serde(rename = "declarative_svg")]
    DeclarativeSvg {
        #[serde(rename = "viewBox", default = "default_view_box")]
        view_box: String,
        /// Flat shapes — for simple diagrams that don't need semantic
        /// clustering. Mutually complementary with `groups`, not exclusive:
        /// a scene may use either or both.
        #[serde(default)]
        elements: Vec<SvgElement>,
        /// Semantically clustered shapes (e.g. one group per anatomical
        /// layer or physical zone) — the required shape for spatially
        /// coherent scientific scenes. See `notebook_agent`'s system prompt
        /// for the layered viewBox convention this is designed around.
        #[serde(default)]
        groups: Vec<SvgGroup>,
        /// What the diagram is teaching in one sentence — e.g. "el soluto se
        /// mueve de mayor a menor concentración". Optional: most diagrams
        /// convey this through `caption` alone.
        #[serde(rename = "pedagogicalFocus", default)]
        pedagogical_focus: Option<String>,
        caption: String,
    },
    #[serde(rename = "conceptual_matrix")]
    ConceptualMatrix {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        #[serde(rename = "contrastFocus")]
        contrast_focus: String,
    },
}

fn default_view_box() -> String {
    "0 0 800 450".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalkthroughStep {
    pub step_number: u32,
    pub target_visual_element: String,
    pub pedagogical_insight: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlawErrorType {
    Syntax,
    ConceptualMisunderstanding,
    StructuralInversion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlawedRepresentation {
    pub context: String,
    pub buggy_snippet_or_diagram: String,
    pub error_type: FlawErrorType,
}

/// One choice branch of a `branching_scenario_challenge` — the consequence
/// is shown to the student immediately after they pick, never hidden or
/// deferred, so the decision's weight is felt right away.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioBranch {
    pub choice: String,
    pub consequence: String,
    /// Exactly one branch per challenge must be `true` — the model's own
    /// judgment of which choice was actually the best one, revealed to the
    /// student as feedback after they pick (never before).
    pub is_optimal: bool,
}

/// One block the model proposes, in the order it should appear — the
/// agent-facing / tool-args shape. Deliberately has NO `id`: like every
/// other backend-owned identifier in this app, block ids are assigned at
/// persistence time (see `notebook_store::NotebookStore::upsert_document_
/// with_blocks`), never invented by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "blockType")]
pub enum GeneratedSectionBlock {
    /// Layered microtheory in exactly 3 parts (🎯📐⚠️) — never one
    /// undifferentiated paragraph — and capped at 160 words combined (see
    /// `notebook_service::notebook_grounding_violations`) so it can never
    /// degrade into flat encyclopedic text.
    #[serde(rename = "anchored_micro_theory", rename_all = "camelCase")]
    AnchoredMicroTheory {
        title: String,
        /// 🎯 Everyday analogy, zero technical terms.
        intuitive_hook: String,
        /// 📐 The formal rule/mechanism, introduced step by step — this is
        /// where technical terminology is allowed to appear.
        system_rule: String,
        /// ⚠️ The specific misconception that typically causes a wrong
        /// answer on an exam or a bug in a project — not a generic warning.
        frequent_error: String,
    },
    /// A standalone static diagram (never embedded inside a theory block) —
    /// labeled connections and an explicit relationship map, walked through
    /// step by step.
    #[serde(rename = "declarative_visual_diagram", rename_all = "camelCase")]
    DeclarativeVisualDiagram { title: String, visual_aid: StaticVisualSpec, guided_walkthrough: Vec<WalkthroughStep> },
    /// Situational decision-making with visible consequences per path —
    /// suited to leadership/strategy/management topics where the content IS
    /// a decision, not a concept or procedure. `visual_aid` is OPTIONAL —
    /// unlike `declarative_visual_diagram` (where a visual IS the block),
    /// here it's only worth including when a diagram genuinely clarifies the
    /// scenario itself (e.g. an org chart, a system state) — never required.
    #[serde(rename = "branching_scenario_challenge", rename_all = "camelCase")]
    BranchingScenarioChallenge {
        scenario: String,
        decision_point: String,
        branches: Vec<ScenarioBranch>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        visual_aid: Option<StaticVisualSpec>,
    },
    /// An artifact with an intentional conceptual/design flaw — the student
    /// plays auditor and must find the root cause, not just spot a typo.
    /// `visual_aid` is OPTIONAL: use it when the flaw is spatial/visual in
    /// nature (a mislabeled diagram, a broken flowchart) rather than
    /// forcing every audit into a text/code snippet.
    #[serde(rename = "heuristic_error_audit", rename_all = "camelCase")]
    HeuristicErrorAudit {
        instruction: String,
        flawed_representation: FlawedRepresentation,
        guiding_questions: Vec<String>,
        model_solution: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        visual_aid: Option<StaticVisualSpec>,
    },
    /// A hypothesis question that GATES the feedback — the student commits
    /// to a prediction before `conceptualFeedbackMap` reveals why each
    /// option was right or wrong. `visual_aid` is OPTIONAL — include it when
    /// the prediction genuinely depends on reading a diagram first (e.g.
    /// "¿qué pasa en este circuito?"), never as decoration.
    #[serde(rename = "interactive_prediction_gate", rename_all = "camelCase")]
    InteractivePredictionGate {
        question: String,
        options: Vec<String>,
        conceptual_feedback_map: HashMap<String, String>,
        /// The answer key — server-only. MUST be stripped by
        /// `notebook_service::grading::redact_answer_key` before any
        /// `content_json` reaches the frontend; grading (`grading::
        /// grade_prediction_gate`) compares the student's `GateSubmission`
        /// against this field directly from the store, never re-derived
        /// from client input.
        correct_option: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        visual_aid: Option<StaticVisualSpec>,
    },
    /// Deliberate practice contextualized with REAL constraints and
    /// observable acceptance criteria — not an open-ended exercise.
    /// `visual_aid` is OPTIONAL: a reference diagram of what the student is
    /// meant to build/reach, when a picture genuinely helps set the target.
    #[serde(rename = "hands_on_mission", rename_all = "camelCase")]
    HandsOnMission {
        challenge_statement: String,
        expected_milestone_artifact: String,
        constraints: Vec<String>,
        scaffolding_hints: Vec<String>,
        evaluation_rubric_summary: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        visual_aid: Option<StaticVisualSpec>,
    },
    /// Reflective contrast between the student's initial mental model and
    /// what the session actually showed them. The closing block of every
    /// notebook (see `notebook_agent`'s system prompt) — `prediction_
    /// comparison` is mandatory precisely because "compare what you
    /// predicted vs. what happened" is the metacognitive traceability
    /// requirement this whole redesign exists to satisfy, not an optional
    /// nicety.
    #[serde(rename = "metacognitive_closure", rename_all = "camelCase")]
    MetacognitiveClosure {
        synthesis_task: String,
        prediction_comparison: PredictionComparison,
        self_evaluation_checklist: Vec<String>,
    },
}

/// The mandatory contrast a `metacognitive_closure` block draws between what
/// the student predicted at their FIRST gate in this notebook
/// (`NotebookDocument::initial_prediction`, captured verbatim, whatever it
/// was — right or wrong) and what the notebook actually demonstrated by the
/// end. `initial_prediction`/`final_result` are threaded in by
/// `notebook_service::render::render_closing_block_input`, not invented by
/// the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PredictionComparison {
    pub initial_prediction: String,
    pub final_result: String,
    pub contrast_narrative: String,
}

impl GeneratedSectionBlock {
    pub fn block_type(&self) -> DynamicBlockType {
        match self {
            Self::AnchoredMicroTheory { .. } => DynamicBlockType::AnchoredMicroTheory,
            Self::DeclarativeVisualDiagram { .. } => DynamicBlockType::DeclarativeVisualDiagram,
            Self::BranchingScenarioChallenge { .. } => DynamicBlockType::BranchingScenarioChallenge,
            Self::HeuristicErrorAudit { .. } => DynamicBlockType::HeuristicErrorAudit,
            Self::InteractivePredictionGate { .. } => DynamicBlockType::InteractivePredictionGate,
            Self::HandsOnMission { .. } => DynamicBlockType::HandsOnMission,
            Self::MetacognitiveClosure { .. } => DynamicBlockType::MetacognitiveClosure,
        }
    }
}

/// The whole dynamically-composed lesson the model proposes for one class —
/// `sections` in the exact display order, 3 to 5 blocks (narrowed from the
/// earlier 3-7 range specifically to make the old fixed-6-block mold
/// impossible to reproduce). No fixed theory/exercise/practice/activity
/// molds: see `notebook_agent`'s system prompt for how the sequence is
/// chosen per class. `classId`/`micromoduleId`/`allocatedHours` are
/// deliberately NOT fields here — like every other backend-owned value in
/// this app, the caller (`NotebookService::generate_class_notebook`/
/// `RoadmapService`'s Gate 4) already knows them from the class row it's
/// generating for, so they're attached to the persisted/returned payload in
/// Rust, never asked of the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedDynamicNotebook {
    pub topic_title: String,
    /// Free-text justification for this specific block combination — why
    /// THESE 3-5 blocks, in THIS order, for THIS topic. Replaces the earlier
    /// closed `PedagogicalProfile` 4-value enum with an actual explanation.
    pub pedagogical_rationale: String,
    pub sections: Vec<GeneratedSectionBlock>,
}

// --- Persisted shapes --------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Course {
    pub id: String,
    pub title: String,
    pub target_goal: String,
    pub total_weeks: u16,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyllabusMilestone {
    pub id: String,
    pub course_id: String,
    pub week_number: u16,
    pub title: String,
    pub deliverable_goal: String,
}

/// One class within a milestone week — now one PER MICROMODULE (see
/// `domain::roadmap::Micromodule`), not one per week: `class_number` is the
/// micromodule's position within its week, `hours` is that micromodule's own
/// bounded (≤5h) allocation, and `title` is composed as "Semana {week}:
/// {micromodule.label}" at import time (see `import_syllabus_into_store`) —
/// never a separate id the model has to invent or echo back.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassRecord {
    pub id: String,
    pub milestone_id: String,
    pub class_number: u16,
    pub title: String,
    pub order_index: u32,
    pub hours: f32,
    /// DERIVED, never stored: whether this class is already "understood and
    /// practiced" (see [`class_is_complete`]) — recomputed on every
    /// `list_classes_for_course` so the path UI can lock the next class
    /// without a second query per node. Defaults to `false` on
    /// deserialization of payloads written before this field existed.
    #[serde(default)]
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotebookStatus {
    Draft,
    Ready,
}

impl NotebookStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Ready => "ready",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(Self::Draft),
            "ready" => Some(Self::Ready),
            _ => None,
        }
    }
}

/// Per-BLOCK gating state — distinct from `NotebookStatus`, which is a
/// whole-DOCUMENT concept (generation succeeded or not). A block is
/// `Ready` the instant it's persisted (nothing to grade — theory/diagram
/// blocks) or awaiting its first submission (gate blocks); `Passed`/
/// `Failed` once `NotebookService::submit_gate_response` grades a
/// submission; `Escalated` once the SAME gate has failed 3 times and a
/// re-approach block was generated instead of a 4th scaffold hint (see
/// `notebook_service::generation::submit_gate_response`) — an escalated
/// block stays locked forever, it is never retried again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockStatus {
    Ready,
    Passed,
    Failed,
    Escalated,
}

impl BlockStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Escalated => "escalated",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ready" => Some(Self::Ready),
            "passed" => Some(Self::Passed),
            "failed" => Some(Self::Failed),
            "escalated" => Some(Self::Escalated),
            _ => None,
        }
    }

    /// Whether the student may move past this block without submitting
    /// anything else. `Ready` means different things for the two block
    /// families — for a non-gate type it means "shown, nothing to grade,
    /// already unlocked"; for a gate type it means "shown, awaiting its
    /// first submission, still locked" — so `block_type` disambiguates.
    /// `Passed`/`Escalated` are always unlocked regardless of type;
    /// `Failed` is always locked (a retry, or escalation, is pending).
    pub fn unlocked(self, block_type: DynamicBlockType) -> bool {
        match self {
            Self::Passed | Self::Escalated => true,
            Self::Failed => false,
            Self::Ready => !block_type.is_gate(),
        }
    }
}

/// What the student submits to a gate — one variant per gate-eligible
/// block type. Deliberately NOT `GeneratedSectionBlock`: the model's
/// answer-key fields (`correct_option`, etc.) never appear here, and this
/// type flows the OPPOSITE direction (frontend -> backend) from
/// `GeneratedSectionBlock` (model -> backend). Tag values match
/// `DynamicBlockType::as_str()` so a submission is self-describing without
/// a separate block-type parameter.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "blockType")]
pub enum GateSubmission {
    #[serde(rename = "interactive_prediction_gate", rename_all = "camelCase")]
    InteractivePredictionGate { selected_option: String },
    #[serde(rename = "branching_scenario_challenge", rename_all = "camelCase")]
    BranchingScenarioChallenge { selected_choice: String },
    /// Free-text root-cause diagnosis, graded by
    /// `notebook_gate_grader` against `HeuristicErrorAudit::model_solution`
    /// — not multiple choice, per the confirmed product decision to keep
    /// this pedagogically richer even though it's non-deterministic.
    #[serde(rename = "heuristic_error_audit", rename_all = "camelCase")]
    HeuristicErrorAudit { diagnosis_text: String },
    /// Free-text solution, graded by `notebook_gate_grader` against
    /// `HandsOnMission::evaluation_rubric_summary`.
    #[serde(rename = "hands_on_mission", rename_all = "camelCase")]
    HandsOnMission { submission_text: String },
}

impl GateSubmission {
    pub fn block_type(&self) -> DynamicBlockType {
        match self {
            Self::InteractivePredictionGate { .. } => DynamicBlockType::InteractivePredictionGate,
            Self::BranchingScenarioChallenge { .. } => DynamicBlockType::BranchingScenarioChallenge,
            Self::HeuristicErrorAudit { .. } => DynamicBlockType::HeuristicErrorAudit,
            Self::HandsOnMission { .. } => DynamicBlockType::HandsOnMission,
        }
    }
}

/// What `NotebookService::submit_gate_response` returns: the graded
/// outcome, plus whatever content the frontend needs to render next without
/// a second round trip.
#[derive(Debug, Clone, Serialize)]
pub struct GateResult {
    pub passed: bool,
    /// The just-graded block, `content_json` already answer-key-redacted.
    pub block: NotebookBlock,
    /// A scaffold hint (Socratic question or worked counterexample) —
    /// present only on a failed attempt 1 or 2, `None` on pass and on the
    /// 3rd (escalation) failure, where `escalation_block` carries the
    /// remediation instead of hint text.
    pub feedback: Option<String>,
    /// Populated only on the 3rd consecutive failure of this block: a
    /// freshly generated, differently-framed block re-approaching the same
    /// concept. The failed block itself is marked `Escalated`, not retried.
    pub escalation_block: Option<NotebookBlock>,
    /// Populated only if the background-buffered next block had already
    /// finished generating by the time this submission was graded — saves
    /// the frontend a `notebook_block://ready` round trip in the common
    /// case where reading+answering took longer than generation.
    pub next_block: Option<NotebookBlock>,
}

/// What `NotebookService::submit_closure_feedback` returns: the verdict on
/// the closing block's free-text reflection. Unlike `GateResult.feedback`
/// (a scaffold hint, only on a failed attempt 1 or 2), `feedback` is ALWAYS
/// student-facing — pass or fail — and `block` carries the closure with its
/// persisted `status`/`last_feedback`. A `passed: true` is what completes
/// the class (see [`class_is_complete`]).
#[derive(Debug, Clone, Serialize)]
pub struct ClosureFeedback {
    pub passed: bool,
    pub feedback: String,
    pub block: NotebookBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookDocument {
    pub id: String,
    pub class_id: String,
    pub title: String,
    pub status: NotebookStatus,
    pub updated_at_ms: i64,
    /// The model's own justification for this notebook's block composition
    /// (`GeneratedDynamicNotebook::pedagogical_rationale`) — tied to the
    /// generated DOCUMENT, not the class bookkeeping row, since it can
    /// change on regeneration. `None` for documents generated before this
    /// field existed.
    #[serde(default)]
    pub pedagogical_rationale: Option<String>,
    /// The literal gate: index into this document's blocks (by
    /// `order_index`) of the furthest block the student has unlocked.
    /// Blocks past this index don't exist client-side — they haven't been
    /// generated yet. Advanced only by
    /// `notebook_service::generation::submit_gate_response` on a pass.
    #[serde(default)]
    pub current_block_index: u32,
    /// The student's own answer text at the FIRST gate they ever answered
    /// in this notebook, captured verbatim (right or wrong) and never
    /// overwritten again. Fed into the closing `metacognitive_closure`
    /// block's `PredictionComparison::initial_prediction`. `None` until
    /// that first gate is answered.
    #[serde(default)]
    pub initial_prediction: Option<String>,
}

/// One persisted block: backend-owned `id`/`order_index` alongside the
/// block's own type-specific payload (`content_json` — the same shape
/// `GeneratedSectionBlock` serializes to, `blockType` tag included, so the
/// frontend's dynamic component registry can read `content_json.blockType`
/// directly without re-deriving it from the `block_type` column).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookBlock {
    pub id: String,
    pub document_id: String,
    pub block_type: DynamicBlockType,
    /// The RAW, unredacted payload as persisted — includes answer-key
    /// fields (`correctOption`, etc.) when this instance came straight
    /// from `NotebookStore`. Every path that returns a `NotebookBlock` to
    /// the frontend (Tauri commands, `NotebookPayload`, `GateResult`) MUST
    /// run it through `notebook_service::grading::redact_answer_key`
    /// first — there is no type-level distinction between a raw and a
    /// redacted instance, so this is an invariant enforced by code
    /// discipline at the service boundary, not the compiler.
    pub content_json: serde_json::Value,
    pub order_index: u32,
    pub status: BlockStatus,
    pub attempt_count: u32,
    /// The most recent scaffold hint / re-approach rationale shown to the
    /// student for this block, if it has ever failed. `None` for blocks
    /// that were passed on the first try or never needed grading.
    pub last_feedback: Option<String>,
}

/// What `generate_class_notebook` / `load_class_notebook` return: a document
/// plus its blocks, already ordered by `order_index`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookPayload {
    pub document: NotebookDocument,
    pub blocks: Vec<NotebookBlock>,
}

/// A class counts as "understood AND practiced" — the state that unlocks the
/// NEXT class in the sequential path — exactly when BOTH hold:
///
/// - **practiced**: every gate block of its notebook is resolved —
///   `Passed`, or `Escalated` (3 failed attempts already earned a
///   re-approach mission; an escalated gate is terminal by design);
/// - **understood**: the metacognitive closure exists, is already revealed
///   (its `order_index` is behind the document cursor), carries a
///   non-blank student reflection, AND was approved by
///   `submit_closure_feedback` (its status is `Passed`) — writing any text
///   is not enough; the closing feedback is the last hurdle.
///
/// No closure yet, an unrevealed one, a blank reflection, a reflection
/// whose feedback never passed, or any gate still awaiting/failed submission
/// all mean NOT complete. Single source of truth: the
/// `start_class_notebook` guard AND `ClassRecord::complete` both derive
/// from this.
pub fn class_is_complete(document: &NotebookDocument, blocks: &[NotebookBlock]) -> bool {
    let Some(closure) = blocks.iter().find(|b| b.block_type == DynamicBlockType::MetacognitiveClosure) else {
        return false;
    };
    if closure.order_index >= document.current_block_index {
        return false;
    }
    let reflection = closure
        .content_json
        .get("studentReflection")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim();
    if reflection.is_empty() {
        return false;
    }
    if closure.status != BlockStatus::Passed {
        return false;
    }
    blocks
        .iter()
        .filter(|b| b.block_type.is_gate())
        .all(|b| matches!(b.status, BlockStatus::Passed | BlockStatus::Escalated))
}

/// One block's edited content, sent back by `save_notebook_state` after the
/// learner interacts with a block. Only `content_json` is mutable from the
/// frontend — `block_type`/`order_index` are fixed at generation time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockUpdate {
    pub id: String,
    pub content_json: serde_json::Value,
}

#[cfg(test)]
mod tests;
