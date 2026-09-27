use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use rig_agent::tool::{Tool, ToolContext};
use serde::Serialize;

use crate::domain::notebook::DiagnosticBattery;
use crate::domain::roadmap::{ConfirmSyllabusPlanArgs, DiagnosticAssessmentArgs, ProposeSyllabusPlanArgs};
use crate::tools::notebook_tools::diagnostic_battery_json_schema;

/// What the 4 roadmap tools write to as they're called — read back by
/// `RoadmapService::apply_capture` once a tool-calling turn completes.
/// `submit_diagnostic_assessment` and `present_diagnostic_battery` are
/// expected to chain in the SAME turn (both derivable from context already
/// known, no new information needed from the student — UNLESS entryLevel is
/// absolute_zero, in which case the battery is skipped and
/// `propose_syllabus_plan` chains instead). `propose_syllabus_plan` and
/// `confirm_syllabus_plan` are each their OWN turn, separated by the
/// student's real answers/confirmation — never expected alongside the first
/// two. See `agents::roadmap_agent`'s 4-gate flow (Capture -> Battery ->
/// Propose -> Confirm).
#[derive(Debug, Default, Clone)]
pub struct RoadmapCapture {
    pub assessment: Option<DiagnosticAssessmentArgs>,
    pub diagnostic_battery: Option<DiagnosticBattery>,
    pub propose_syllabus_plan: Option<ProposeSyllabusPlanArgs>,
    pub confirm_syllabus_plan: Option<ConfirmSyllabusPlanArgs>,
}

pub type SharedRoadmapCapture = Arc<Mutex<RoadmapCapture>>;

/// Which subset of the roadmap agent's 4 tools a single execution turn
/// registers. Gate 1 now runs through a dedicated `diagnostic_analyst` agent
/// (scope `Assessment` only), and the battery/propose hand-offs run as
/// separate follow-up turns (scopes `Battery`/`Propose`) instead of one
/// chained mega-turn — `All` is the full 4-tool set the main
/// `roadmap_syllabus_diagnostic` agent uses on a normal chat turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticToolScope {
    All,
    Assessment,
    Battery,
    Propose,
}

impl DiagnosticToolScope {
    pub fn allows_assessment(self) -> bool {
        matches!(self, Self::All | Self::Assessment)
    }

    pub fn allows_battery(self) -> bool {
        matches!(self, Self::All | Self::Battery)
    }

    pub fn allows_propose(self) -> bool {
        matches!(self, Self::All | Self::Propose)
    }

    pub fn allows_confirm(self) -> bool {
        matches!(self, Self::All)
    }
}

#[derive(Debug, Serialize)]
pub struct SavedAck {
    pub saved: bool,
}

/// Gate 1 — CAPTURE. Closes the instant enough is known: topic, goal,
/// timeframe, weekly hours, and an EXPLICIT self-declared entry level (one
/// of the 3 closed options — never inferred). No confirmation, no separate
/// situational-question turn, and NO pedagogical inference here (no
/// coreFocus/identifiedNeeds/learningStrategy): see the agent's system
/// prompt.
pub struct SubmitDiagnosticAssessmentTool(pub SharedRoadmapCapture);

impl Tool for SubmitDiagnosticAssessmentTool {
    const NAME: &'static str = "submit_diagnostic_assessment";
    type Args = DiagnosticAssessmentArgs;
    type Output = SavedAck;
    type Error = Infallible;

