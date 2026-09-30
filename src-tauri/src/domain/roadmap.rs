use serde::{Deserialize, Serialize};

use crate::domain::notebook::{DiagnosticBattery, DiagnosticBatteryState};

/// The 3 phases of the Roadmap & Syllabus Diagnostic Agent's conversation —
/// now purely a DISPLAY concern for the frontend's phase tracker, not a
/// strict multi-turn gate sequence. `submit_diagnostic_assessment` (see
/// [`DiagnosticAssessmentArgs`]) closes Onboarding AND Diagnostic together
/// (there is no separate situational-question gate anymore — see
/// `agents::roadmap_agent`'s "REGLA DE ORO DE AUTONOMÍA"); the session is
/// normally already `Sealed` by the time `phase` reaches `Roadmap`, since
/// `generate_syllabus_execution` is expected to fire in the SAME tool-
/// calling turn as the assessment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoadmapPhase {
    Onboarding,
    Diagnostic,
    Roadmap,
}

impl RoadmapPhase {
    pub fn next(self) -> Option<Self> {
        match self {
            Self::Onboarding => Some(Self::Diagnostic),
            Self::Diagnostic => Some(Self::Roadmap),
            Self::Roadmap => None,
        }
    }
}

/// A CLOSED set the student must pick from explicitly — Gate 1 is not
/// allowed to infer this from vague phrasing (e.g. defaulting to
/// `"novice_zero"`). See `agents::roadmap_agent`'s "INDAGACIÓN DE NIVEL
/// EXPLÍCITA" rule. `AbsoluteZero` is also the ONLY value that skips Gate 2
/// (the situational battery) entirely — see `RoadmapService::apply_capture`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryLevel {
    AbsoluteZero,
    TheoreticalFoundations,
    AppliedIntermediate,
}

impl EntryLevel {
    pub fn label_es(self) -> &'static str {
        match self {
            Self::AbsoluteZero => "Cero absoluto",
            Self::TheoreticalFoundations => "Fundamentos teóricos",
            Self::AppliedIntermediate => "Nivel medio que busca aplicar",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearnerProfileCard {
    pub topic: String,
    pub target_goal: String,
    pub timeframe_weeks: u16,
    pub weekly_commitment_hours: f32,
    pub total_available_hours: f32,
    pub entry_level: EntryLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticSummaryCard {
    pub core_focus: String,
    /// Concrete learning needs, not knowledge-gap labels — always phrased
    /// as "what we'll work on", never as a deficiency.
    pub identified_needs: Vec<String>,
    pub learning_strategy: String,
}

/// Closed taxonomy for WHAT KIND of artifact a `Micromodule::deliverable`
/// actually is. Replaces a denylist-only check (`grounding::
/// VAGUE_DELIVERABLE_PHRASES`) that could only reject phrasing it already
/// knew about, letting anything else equally vague through unnoticed.
/// Naming the shape up front makes "just a vague sentence" structurally
/// impossible for the 5 self-describing variants; `Other` is the deliberate
/// escape hatch for a real deliverable that genuinely doesn't fit any named
/// bucket, so it alone is held to a stricter minimum `description` length
/// (see `grounding::deliverable_violations`) — the one place a vague
/// one-liner could otherwise hide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliverableArtifactType {
    TestsPassing,
    FormalDiagram,
    FunctionalCli,
    DiagnosticMatrix,
    WorkingDemo,
    Other,
}

/// A session's concrete, checkable artifact — see `Micromodule::deliverable`.
/// Distinct from `Milestone::deliverable` (still a plain-text week-rollup
/// sentence, deliberately NOT restructured — see that field's own doc
/// comment) and from `CapstoneProject` (the terminal project, already its
/// own struct).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Deliverable {
    pub artifact_type: DeliverableArtifactType,
    /// The concrete description of THIS artifact (e.g. "circuito simulado de
    /// 2 qubits con histograma analizado") — never a vague verb phrase like
    /// "comprender la teoría" (see `grounding::deliverable_violations`).
    /// Required non-empty for every `artifactType`; when `artifactType` is
    /// `other` (the only bucket that isn't already self-describing) it must
    /// also clear a higher minimum length, so "otro" can't be the whole
    /// answer.
    pub description: String,
}

impl<'de> Deserialize<'de> for Deliverable {
    /// Manual impl (rather than `#[derive]`) so a `RoadmapSession` persisted
    /// before this field became a struct — when `Micromodule::deliverable`
    /// was still a plain `String` — keeps deserializing instead of breaking
    /// resumption of in-flight roadmap creation. A bare legacy string maps to
    /// `artifactType: Other` (the closed taxonomy's own escape hatch) with
    /// that string as `description`.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Legacy(String),
            #[serde(rename_all = "camelCase")]
            Structured {
                artifact_type: DeliverableArtifactType,
                description: String,
            },
        }

