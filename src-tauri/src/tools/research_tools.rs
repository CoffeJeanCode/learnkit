//! The single-tool capture flows that run OUTSIDE the notebook and the roadmap
//! chat (also the two steps of the syllabus written in parts, see
//! `plan_part_tools`): the resource-research pipeline (the PLANNER proposes search queries,
//! the CURATOR picks among the hits those searches returned) and the intake
//! QUESTIONS agent (proposes what to ask a learner about their topic). Each tool
//! is called once by its own agent and does nothing but store its arguments —
//! validation in code (`domain::resources`, `domain::learning_context`) decides
//! what survives.

use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use rig_agent::tool::{Tool, ToolContext};
use serde::Serialize;

use crate::domain::learning_context::ContextQuestionsArgs;
use crate::domain::plan_parts::{PlanOutlineArgs, WeekSessionsArgs};
use crate::domain::resources::{ResearchPlanArgs, ResourceSelectionArgs};

/// Which single-tool step a model turn is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchStep {
    /// Propose search queries per class.
    Plan,
    /// Pick the resources to keep among the hits found.
    Select,
    /// Propose the intake questions for a learner's topic.
    Questions,
    /// Write the syllabus OUTLINE (title, one line per week, capstone).
    Outline,
    /// Write the sessions of ONE week of the syllabus.
    Week,
}

/// Where the research tools write. One slot per step: a run only ever fills its own.
#[derive(Debug, Default, Clone)]
pub struct ResearchCapture {
    pub plan: Option<ResearchPlanArgs>,
    pub selection: Option<ResourceSelectionArgs>,
    pub questions: Option<ContextQuestionsArgs>,
    pub outline: Option<PlanOutlineArgs>,
    pub week: Option<WeekSessionsArgs>,
}

impl ResearchCapture {
    /// Whether the slot of `step` is filled — the signal that ends the tool
    /// loop and that makes a post-capture error salvageable.
    pub fn is_complete(&self, step: ResearchStep) -> bool {
        match step {
            ResearchStep::Plan => self.plan.is_some(),
            ResearchStep::Select => self.selection.is_some(),
            ResearchStep::Questions => self.questions.is_some(),
            ResearchStep::Outline => self.outline.is_some(),
            ResearchStep::Week => self.week.is_some(),
        }
    }
}

pub type SharedResearchCapture = Arc<Mutex<ResearchCapture>>;

#[derive(Debug, Serialize)]
pub struct ResearchAck {
    pub saved: bool,
}

pub struct SubmitResearchPlanTool(pub SharedResearchCapture);

impl Tool for SubmitResearchPlanTool {
    const NAME: &'static str = "submit_research_plan";
    type Args = ResearchPlanArgs;
    type Output = ResearchAck;
    type Error = Infallible;

    fn description(&self) -> String {
        "Entrega las consultas de búsqueda web para encontrar recursos prácticos (tutoriales, \
         documentación, ejercicios) de cada clase: 1 o 2 por clase, usando el `classId` exacto del contexto."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "queries": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "classId": {"type": "string", "description": "El classId EXACTO de la clase, tal cual viene en el contexto."},
                            "query": {"type": "string", "description": "Consulta de búsqueda concreta y práctica, de hasta 120 caracteres."}
                        },
                        "required": ["classId", "query"]
                    }
                }
            },
            "required": ["queries"]
        })
    }

    async fn call(&self, _context: &mut ToolContext, args: Self::Args) -> Result<Self::Output, Self::Error> {
        crate::tools::emit_tool_event(Self::NAME);
        crate::dev_trace::trace_tool(Self::NAME, &args);
        self.0.lock().unwrap_or_else(|e| e.into_inner()).plan = Some(args);
        Ok(ResearchAck { saved: true })
    }
}

pub struct SubmitResourceSelectionTool(pub SharedResearchCapture);

impl Tool for SubmitResourceSelectionTool {
    const NAME: &'static str = "submit_resource_selection";
    type Args = ResourceSelectionArgs;
    type Output = ResearchAck;
    type Error = Infallible;

