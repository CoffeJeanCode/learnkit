//! The two tools behind the syllabus written in parts (see
//! `domain::plan_parts`): the OUTLINE and the SESSIONS of one week. Like every
//! single-tool flow they only store their arguments; code validates them.

use std::convert::Infallible;

use rig_agent::tool::{Tool, ToolContext};

use crate::domain::plan_parts::{PlanOutlineArgs, WeekSessionsArgs};
use crate::tools::research_tools::{ResearchAck, SharedResearchCapture};

fn capstone_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "description": "El proyecto de transferencia terminal (Backward Design): el punto de llegada desde el que se diseñó todo el temario hacia atrás.",
        "properties": {
            "title": {"type": "string"},
            "description": {"type": "string"},
            "verifiableEvidence": {"type": "string"}
        },
        "required": ["title", "description", "verifiableEvidence"]
    })
}

pub struct SubmitPlanOutlineTool(pub SharedResearchCapture);

impl Tool for SubmitPlanOutlineTool {
    const NAME: &'static str = "submit_plan_outline";
    type Args = PlanOutlineArgs;
    type Output = ResearchAck;
    type Error = Infallible;

    fn description(&self) -> String {
        "Entrega el esquema del temario: el diagnóstico consolidado, una línea por semana (título, entregable, \
         meta semanal), el proyecto terminal y la pregunta de cierre. SIN las sesiones de cada semana: esas se \
         piden después, una semana por vez."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "courseTitle": {"type": "string"},
                "coreFocus": {"type": "string"},
                "identifiedNeeds": {"type": "array", "items": {"type": "string"}},
                "learningStrategy": {"type": "string"},
                "weeks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "week": {"type": "integer"},
                            "title": {"type": "string"},
                            "deliverable": {"type": "string", "description": "UNA frase: el artefacto verificable con el que cierra la semana."},
                            "weeklyGoal": {"type": "string", "description": "Una o dos frases: qué problema real resuelve la semana y qué capacidad nueva desbloquea."}
                        },
                        "required": ["week", "title", "deliverable", "weeklyGoal"]
                    }
                },
                "capstoneProject": capstone_schema(),
                "closingQuestion": {"type": "string", "description": "p. ej. \"¿Te parece adecuada esta distribución o deseas ajustar el ritmo?\""}
            },
            "required": ["courseTitle", "coreFocus", "identifiedNeeds", "learningStrategy", "weeks", "capstoneProject", "closingQuestion"]
        })
    }

    async fn call(&self, _context: &mut ToolContext, args: Self::Args) -> Result<Self::Output, Self::Error> {
        crate::tools::emit_tool_event(Self::NAME);
        crate::dev_trace::trace_tool(Self::NAME, &args);
        self.0.lock().unwrap_or_else(|e| e.into_inner()).outline = Some(args);
        Ok(ResearchAck { saved: true })
    }
}

pub struct SubmitWeekSessionsTool(pub SharedResearchCapture);

impl Tool for SubmitWeekSessionsTool {
    const NAME: &'static str = "submit_week_sessions";
    type Args = WeekSessionsArgs;
    type Output = ResearchAck;
    type Error = Infallible;

    fn description(&self) -> String {
        "Entrega las sesiones de UNA semana del temario, tantas como pide el contexto (`sessionCount`). Las horas de \
         cada sesión las asigna el sistema: no las incluyas."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "sessions": {
                    "type": "array",
                    "minItems": 2,
                    "items": {
                        "type": "object",
                        "properties": {
                            "label": {"type": "string", "description": "El tramo de días + el título concreto, p. ej. \"Días 1-2 — Recorrer el árbol sin clonar nodos\"."},
                            "focus": {"type": "string", "description": "UNA frase (máx. 25 palabras): los conceptos centrales y la fricción real que resuelve la sesión."},
                            "deliverable": {
                                "type": "object",
                                "description": "Artefacto concreto y verificable de ESTA sesión — nunca texto libre.",
                                "properties": {
                                    "artifactType": {"type": "string", "enum": ["tests_passing", "formal_diagram", "functional_cli", "diagnostic_matrix", "working_demo", "other"]},
                                    "description": {"type": "string", "description": "El artefacto concreto, UNA frase (máx. 15 palabras), nunca un verbo vago."}
                                },
                                "required": ["artifactType", "description"]
                            },
                            "objective": {"type": "string", "description": "Capacidad observable, UNA frase en infinitivo (máx. 15 palabras)."},
                            "interactiveBlocks": {
                                "type": "array",
                                "minItems": 3,
                                "maxItems": 4,
                                "items": {
                                    "type": "string",
                                    "enum": ["interactive_visual_anchor", "socratic_prediction", "error_audit_challenge", "hands_on_mission", "metacognitive_closure"]
                                }
                            }
                        },
                        "required": ["label", "focus", "deliverable", "objective", "interactiveBlocks"]
                    }
                }
            },
            "required": ["sessions"]
        })
    }

    async fn call(&self, _context: &mut ToolContext, args: Self::Args) -> Result<Self::Output, Self::Error> {
        crate::tools::emit_tool_event(Self::NAME);
        crate::dev_trace::trace_tool(Self::NAME, &args);
        self.0.lock().unwrap_or_else(|e| e.into_inner()).week = Some(args);
        Ok(ResearchAck { saved: true })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schemas_name_the_fields_the_args_deserialize() {
        let outline = SubmitPlanOutlineTool(Default::default()).parameters();
        assert_eq!(outline["properties"]["weeks"]["items"]["required"], serde_json::json!(["week", "title", "deliverable", "weeklyGoal"]));
        assert!(outline["required"].as_array().unwrap().iter().any(|v| v == "capstoneProject"));

        let week = SubmitWeekSessionsTool(Default::default()).parameters();
        let item = &week["properties"]["sessions"]["items"];
        assert!(item["properties"].get("hours").is_none(), "hours are code's job, never the model's");
        assert_eq!(item["required"], serde_json::json!(["label", "focus", "deliverable", "objective", "interactiveBlocks"]));
        // Every artifact type and block the schema offers must be accepted.
        for kind in item["properties"]["deliverable"]["properties"]["artifactType"]["enum"].as_array().unwrap() {
            let parsed: Result<WeekSessionsArgs, _> = serde_json::from_value(serde_json::json!({ "sessions": [{
                "label": "l", "focus": "f", "objective": "o", "interactiveBlocks": ["hands_on_mission"],
                "deliverable": { "artifactType": kind, "description": "d" }
            }]}));
            assert!(parsed.is_ok(), "{kind} must deserialize");
        }
    }
}