        match Repr::deserialize(deserializer)? {
            Repr::Legacy(description) => Ok(Deliverable {
                artifact_type: DeliverableArtifactType::Other,
                description,
            }),
            Repr::Structured {
                artifact_type,
                description,
            } => Ok(Deliverable {
                artifact_type,
                description,
            }),
        }
    }
}

/// One of the week's exactly-2 study sessions — the anti-monolith unit.
/// Never more than 4 hours (see `syllabus_violations`'s `hours` check): a
/// week's `paceHoursPerWeek` is always split into EXACTLY two homogeneous
/// sessions (e.g. 3.5h/3.5h, or 4h/3h — never one 5h block against a lone
/// 2h one), per Backward Design / Constructive Alignment (Wiggins & McTighe;
/// Biggs) — each session ends in ITS OWN authentic, verifiable artifact, not
/// a shared week-level "understand the theory".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Micromodule {
    /// The session's title, carrying its day range — e.g. "Días 1-2 —
    /// Recorrer el árbol sin clonar nodos". Student-facing: never a raw
    /// internal/methodological term (see `syllabus_violations`'s jargon
    /// check).
    pub label: String,
    pub hours: f32,
    /// A concrete, checkable artifact THIS session produces — a typed
    /// `{artifactType, description}` pair (see `Deliverable`), not free text:
    /// a denylist alone let anything not already on the list through
    /// unchecked. Never a vague verb phrase like "comprender la teoría" (see
    /// `grounding::deliverable_violations`).
    pub deliverable: Deliverable,
    /// The central concepts and cause-effect relationships this session
    /// explores, landed in the real friction it resolves (e.g. "por qué
    /// `iter_mut` evita clonar la estructura completa" instead of an
    /// abstract definition). Distinct from `objective` (the capability
    /// gained) and from `deliverable` (the artifact handed in). Required for
    /// every NEW proposal (see `syllabus_violations`); nullable only so
    /// sessions sealed before this field existed keep deserializing.
    #[serde(default)]
    pub focus: Option<String>,
    /// What the student will be ABLE TO DO once this session is done — one
    /// observable sentence in the infinitive ("Explicar por qué el ciclo
    /// arranca con acetil-CoA"), answering "¿qué me llevo de esta sesión?"
    /// where `deliverable` answers "¿qué entrego?". Required for every NEW
    /// proposal (see `syllabus_violations`); nullable only so sessions
    /// sealed before this field existed keep deserializing (renders as
    /// "no disponible" instead of breaking the plan).
    #[serde(default)]
    pub objective: Option<String>,
    /// 3-4 entries from the fixed methodological catalog (see
    /// `agents::roadmap_agent`'s block catalog and `syllabus_violations`'s
    /// membership check): interactive_visual_anchor | socratic_prediction |
    /// error_audit_challenge | hands_on_mission | metacognitive_closure.
    /// PURE BACKEND METADATA for `notebook_generator` to orchestrate the
    /// class's blocks — the student never sees these names; the frontend
    /// never renders this field.
    pub interactive_blocks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Milestone {
    pub week: u16,
    pub title: String,
    /// Week-level rollup/summary — kept (rather than derived) because
    /// `notebook_service::seal_session`'s `create_milestone` call and the
    /// UI's week header both want a single sentence, not a module list.
    pub deliverable: String,
    /// Backward Design's weekly goal: the real problem/friction this week
    /// resolves and the operative capability it unlocks — never a vague
    /// syllabus-speak line ("familiarizarse con X"). Shown to the student as
    /// the week's "Meta semanal". Required for every NEW proposal (see
    /// `syllabus_violations`); nullable only so sessions sealed before this
    /// field existed keep deserializing.
    #[serde(default)]
    pub weekly_goal: Option<String>,
    /// The week's real decomposition — EXACTLY 2 sessions, homogeneous in
    /// hours (see `syllabus_violations`), replacing what used to be a single
    /// flat `interactive_blocks` list on the week itself.
    pub micromodules: Vec<Micromodule>,
}