    fn description(&self) -> String {
        "Entrega, por clase, los recursos que conservas de entre los resultados de búsqueda recibidos \
         (máximo 4 por clase). Cada `url` debe ser EXACTAMENTE una de las que aparecen en los resultados \
         de esa clase; una URL que no esté ahí se descarta."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "classes": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "classId": {"type": "string"},
                            "picks": {
                                "type": "array",
                                "description": "Vacío si ningún resultado sirve para esa clase.",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "url": {"type": "string", "description": "Copiada tal cual de los resultados de ESA clase."},
                                        "kind": {"type": "string", "enum": ["tutorial", "documentation", "video", "exercise", "reference"]},
                                        "why": {"type": "string", "description": "Una frase: por qué sirve para esta clase y para este estudiante."},
                                        "whatToDo": {"type": "string", "description": "La primera acción concreta con él (p. ej. «sigue la sección Primeros pasos hasta instalar X»)."}
                                    },
                                    "required": ["url", "kind", "why", "whatToDo"]
                                }
                            }
                        },
                        "required": ["classId", "picks"]
                    }
                }
            },
            "required": ["classes"]
        })
    }

    async fn call(&self, _context: &mut ToolContext, args: Self::Args) -> Result<Self::Output, Self::Error> {
        crate::tools::emit_tool_event(Self::NAME);
        crate::dev_trace::trace_tool(Self::NAME, &args);
        self.0.lock().unwrap_or_else(|e| e.into_inner()).selection = Some(args);
        Ok(ResearchAck { saved: true })
    }
}

pub struct SubmitContextQuestionsTool(pub SharedResearchCapture);

impl Tool for SubmitContextQuestionsTool {
    const NAME: &'static str = "submit_context_questions";
    type Args = ContextQuestionsArgs;
    type Output = ResearchAck;
    type Error = Infallible;