    fn description(&self) -> String {
        "Guarda el diagnóstico del estudiante en cuanto se conocen tema, meta, semanas \
         disponibles, horas semanales, Y el nivel de entrada que el estudiante declaró \
         explícitamente (nunca lo asumas). Llámala de inmediato, sin pedir confirmación."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "topic": {"type": "string"},
                "targetGoal": {"type": "string"},
                "entryLevel": {
                    "type": "string",
                    "enum": ["absolute_zero", "theoretical_foundations", "applied_intermediate"],
                    "description": "absolute_zero = \"no sé nada / parto de cero\"; theoretical_foundations = ya tiene bases teóricas; applied_intermediate = nivel medio que ya busca aplicar. SIEMPRE el valor que el estudiante declaró explícitamente, nunca una suposición tuya."
                },
                "timeframeWeeks": {"type": "integer"},
                "weeklyCommitmentHours": {"type": "number"}
            },
            "required": ["topic", "targetGoal", "entryLevel", "timeframeWeeks", "weeklyCommitmentHours"]
        })
    }

    async fn call(&self, _context: &mut ToolContext, args: Self::Args) -> Result<Self::Output, Self::Error> {
        crate::tools::emit_tool_event(Self::NAME);
        self.0.lock().unwrap_or_else(|e| e.into_inner()).assessment = Some(args);
        Ok(SavedAck { saved: true })
    }
}

/// Gate 2 — SITUATIONAL BATTERY. Generates the calibration battery — the
/// ROADMAP's own artifact, never a notebook block. Call this in the SAME
/// turn as `submit_diagnostic_assessment`, UNLESS entryLevel is
/// absolute_zero (then skip this tool entirely and call
/// `propose_syllabus_plan` directly). After calling this, STOP: do not call
/// `propose_syllabus_plan` in this turn or any turn after it — the student
/// answers these questions outside the chat, and the plan is only proposed
/// once that's done (see `RoadmapService::answer_diagnostic_question`).
pub struct PresentDiagnosticBatteryTool(pub SharedRoadmapCapture);

impl Tool for PresentDiagnosticBatteryTool {
    const NAME: &'static str = "present_diagnostic_battery";
    type Args = DiagnosticBattery;
    type Output = SavedAck;
    type Error = Infallible;

    fn description(&self) -> String {
        "Genera la batería situacional de 3 a 4 preguntas (una sola vez, para todo el plan — \
         nunca dentro de una clase), con simetría psicométrica estricta entre opciones y \
         distractores basados en malentendidos reales — nunca opciones absurdas ni la correcta \
         más larga o explicada que las demás. Llámala en el mismo turno que \
         submit_diagnostic_assessment, SALVO que entryLevel sea absolute_zero (en ese caso \
         omítela). Después de llamarla, NO llames propose_syllabus_plan todavía: el estudiante \
         debe responder esta batería primero."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        diagnostic_battery_json_schema()
    }

    async fn call(&self, _context: &mut ToolContext, args: Self::Args) -> Result<Self::Output, Self::Error> {
        crate::tools::emit_tool_event(Self::NAME);
        self.0.lock().unwrap_or_else(|e| e.into_inner()).diagnostic_battery = Some(args);
        Ok(SavedAck { saved: true })
    }
}

fn syllabus_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "courseTitle": {"type": "string"},
            "totalWeeks": {"type": "integer"},
            "paceHoursPerWeek": {"type": "number"},
            "milestones": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "week": {"type": "integer"},
                        "title": {"type": "string"},
                        "deliverable": {
                            "type": "string",
                            "description": "Resumen de UNA frase del hito de la semana — el detalle real va en micromodules."
                        },
                        "micromodules": {
                            "type": "array",
                            "minItems": 1,
                            "maxItems": 3,
                            "description": "Desglose obligatorio anti-monolito: ningún módulo supera 5 horas — si la semana necesita más, repártela en 2-3 módulos. La suma de horas debe igualar paceHoursPerWeek.",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "label": {"type": "string", "description": "p. ej. \"Módulo 1: Días 1-2\""},
                                    "hours": {"type": "number", "maximum": 5},
                                    "deliverable": {
                                        "type": "string",
                                        "description": "Artefacto concreto y verificable de ESTE módulo (nunca \"comprender la teoría\" o \"leer sobre el tema\")."
                                    },
                                    "interactiveBlocks": {
                                        "type": "array",
                                        "minItems": 3,
                                        "maxItems": 4,
                                        "items": {
                                            "type": "string",
                                            "enum": [
                                                "interactive_visual_anchor",
                                                "socratic_prediction",
                                                "error_audit_challenge",
                                                "hands_on_mission",
                                                "metacognitive_closure"
                                            ]
                                        }
                                    }
                                },
                                "required": ["label", "hours", "deliverable", "interactiveBlocks"]
                            }
                        }
                    },
                    "required": ["week", "title", "deliverable", "micromodules"]
                }
            }
        },
        "required": ["courseTitle", "totalWeeks", "paceHoursPerWeek", "milestones"]
    })
}