/// The terminal transfer project the whole roadmap builds toward, per
/// Backward Design (Wiggins & McTighe) — distinct from the last week's
/// `Milestone`, which is still just that week's rollup. This is the single
/// authentic artifact that certifies the course's `targetGoal` was actually
/// reached, not merely covered week by week. Required on every NEW proposal
/// (see `syllabus_violations`'s capstone check). `Default` exists ONLY so
/// `RoadmapSyllabusPackage::capstone_project` can carry `#[serde(default)]`
/// for sessions sealed before this field existed — mirrors the nullable
/// back-compat pattern used by `Milestone::weekly_goal`/`Micromodule::focus`/
/// `Micromodule::objective`, just applied to a whole nested struct instead of
/// an `Option<String>`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapstoneProject {
    /// Short, student-facing name for the terminal project.
    pub title: String,
    /// The real transfer task the student performs at the end — a concrete
    /// scenario/problem that integrates the course's capabilities, never a
    /// bare topic list ("proyecto final sobre grafos").
    pub description: String,
    /// The tangible artifact(s) that certify closure — what gets handed in
    /// or demonstrated (e.g. "repositorio con la app funcionando + demo en
    /// video de 3 min"), never a vague "dominio del tema".
    pub verifiable_evidence: String,
}

/// The syllabus exactly as the model must emit it — no ids or timestamps,
/// those are backend bookkeeping the model should never be asked to invent.
/// See [`SealedRoadmap`] for the persisted superset.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadmapSyllabusPackage {
    pub course_title: String,
    pub total_weeks: u16,
    pub pace_hours_per_week: f32,
    pub milestones: Vec<Milestone>,
    /// The terminal competency/transfer project — required so the roadmap
    /// has an explicit, verifiable closing deliverable distinct from the
    /// last milestone (see `CapstoneProject`). Every NEW proposal must supply
    /// it (enforced by the tool's JSON schema `required` list and by
    /// `syllabus_violations`'s capstone check); `#[serde(default)]` only
    /// keeps sessions sealed before this field existed deserializing, same
    /// as `Milestone::weekly_goal`/`Micromodule::focus`/`Micromodule::
    /// objective` do for their own back-compat gap.
    #[serde(default)]
    pub capstone_project: CapstoneProject,
}

/// Args for the `submit_diagnostic_assessment` tool call (see
/// `tools::roadmap_tools::SubmitDiagnosticAssessmentTool`) — Gate 1's ONLY
/// job is capturing facts (topic, goal, timeframe, hours, self-declared
/// level). Deliberately omits `totalAvailableHours` — the model doing that
/// arithmetic was a recurring source of grounding failures; it's computed in
/// Rust instead (see `build_profile_card`). Deliberately does NOT carry
/// `coreFocus`/`identifiedNeeds`/`learningStrategy` anymore either — asking
/// the model to write the pedagogical approach HERE, before any diagnosis
/// (Gate 2's battery, or Gate 3's consolidation), was the exact "inversión
/// lógica" this gate was redesigned to fix. Those now come out of
/// `propose_syllabus_plan` at Gate 3, once real gaps are known.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticAssessmentArgs {
    pub topic: String,
    pub target_goal: String,
    pub entry_level: EntryLevel,
    pub timeframe_weeks: u16,
    pub weekly_commitment_hours: f32,
}

/// Args for the `propose_syllabus_plan` tool call (Gate 3a) — a PROPOSAL,
/// not a commit: no course is created and no first-class notebook is
/// generated yet. `core_focus`/`identified_needs`/`learning_strategy` are
/// flat here (not nested) so the model doesn't have to get an extra layer of
/// JSON nesting right; Rust assembles them into a `DiagnosticSummaryCard`.
/// See `tools::roadmap_tools::ProposeSyllabusPlanTool`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposeSyllabusPlanArgs {
    pub core_focus: String,
    pub identified_needs: Vec<String>,
    pub learning_strategy: String,
    pub syllabus: RoadmapSyllabusPackage,
    /// The single closing validation question shown to the student (e.g.
    /// "¿Te parece adecuada esta distribución o deseas ajustar el ritmo?").
    pub closing_question: String,
}

/// What Gate 3a actually stores while awaiting the student's confirmation —
/// see `RoadmapSession::proposed_plan`. `None` once Gate 3b (`confirm_
/// syllabus_plan`) seals the session; the syllabus/diagnostic summary then
/// live on `SealedRoadmap` instead.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedPlan {
    pub diagnostic_summary: DiagnosticSummaryCard,
    pub syllabus: RoadmapSyllabusPackage,
    pub closing_question: String,
}