    fn description(&self) -> String {
        "Entrega entre 2 y 5 preguntas cortas para conocer al estudiante en SU tema: qué sabe ya, con qué \
         cuenta para practicar y qué límites tiene. Debe haber al menos una de conocimiento previo."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "questions": {
                    "type": "array",
                    "minItems": 2,
                    "maxItems": 5,
                    "items": {
                        "type": "object",
                        "properties": {
                            "followUps": {
                                "type": "array",
                                "maxItems": 2,
                                "description": "Opcional, solo en preguntas kind=choice que NO sean prior_knowledge: 1 o 2 preguntas más simples que se hacen únicamente si el estudiante contesta «No lo sé» (p. ej. si no sabe qué computadora tiene: «¿Qué ves al encenderla?»). Mismos campos que una pregunta, sin followUps propios ni prior_knowledge.",
                                "items": {"type": "object"}
                            },
                            "id": {"type": "string", "description": "Identificador corto en minúsculas, números y _ (p. ej. adn_previo)."},
                            "label": {"type": "string", "description": "La pregunta, en español, dirigida al estudiante (máx. 140 caracteres)."},
                            "kind": {"type": "string", "enum": ["choice", "text"]},
                            "purpose": {"type": "string", "enum": ["prior_knowledge", "equipment", "constraint"]},
                            "options": {
                                "type": "array",
                                "description": "Solo si kind=choice: 2 a 6 opciones distintas y cortas. NO incluyas «No lo sé»: el formulario ya lo agrega. Vacío si kind=text.",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "label": {"type": "string"},
                                        "signal": {"type": "integer", "minimum": 0, "maximum": 2, "description": "SOLO en prior_knowledge: 0 = nada todavía, 1 = algo, 2 = lo domina."}
                                    },
                                    "required": ["label"]
                                }
                            }
                        },
                        "required": ["id", "label", "kind", "purpose"]
                    }
                }
            },
            "required": ["questions"]
        })
    }

    async fn call(&self, _context: &mut ToolContext, args: Self::Args) -> Result<Self::Output, Self::Error> {
        crate::tools::emit_tool_event(Self::NAME);
        crate::dev_trace::trace_tool(Self::NAME, &args);
        self.0.lock().unwrap_or_else(|e| e.into_inner()).questions = Some(args);
        Ok(ResearchAck { saved: true })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_questions_step_is_complete_only_when_its_own_slot_is_filled() {
        let mut c = ResearchCapture::default();
        c.plan = Some(ResearchPlanArgs::default());
        assert!(!c.is_complete(ResearchStep::Questions), "another step's slot does not count");
        c.questions = Some(ContextQuestionsArgs::default());
        assert!(c.is_complete(ResearchStep::Questions));
    }

    #[test]
    fn the_questions_schema_matches_what_the_args_accept() {
        let schema = SubmitContextQuestionsTool(Default::default()).parameters();
        let item = &schema["properties"]["questions"]["items"];
        assert_eq!(item["required"], serde_json::json!(["id", "label", "kind", "purpose"]));
        for kind in item["properties"]["kind"]["enum"].as_array().expect("enum") {
            for purpose in item["properties"]["purpose"]["enum"].as_array().expect("enum") {
                let args: Result<ContextQuestionsArgs, _> =
                    serde_json::from_value(serde_json::json!({ "questions": [{ "id": "q", "label": "x", "kind": kind, "purpose": purpose }] }));
                assert!(args.is_ok(), "{kind}/{purpose} must deserialize");
            }
        }
        let with_options: ContextQuestionsArgs = serde_json::from_value(serde_json::json!({
            "questions": [{ "id": "q", "label": "x", "kind": "choice", "purpose": "prior_knowledge", "options": [{ "label": "Nada", "signal": 0 }, { "label": "Algo" }] }]
        }))
        .expect("options with and without a signal");
        assert_eq!(with_options.questions[0].options[1].signal, None);

        // The follow-ups the schema offers deserialize under the name the schema uses.
        assert!(item["properties"]["followUps"].is_object(), "the schema must offer followUps");
        let nested: ContextQuestionsArgs = serde_json::from_value(serde_json::json!({
            "questions": [{ "id": "q", "label": "x", "kind": "choice", "purpose": "equipment",
                "options": [{ "label": "A" }, { "label": "B" }],
                "followUps": [{ "id": "f", "label": "y", "kind": "text", "purpose": "equipment" }] }]
        }))
        .expect("followUps");
        assert_eq!(nested.questions[0].follow_ups.len(), 1);
    }

    #[test]
    fn a_step_is_complete_only_when_its_own_slot_is_filled() {
        let mut c = ResearchCapture::default();
        assert!(!c.is_complete(ResearchStep::Plan) && !c.is_complete(ResearchStep::Select));
        c.plan = Some(ResearchPlanArgs::default());
        assert!(c.is_complete(ResearchStep::Plan));
        assert!(!c.is_complete(ResearchStep::Select), "filling the plan does not complete the selection step");
        c.selection = Some(ResourceSelectionArgs::default());
        assert!(c.is_complete(ResearchStep::Select));
    }

    #[test]
    fn the_schemas_name_the_fields_the_args_deserialize() {
        let plan = SubmitResearchPlanTool(Default::default()).parameters();
        assert_eq!(plan["required"], serde_json::json!(["queries"]));
        assert_eq!(plan["properties"]["queries"]["items"]["required"], serde_json::json!(["classId", "query"]));

        let selection = SubmitResourceSelectionTool(Default::default()).parameters();
        let pick = &selection["properties"]["classes"]["items"]["properties"]["picks"]["items"];
        assert_eq!(pick["required"], serde_json::json!(["url", "kind", "why", "whatToDo"]));
        // Every kind the schema offers must deserialize into a real `ResourceKind`.
        for kind in pick["properties"]["kind"]["enum"].as_array().expect("enum") {
            let args: Result<ResourceSelectionArgs, _> = serde_json::from_value(serde_json::json!({
                "classes": [{ "classId": "c", "picks": [{ "url": "https://a.dev", "kind": kind, "why": "w", "whatToDo": "d" }] }]
            }));
            assert!(args.is_ok(), "kind {kind} must be accepted");
        }
    }

    #[test]
    fn the_wire_format_the_model_produces_deserializes() {
        let plan: ResearchPlanArgs = serde_json::from_value(serde_json::json!({ "queries": [{ "classId": "c1", "query": "instalar ubuntu" }] })).expect("plan");
        assert_eq!(plan.queries[0].class_id, "c1");
        let sel: ResourceSelectionArgs =
            serde_json::from_value(serde_json::json!({ "classes": [{ "classId": "c1", "picks": [] }] })).expect("selection");
        assert!(sel.classes[0].picks.is_empty());
    }
}