/// Gate 3a — PROPOSE. Consolidates the diagnostic (from the battery's
/// results, or directly from the declared level if it was skipped) and
/// proposes the syllabus — a PROPOSAL only, nothing is persisted and no
/// first-class notebook is generated yet. Call this the instant Gate 2
/// finishes (battery fully answered, OR entryLevel was absolute_zero and
/// the battery was skipped). End your accompanying text with the
/// `closingQuestion` — do not call `confirm_syllabus_plan` in this same
/// turn; that only happens after the student replies.
pub struct ProposeSyllabusPlanTool(pub SharedRoadmapCapture);

impl Tool for ProposeSyllabusPlanTool {
    const NAME: &'static str = "propose_syllabus_plan";
    type Args = ProposeSyllabusPlanArgs;
    type Output = SavedAck;
    type Error = Infallible;

    fn description(&self) -> String {
        "Propone (sin persistir nada todavía) el diagnóstico consolidado y el temario completo, \
         calibrados por los resultados reales del diagnóstico (batería respondida, o el nivel \
         absolute_zero declarado si la batería se omitió). Termina con closingQuestion. NO genera \
         el notebook de la primera clase ni guarda el curso — eso ocurre solo si el estudiante \
         confirma (confirm_syllabus_plan)."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "coreFocus": {"type": "string"},
                "identifiedNeeds": {"type": "array", "items": {"type": "string"}},
                "learningStrategy": {"type": "string"},
                "syllabus": syllabus_json_schema(),
                "closingQuestion": {"type": "string", "description": "p. ej. \"¿Te parece adecuada esta distribución o deseas ajustar el ritmo?\""}
            },
            "required": ["coreFocus", "identifiedNeeds", "learningStrategy", "syllabus", "closingQuestion"]
        })
    }

    async fn call(&self, _context: &mut ToolContext, args: Self::Args) -> Result<Self::Output, Self::Error> {
        crate::tools::emit_tool_event(Self::NAME);
        self.0.lock().unwrap_or_else(|e| e.into_inner()).propose_syllabus_plan = Some(args);
        Ok(SavedAck { saved: true })
    }
}

/// Gate 3b — CONFIRM. Persists the course with the temario already proposed
/// and confirmed — no notebook content is generated here anymore (every
/// class, including the first, now gets its notebook lazily the first time
/// the student opens it — see `notebook_service::generation::
/// start_class_notebook`). Call this ONLY when the student's reply to the
/// closing question is an affirmative confirmation. If instead they ask for
/// changes, call `propose_syllabus_plan` again with a revised syllabus —
/// never this tool.
pub struct ConfirmSyllabusPlanTool(pub SharedRoadmapCapture);

impl Tool for ConfirmSyllabusPlanTool {
    const NAME: &'static str = "confirm_syllabus_plan";
    type Args = ConfirmSyllabusPlanArgs;
    type Output = SavedAck;
    type Error = Infallible;