/// Args for the `confirm_syllabus_plan` tool call (Gate 3b, renamed from
/// `generate_syllabus_execution`) — pure confirmation, no payload. The
/// syllabus itself is NOT re-emitted here: it was already agreed on at Gate
/// 3a and is sourced from `RoadmapSession::proposed_plan` — resending a big
/// payload the model could get wrong again right before persisting defeats
/// the point of a confirm step. Used to also carry `first_class_notebook`
/// (the first class's whole notebook, generated inline at seal time); that
/// field is RETIRED — every class, including the first, now gets its
/// notebook lazily via `notebook_service::generation::start_class_notebook`
/// the first time the student actually opens it, so `roadmap_agent` has
/// nothing left to generate at this gate beyond the confirmation itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmSyllabusPlanArgs {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Sealed,
}

/// The confirmed syllabus plus backend-generated bookkeeping — the
/// persisted, learner-facing "your plan" card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealedRoadmap {
    pub schema_version: String,
    pub package_id: String,
    pub session_id: String,
    pub generated_at_ms: u64,
    pub learner_profile: LearnerProfileCard,
    pub diagnostic_summary: DiagnosticSummaryCard,
    pub syllabus: RoadmapSyllabusPackage,
    /// The calibration battery generated alongside this plan — `None` for
    /// the `absolute_zero` path, where Gate 2 is skipped entirely by design.
    /// The live, mutable copy (with the student's answers) lives on the
    /// imported course row instead (see `NotebookStore::
    /// get_course_diagnostic_battery`); this is just the as-generated
    /// snapshot for record-keeping.
    pub diagnostic_battery: Option<DiagnosticBattery>,
}

/// Persisted, cross-turn state for one roadmap diagnostic conversation. The
/// backend orchestrator itself is stateless per call (see
/// [`crate::orchestration::Orchestrator::run_diagnostic_agent`]); this
/// struct is what carries continuity between turns, loaded/saved by
/// [`crate::application::RoadmapService`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapSession {
    pub session_id: String,
    pub phase: RoadmapPhase,
    pub status: SessionStatus,
    /// Turns spent gathering info (no tool call yet). Purely a hard
    /// ceiling against a model that never acts — see
    /// `RoadmapService::MAX_GATHER_TURNS`. Once ANY tool is called, this
    /// stops mattering for that session.
    pub turn_count_in_phase: u32,
    /// Free-form fields accumulated across turns before enough is known to
    /// call `submit_diagnostic_assessment` — re-sent as context so the
    /// (stateless) model remembers partial answers, and used to
    /// force-synthesize a profile if the turn cap is exhausted first.
    #[serde(default = "empty_object")]
    pub draft: serde_json::Value,
    pub learner_profile_card: Option<LearnerProfileCard>,
    pub diagnostic_summary_card: Option<DiagnosticSummaryCard>,
    pub roadmap_package: Option<SealedRoadmap>,
    /// Learner-chosen label for session lists. `None` = derived title.
    /// `#[serde(default)]` keeps pre-title snapshot files loadable.
    #[serde(default)]
    pub custom_title: Option<String>,
    /// The calibration battery + in-progress answers while the session is
    /// in `RoadmapPhase::Diagnostic` — set the moment
    /// `present_diagnostic_battery` is called, cleared once
    /// `answer_diagnostic_question`/`skip_diagnostic_battery` finishes the
    /// stage and hands the (already-answered) battery to the new course.
    /// No course exists yet at this point, hence living on the session
    /// instead of `NotebookStore::set_course_diagnostic_battery`.
    #[serde(default)]
    pub pending_diagnostic_battery: Option<DiagnosticBatteryState>,
    /// Set the moment `propose_syllabus_plan` (Gate 3a) succeeds — the
    /// diagnostic summary + syllabus awaiting the student's free-text
    /// confirmation. Cleared once `confirm_syllabus_plan` (Gate 3b) seals
    /// the session; nothing is persisted into SQLite until then.
    #[serde(default)]
    pub proposed_plan: Option<ProposedPlan>,
    /// Set once `confirm_syllabus_plan` seals the session — the
    /// SQLite-backed course this plan was imported into, so the frontend
    /// can list its classes without a separate import step.
    #[serde(default)]
    pub imported_course_id: Option<String>,
    /// The first class's id within `imported_course_id` — its notebook is
    /// already generated by the time the session seals, so the frontend
    /// jumps straight there.
    #[serde(default)]
    pub first_class_id: Option<String>,
    /// Set when the most recent tool call was structurally valid but failed
    /// CONTENT grounding — e.g. `totalWeeks` not matching the assessment's
    /// `timeframeWeeks`. Fed back into the next turn's directives and
    /// cleared the moment the session actually seals.
    #[serde(default)]
    pub last_rejection_reasons: Vec<String>,
    /// Consecutive real (user-facing) turns whose tool call(s) failed
    /// grounding even after the in-turn retry — reset to 0 the moment any
    /// turn succeeds. Unlike `turn_count_in_phase` (which freezes forever
    /// once a battery exists — Gate 3 is meant to be paced by the student,
    /// not capped), this keeps watching AFTER that point too: a model stuck
    /// re-presenting an already-answered battery, or never managing a valid
    /// `propose_syllabus_plan`/`confirm_syllabus_plan`, is just as real a
    /// stuck-forever risk as failing Gate 1 — see
    /// `RoadmapService::MAX_CONSECUTIVE_GROUNDING_FAILURES`.
    #[serde(default)]
    pub consecutive_grounding_failures: u32,
    /// Full conversation log (user + assistant turns), persisted with the
    /// session so reopening from the drawer restores the chat. `#[serde(default)]`
    /// keeps pre-log snapshots loadable. Trimmed defensively — see
    /// `MAX_LOG_ENTRIES`.
    #[serde(default)]
    pub messages: Vec<ChatTurn>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

/// One saved conversation turn. `role` is `"user"` or `"assistant"` — the
/// same vocabulary the frontend's `ChatMessage` uses, so restoring is a
/// straight mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTurn {
    pub role: String,
    pub text: String,
    pub at_ms: u64,
}

fn empty_object() -> serde_json::Value {
    serde_json::Value::Object(Default::default())
}

impl RoadmapSession {
    pub fn new(session_id: String, now_ms: u64) -> Self {
        Self {
            session_id,
            phase: RoadmapPhase::Onboarding,
            status: SessionStatus::Active,
            turn_count_in_phase: 0,
            draft: empty_object(),
            learner_profile_card: None,
            diagnostic_summary_card: None,
            roadmap_package: None,
            custom_title: None,
            pending_diagnostic_battery: None,
            proposed_plan: None,
            imported_course_id: None,
            first_class_id: None,
            last_rejection_reasons: Vec::new(),
            consecutive_grounding_failures: 0,
            messages: Vec::new(),
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
        }
    }

    /// Shallow-merges `patch`'s top-level keys into the draft bucket.
    /// Non-object patches are ignored (defensive: a malformed patch must
    /// never corrupt already-confirmed state).
    pub fn merge_patch(&mut self, patch: serde_json::Value) {
        let serde_json::Value::Object(patch_map) = patch else { return };
        if !self.draft.is_object() {
            self.draft = empty_object();
        }
        let obj = self.draft.as_object_mut().expect("just ensured object");
        for (k, v) in patch_map {
            obj.insert(k, v);
        }
    }

    /// Human-readable title for session lists: learner-chosen label first,
    /// then confirmed profile topic, then whatever `topic` the current
    /// gate's draft holds, else a fallback (ids are bookkeeping, not display
    /// strings).
    pub fn display_title(&self) -> String {
        if let Some(custom) = &self.custom_title {
            if !custom.trim().is_empty() {
                return custom.clone();
            }
        }
        if let Some(card) = &self.learner_profile_card {
            if !card.topic.trim().is_empty() {
                return card.topic.clone();
            }
        }
        if let Some(topic) = self.draft.get("topic").and_then(|v| v.as_str()) {
            if !topic.trim().is_empty() {
                return topic.to_string();
            }
        }
        "Sesión sin título".to_string()
    }

    pub fn summary(&self) -> RoadmapSessionSummary {
        RoadmapSessionSummary {
            session_id: self.session_id.clone(),
            phase: self.phase,
            status: self.status,
            title: self.display_title(),
            created_at_ms: self.created_at_ms,
            updated_at_ms: self.updated_at_ms,
        }
    }
}