    fn description(&self) -> String {
        "Confirma el plan propuesto y persiste el curso con el temario ya acordado. Llámala SOLO \
         cuando el estudiante confirmó el plan propuesto — nunca antes. No lleva parámetros."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn call(&self, _context: &mut ToolContext, args: Self::Args) -> Result<Self::Output, Self::Error> {
        crate::tools::emit_tool_event(Self::NAME);
        self.0.lock().unwrap_or_else(|e| e.into_inner()).confirm_syllabus_plan = Some(args);
        Ok(SavedAck { saved: true })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::notebook::{DiagnosticBattery, DiagnosticDimension, DiagnosticQuestion};
    use crate::domain::roadmap::{EntryLevel, Micromodule, Milestone, RoadmapSyllabusPackage};

    fn sample_assessment() -> DiagnosticAssessmentArgs {
        DiagnosticAssessmentArgs {
            topic: "Ciclo de Krebs".to_string(),
            target_goal: "Aprobar el examen".to_string(),
            entry_level: EntryLevel::AbsoluteZero,
            timeframe_weeks: 4,
            weekly_commitment_hours: 3.0,
        }
    }

    #[tokio::test]
    async fn submit_diagnostic_assessment_writes_into_the_shared_capture() {
        let capture: SharedRoadmapCapture = Arc::new(Mutex::new(RoadmapCapture::default()));
        let tool = SubmitDiagnosticAssessmentTool(capture.clone());
        let ack = tool.call(&mut ToolContext::new(), sample_assessment()).await.expect("infallible");
        assert!(ack.saved);
        assert_eq!(capture.lock().unwrap().assessment.as_ref().unwrap().topic, "Ciclo de Krebs");
    }

    #[tokio::test]
    async fn propose_syllabus_plan_writes_into_the_shared_capture() {
        let capture: SharedRoadmapCapture = Arc::new(Mutex::new(RoadmapCapture::default()));
        let tool = ProposeSyllabusPlanTool(capture.clone());
        let args = ProposeSyllabusPlanArgs {
            core_focus: "Lo esencial".to_string(),
            identified_needs: vec!["Entender el flujo".to_string()],
            learning_strategy: "Guiado".to_string(),
            syllabus: RoadmapSyllabusPackage {
                course_title: "Plan".to_string(),
                total_weeks: 1,
                pace_hours_per_week: 3.0,
                milestones: vec![Milestone {
                    week: 1,
                    title: "Semana 1".to_string(),
                    deliverable: "Entregable".to_string(),
                    micromodules: vec![Micromodule {
                        label: "Módulo 1".to_string(),
                        hours: 3.0,
                        deliverable: "Artefacto verificable".to_string(),
                        interactive_blocks: vec!["socratic_prediction".to_string(), "hands_on_mission".to_string(), "metacognitive_closure".to_string()],
                    }],
                }],
            },
            closing_question: "¿Te parece adecuada esta distribución?".to_string(),
        };
        tool.call(&mut ToolContext::new(), args).await.expect("infallible");
        assert_eq!(capture.lock().unwrap().propose_syllabus_plan.as_ref().unwrap().syllabus.course_title, "Plan");
    }

    #[tokio::test]
    async fn confirm_syllabus_plan_writes_into_the_shared_capture() {
        let capture: SharedRoadmapCapture = Arc::new(Mutex::new(RoadmapCapture::default()));
        let tool = ConfirmSyllabusPlanTool(capture.clone());
        let args = ConfirmSyllabusPlanArgs {};
        tool.call(&mut ToolContext::new(), args).await.expect("infallible");
        assert!(capture.lock().unwrap().confirm_syllabus_plan.is_some());
    }

    #[tokio::test]
    async fn present_diagnostic_battery_writes_into_the_shared_capture() {
        let capture: SharedRoadmapCapture = Arc::new(Mutex::new(RoadmapCapture::default()));
        let tool = PresentDiagnosticBatteryTool(capture.clone());
        let battery = DiagnosticBattery {
            goal_alignment: "Mide la brecha".to_string(),
            questions: vec![DiagnosticQuestion {
                dimension: DiagnosticDimension::Intuition,
                prompt: "p".to_string(),
                options: vec!["A".to_string(), "B".to_string()],
                correct_option: "A".to_string(),
                diagnostic_insight: "insight".to_string(),
            }],
        };
        let ack = tool.call(&mut ToolContext::new(), battery).await.expect("infallible");
        assert!(ack.saved);
        assert_eq!(capture.lock().unwrap().diagnostic_battery.as_ref().unwrap().questions.len(), 1);
    }
}