/// Lightweight row for the saved-sessions list — everything the drawer
/// needs without shipping full drafts and cards for every session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapSessionSummary {
    pub session_id: String,
    pub phase: RoadmapPhase,
    pub status: SessionStatus,
    pub title: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_patch_upserts_fields_into_draft() {
        let mut session = RoadmapSession::new("s1".to_string(), 0);
        session.merge_patch(serde_json::json!({"topic": "Rust CLI", "weeklyCommitmentHours": 5}));
        session.merge_patch(serde_json::json!({"weeklyCommitmentHours": 8}));

        assert_eq!(session.draft["topic"], "Rust CLI");
        assert_eq!(session.draft["weeklyCommitmentHours"], 8);
    }

    #[test]
    fn deliverable_deserializes_a_pre_upgrade_plain_string_as_other() {
        // Sessions sealed before `Micromodule::deliverable` became a struct
        // persisted it as a bare string — must keep loading.
        let legacy: Deliverable = serde_json::from_value(serde_json::json!("CLI con 3 comandos funcionando")).unwrap();
        assert_eq!(legacy.artifact_type, DeliverableArtifactType::Other);
        assert_eq!(legacy.description, "CLI con 3 comandos funcionando");

        let current: Deliverable = serde_json::from_value(serde_json::json!({
            "artifactType": "functional_cli",
            "description": "CLI con 3 comandos funcionando",
        }))
        .unwrap();
        assert_eq!(current.artifact_type, DeliverableArtifactType::FunctionalCli);
    }

    #[test]
    fn gate_sequence_ends_at_roadmap() {
        assert_eq!(RoadmapPhase::Onboarding.next(), Some(RoadmapPhase::Diagnostic));
        assert_eq!(RoadmapPhase::Diagnostic.next(), Some(RoadmapPhase::Roadmap));
        assert_eq!(RoadmapPhase::Roadmap.next(), None);
    }

    #[test]
    fn summary_title_prefers_card_then_draft_then_fallback() {
        let mut session = RoadmapSession::new("s1".to_string(), 0);
        assert_eq!(session.summary().title, "Sesión sin título");

        session.merge_patch(serde_json::json!({"topic": "Borrador"}));
        assert_eq!(session.summary().title, "Borrador");

        session.learner_profile_card = Some(LearnerProfileCard {
            topic: "Tema confirmado".to_string(),
            target_goal: "meta".to_string(),
            timeframe_weeks: 4,
            weekly_commitment_hours: 5.0,
            total_available_hours: 20.0,
            entry_level: EntryLevel::TheoreticalFoundations,
        });
        let summary = session.summary();
        assert_eq!(summary.title, "Tema confirmado");
        assert_eq!(summary.session_id, "s1");
        assert_eq!(summary.phase, RoadmapPhase::Onboarding);
    }

    #[test]
    fn diagnostic_assessment_args_parse_camel_case_wire_shape() {
        let json = serde_json::json!({
            "topic": "Ciclo de Krebs",
            "targetGoal": "Aprobar el examen",
            "entryLevel": "absolute_zero",
            "timeframeWeeks": 4,
            "weeklyCommitmentHours": 30,
        });
        let parsed: DiagnosticAssessmentArgs = serde_json::from_value(json).expect("parses");
        assert_eq!(parsed.entry_level, EntryLevel::AbsoluteZero);
        assert_eq!(parsed.timeframe_weeks, 4);
    }

    #[test]
    fn propose_syllabus_plan_args_parse_camel_case_wire_shape() {
        let json = serde_json::json!({
            "coreFocus": "Lo esencial",
            "identifiedNeeds": ["Entender el flujo"],
            "learningStrategy": "Guiado",
            "syllabus": {
                "courseTitle": "Plan",
                "totalWeeks": 1,
                "paceHoursPerWeek": 3.0,
                "milestones": [{
                    "week": 1,
                    "title": "Semana 1",
                    "deliverable": "Entregable",
                    "micromodules": [{
                        "label": "Módulo 1",
                        "hours": 3.0,
                        "deliverable": {
                            "artifactType": "functional_cli",
                            "description": "Artefacto verificable"
                        },
                        "interactiveBlocks": ["socratic_prediction", "hands_on_mission", "metacognitive_closure"]
                    }]
                }],
                "capstoneProject": {
                    "title": "Proyecto terminal",
                    "description": "Integrar lo aprendido en un escenario real de transferencia",
                    "verifiableEvidence": "Repositorio con la app corriendo + demo grabada"
                }
            },
            "closingQuestion": "¿Te parece adecuada esta distribución?"
        });
        let parsed: ProposeSyllabusPlanArgs = serde_json::from_value(json).expect("parses");
        assert_eq!(parsed.syllabus.course_title, "Plan");
    }
}
