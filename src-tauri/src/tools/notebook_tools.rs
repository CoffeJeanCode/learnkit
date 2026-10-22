use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use rig_agent::tool::{Tool, ToolContext};
use serde::{Deserialize, Serialize};

use crate::domain::block_templates::BlockTemplate;
use crate::domain::notebook::GeneratedSectionBlock;

/// What `PublishNotebookBlockTool` writes to — read back by
/// `notebook_service::generation` once the tool-calling turn completes. ONE
/// block per call now (the whole-notebook `publish_class_notebook` contract
/// this replaces is retired — see `domain::notebook::GeneratedDynamicNotebook`'s
/// removal): the orchestrator calls `notebook_agent` once per block instead
/// of once per class, so the model never has to hold 3-5 blocks' worth of
/// content in one response.
pub type NotebookBlockCapture = Arc<Mutex<Option<PublishedBlock>>>;

/// What `publish_notebook_block` hands back to the generation loop: the
/// expanded block plus the ONE bit of provenance the critique stage can't
/// recover from the block alone — whether a MECHANICAL template
/// (`tpl_true_false`, `tpl_ordering`) produced it. Those expand to a grading
/// shape fully determined by construction, so the critique can skip the LLM
/// pedagogical critic and keep only the deterministic guardrails (see
/// `notebook_service::generation::BlockGenerationJob::critique`); the
/// expanded `GeneratedSectionBlock` itself carries no such marker.
#[derive(Debug)]
pub struct PublishedBlock {
    pub block: GeneratedSectionBlock,
    pub mechanical_template: bool,
}

#[derive(Debug, Serialize)]
pub struct PublishedAck {
    pub saved: bool,
}

/// What the model sends to `publish_notebook_block`: either a TEMPLATE for a
/// simple block (it picks one and fills only its parameters — see
/// `domain::block_templates`) or a free-form block, exactly as before. Which
/// one is decided by the `blockType` tag (`tpl_*` = template). Dispatching by
/// hand (instead of an untagged enum) keeps serde's real error message, so a
/// malformed call names the field that is wrong rather than "no variant matched".
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum PublishBlockArgs {
    Template(BlockTemplate),
    Block(GeneratedSectionBlock),
}

impl PublishBlockArgs {
    /// The expanded block plus whether a MECHANICAL template produced it (the
    /// provenance the critique stage needs and the block alone can't convey).
    pub fn into_published(self) -> PublishedBlock {
        match self {
            Self::Template(t) => {
                let mechanical_template = t.is_mechanical();
                PublishedBlock { block: t.expand(), mechanical_template }
            }
            Self::Block(b) => PublishedBlock { block: b, mechanical_template: false },
        }
    }
}

impl From<GeneratedSectionBlock> for PublishBlockArgs {
    fn from(block: GeneratedSectionBlock) -> Self {
        Self::Block(block)
    }
}

impl<'de> Deserialize<'de> for PublishBlockArgs {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let value = serde_json::Value::deserialize(deserializer)?;
        let is_template = value.get("blockType").and_then(|t| t.as_str()).is_some_and(BlockTemplate::is_template_tag);
        if is_template {
            let template: BlockTemplate = serde_json::from_value(value).map_err(D::Error::custom)?;
            template.validate().map_err(D::Error::custom)?;
            Ok(Self::Template(template))
        } else {
            serde_json::from_value(value).map(Self::Block).map_err(D::Error::custom)
        }
    }
}

pub struct PublishNotebookBlockTool(pub NotebookBlockCapture);

impl Tool for PublishNotebookBlockTool {
    const NAME: &'static str = "publish_notebook_block";
    type Args = PublishBlockArgs;
    type Output = PublishedAck;
    type Error = Infallible;

    fn description(&self) -> String {
        "Publica el SIGUIENTE bloque pedagógico de la clase (uno solo, nunca la clase completa) — \
         elegido según la forma epistemológica del tema y lo que el estudiante ya hizo hasta ahora, \
         no un molde fijo."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        single_block_json_schema()
    }

    async fn call(&self, _context: &mut ToolContext, args: Self::Args) -> Result<Self::Output, Self::Error> {
        crate::tools::emit_tool_event(Self::NAME);
        let args = repair_double_escaped(args);
        crate::dev_trace::trace_tool(Self::NAME, &args);
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(args.into_published());
        Ok(PublishedAck { saved: true })
    }
}

/// Model-authored arguments occasionally come back double-escaped: the JSON
/// string carries `\\n` instead of a newline, so once serde decodes it the
/// field is ONE line full of literal backslashes. Persisted like that, the
/// notebook shows raw `\n`, its fence detector never sees a newline (no code
/// block) and the inline-code span runs across half the text — the "cut
/// text" symptom. Applied to every model-authored tool capture below, so the
/// repair happens before anything reaches the store. Strings that already
/// contain a real newline, or that carry neither telltale, pass through
/// untouched: normal prose and paths like `C:\new` stay byte-identical.
fn repair_double_escaped<T>(value: T) -> T
where
    T: serde::de::DeserializeOwned + Serialize,
{
    let Ok(mut json) = serde_json::to_value(&value) else {
        return value;
    };
    repair_escaped_value(&mut json);
    serde_json::from_value(json).unwrap_or(value)
}

fn repair_escaped_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(s) => *s = repair_escaped_string(s),
        serde_json::Value::Array(items) => items.iter_mut().for_each(repair_escaped_value),
        serde_json::Value::Object(map) => map.values_mut().for_each(repair_escaped_value),
        _ => {}
    }
}

fn repair_escaped_string(s: &str) -> String {
    if s.contains(['\n', '\r']) || !s.contains('\\') {
        return s.to_string();
    }
    if !s.contains("```") && !s.contains("\\\"") {
        return s.to_string();
    }
    s.replace("\\r\\n", "\n").replace("\\n", "\n").replace("\\\"", "\"")
}

/// A remediation hint shown ONLY on a failed gate attempt (1st or 2nd — the
/// 3rd triggers a full re-approach block instead, see `notebook_service::
/// generation::submit_gate_response`). `scaffold_type` is informational (lets
/// the frontend style a Socratic question differently from a worked
/// counterexample); `content` is the actual text shown to the student, and
/// MUST NOT contain the correct answer/solution — enforced by prompt
/// discipline in `notebook_gate_grader_agent`'s system prompt, not by code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScaffoldType {
    SocraticHint,
    WorkedExample,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GateScaffold {
    pub scaffold_type: ScaffoldType,
    pub content: String,
}

/// What `notebook_gate_grader_agent` reports back for a free-text gate
/// submission (`heuristic_error_audit`'s diagnosis, `hands_on_mission`'s
/// solution) — the two block types deterministic Rust grading can't handle.
/// `scaffold` is `None` when `passed` (nothing to remediate).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GateGradingResult {
    pub passed: bool,
    pub rationale: String,
    /// Per-criterion judgment against the block's rubric/reference
    /// (`evaluationRubricSummary` / `modelSolution`): what was met, with the
    /// evidence found, and what is still missing. Stored in the skill
    /// evidence history; never shown to the student.
    #[serde(default)]
    pub criteria: Vec<crate::domain::skill_evidence::RubricCriterion>,
    #[serde(default)]
    pub scaffold: Option<GateScaffold>,
}

pub type GateGradingCapture = Arc<Mutex<Option<GateGradingResult>>>;

#[derive(Debug, Serialize)]
pub struct GradedAck {
    pub saved: bool,
}

pub struct GradeGateSubmissionTool(pub GateGradingCapture);

impl Tool for GradeGateSubmissionTool {
    const NAME: &'static str = "grade_gate_submission";
    type Args = GateGradingResult;
    type Output = GradedAck;
    type Error = Infallible;

    fn description(&self) -> String {
        "Califica la entrega en texto libre de un estudiante contra la solución/rúbrica del bloque. \
         Si NO aprueba, agrega exactamente UNA pista socrática o un contraejemplo trabajado — nunca \
         reveles la solución/respuesta correcta en `scaffold.content`."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        grade_gate_submission_json_schema()
    }

    async fn call(&self, _context: &mut ToolContext, args: Self::Args) -> Result<Self::Output, Self::Error> {
        crate::tools::emit_tool_event(Self::NAME);
        let args = repair_double_escaped(args);
        crate::dev_trace::trace_tool(Self::NAME, &args);
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(args);
        Ok(GradedAck { saved: true })
    }
}

/// What `closure_feedback_grader` reports back for the closing block's
/// student reflection — the notebook's only submission graded for genuine
/// synthesis (gates grade mastery instead). Unlike [`GateGradingResult`],
/// `feedback` is ALWAYS student-facing: a pass confirms the module is done,
/// a fail says concretely what to add. Approving this is what completes the
/// class (see `domain::notebook::class_is_complete`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosureFeedbackResult {
    pub passed: bool,
    pub feedback: String,
}

pub type ClosureFeedbackCapture = Arc<Mutex<Option<ClosureFeedbackResult>>>;

pub struct GradeClosureSubmissionTool(pub ClosureFeedbackCapture);

impl Tool for GradeClosureSubmissionTool {
    const NAME: &'static str = "grade_closure_submission";
    type Args = ClosureFeedbackResult;
    type Output = GradedAck;
    type Error = Infallible;

    fn description(&self) -> String {
        "Califica la reflexión de cierre de un estudiante contra la tarea de síntesis y la lista de \
         autoevaluación del bloque. `feedback` es SIEMPRE para el estudiante, en español, 2-4 frases: \
         qué logró conectar o qué agregar (sin escribir la reflexión por él)."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        grade_closure_submission_json_schema()
    }

    async fn call(&self, _context: &mut ToolContext, args: Self::Args) -> Result<Self::Output, Self::Error> {
        crate::tools::emit_tool_event(Self::NAME);
        let args = repair_double_escaped(args);
        crate::dev_trace::trace_tool(Self::NAME, &args);
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(args);
        Ok(GradedAck { saved: true })
    }
}

/// What `pedagogical_critic_agent` reports back for one generated block —
/// the semantic (LLM) half of the two-layer review. The deterministic half
/// (`domain::pedagogy_guardrails` + `notebook_service::grounding`) runs in
/// Rust FIRST and never reaches this tool; `accepted: false` carries the
/// critic's own findings as corrective `feedback` for the generator's next
/// attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockAuditResult {
    pub accepted: bool,
    #[serde(default)]
    pub feedback: Vec<String>,
}

pub type BlockAuditCapture = Arc<Mutex<Option<BlockAuditResult>>>;

#[derive(Debug, Serialize)]
pub struct AuditedAck {
    pub saved: bool,
}

pub struct SubmitBlockAuditTool(pub BlockAuditCapture);

impl Tool for SubmitBlockAuditTool {
    const NAME: &'static str = "submit_block_audit";
    type Args = BlockAuditResult;
    type Output = AuditedAck;
    type Error = Infallible;

    fn description(&self) -> String {
        "Emite tu veredicto sobre UN bloque pedagógico recién generado: `accepted` o la lista \
         exacta de problemas en `feedback` para que el generador los corrija en su siguiente intento."
            .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "accepted": {"type": "boolean"},
                "feedback": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Obligatorio si accepted=false; vacío si accepted=true. Cada entrada es un problema concreto y corregible del bloque."
                }
            },
            "required": ["accepted", "feedback"]
        })
    }

    async fn call(&self, _context: &mut ToolContext, args: Self::Args) -> Result<Self::Output, Self::Error> {
        crate::tools::emit_tool_event(Self::NAME);
        let args = repair_double_escaped(args);
        crate::dev_trace::trace_tool(Self::NAME, &args);
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(args);
        Ok(AuditedAck { saved: true })
    }
}

fn grade_gate_submission_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "passed": {"type": "boolean"},
            "rationale": {"type": "string", "description": "Justificación breve de la calificación — no se muestra al estudiante si falló, solo se registra"},
            "criteria": {
                "type": "array",
                "description": "Un elemento por criterio de la rúbrica (hands_on_mission) o por componente de la causa raíz (heuristic_error_audit): qué se pidió, si la entrega lo cumple y la evidencia concreta encontrada en ella (o qué falta). 2 a 6 elementos.",
                "items": {
                    "type": "object",
                    "properties": {
                        "criterion": {"type": "string"},
                        "met": {"type": "boolean"},
                        "evidence": {"type": "string", "description": "Cita o hecho concreto de la entrega que lo respalda; si no se cumple, qué falta"}
                    },
                    "required": ["criterion", "met"]
                }
            },
            "scaffold": {
                "type": "object",
                "description": "Obligatorio si passed=false; omitir si passed=true. NUNCA reveles la solución/respuesta correcta aquí.",
                "properties": {
                    "scaffoldType": {"type": "string", "enum": ["socratic_hint", "worked_example"]},
                    "content": {"type": "string"}
                },
                "required": ["scaffoldType", "content"]
            }
        },
        "required": ["passed", "rationale"]
    })
}

fn grade_closure_submission_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "passed": {
                "type": "boolean",
                "description": "true solo si la reflexión responde de verdad la tarea de síntesis con sustancia propia; una frase vacía genérica no aprueba"
            },
            "feedback": {
                "type": "string",
                "description": "Feedback para el estudiante, en español, 2-4 frases, dirigido de tú: si passed=true, qué logró conectar y que el módulo queda completado; si passed=false, qué agregar concretamente. OBLIGATORIO en ambos casos."
            }
        },
        "required": ["passed", "feedback"]
    })
}

fn visual_aid_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "description": "Visual estático y declarativo — nunca un simulador dinámico",
        "oneOf": [
            {
                "properties": {
                    "renderEngine": {"const": "mermaid"},
                    "chartType": {"type": "string", "enum": ["flowchart", "sequenceDiagram", "classDiagram", "stateDiagram", "erDiagram"]},
                    "code": {"type": "string", "description": "Código Mermaid válido y compilable. En stateDiagram no uses '::' dentro de una etiqueta (p. ej. String::from): Mermaid lo rechaza y el diagrama no se dibuja — di el paso con palabras"},
                    "caption": {"type": "string"}
                },
                "required": ["renderEngine", "chartType", "code", "caption"]
            },
            {
                "properties": {
                    "renderEngine": {"const": "declarative_svg"},
                    "viewBox": {"type": "string", "description": "Estándar: \"0 0 800 450\""},
                    "elements": {
                        "type": "array",
                        "description": "Formas sueltas, sin agrupar — solo para diagramas simples. Para una escena científica coherente (membranas, campos, sistemas) usa `groups` en su lugar.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "tag": {"type": "string", "enum": ["rect", "circle", "line", "path", "text"]},
                                "props": {"type": "object"},
                                "label": {"type": ["string", "null"], "description": "Solo en `text`. En formas geométricas usa null u omite la clave."}
                            },
                            "required": ["tag", "props"]
                        }
                    },
                    "groups": {
                        "type": "array",
                        "description": "Clusters semánticos de formas (una capa anatómica, una zona física) — el shape obligatorio para escenas científicas espacialmente coherentes.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "groupId": {"type": "string"},
                                "pedagogicalRole": {"type": "string", "description": "p. ej. \"high_concentration_zone\", \"selective_barrier\""},
                                "elements": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "tag": {"type": "string", "enum": ["rect", "circle", "line", "path", "text"]},
                                            "props": {"type": "object"},
                                            "label": {"type": ["string", "null"]}
                                        },
                                        "required": ["tag", "props"]
                                    }
                                }
                            },
                            "required": ["groupId", "pedagogicalRole", "elements"]
                        }
                    },
                    "pedagogicalFocus": {"type": "string", "description": "Qué enseña el diagrama en una frase (opcional)"},
                    "caption": {"type": "string"}
                },
                "required": ["renderEngine", "caption"]
            },
            {
                "properties": {
                    "renderEngine": {"const": "conceptual_matrix"},
                    "headers": {"type": "array", "items": {"type": "string"}},
                    "rows": {"type": "array", "items": {"type": "array", "items": {"type": "string"}}},
                    "contrastFocus": {"type": "string"}
                },
                "required": ["renderEngine", "headers", "rows", "contrastFocus"]
            }
        ]
    })
}

/// Compact stand-in for the optional `visualAid` of the blocks that accept one.
/// The full schema (~2.5 KB) is declared once, on `declarative_visual_diagram`
/// (where it is required); repeating it in four more variants cost ~10 KB per
/// call. A natural-language pointer is used instead of `$ref`/`$defs`: Gemini
/// flattens refs but only reads the first `oneOf` variant, and DeepSeek /
/// Xiaomi / OpenRouter backends are unverified. The accepted arguments are
/// unchanged: deserialization lives in Rust, not in this schema.
fn visual_aid_pointer_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "description": "Opcional. Mismo shape que `visualAid` de declarative_visual_diagram (renderEngine mermaid | declarative_svg | conceptual_matrix)"
    })
}

/// JSON schema for exactly ONE `GeneratedSectionBlock` — the `Args` shape
/// for `PublishNotebookBlockTool`. Was previously only reachable as the
/// `items` schema inside `dynamic_notebook_json_schema`'s `sections` array
/// (now retired along with the whole-notebook contract); factored out to its
/// own top-level function since it's now a schema in its own right, not a
/// nested one.
pub(crate) fn single_block_json_schema() -> serde_json::Value {
    let mut schema = serde_json::json!({
        "type": "object",
        "oneOf": [
            {
                "properties": {
                    "blockType": {"const": "spaced_interleaved_retrieval"},
                    "items": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 2,
                        "items": {
                            "type": "object",
                            "properties": {
                                "conceptLabel": {"type": "string", "description": "El concepto de una clase ANTERIOR que se reactiva — nunca material nuevo de esta clase"},
                                "prompt": {"type": "string", "description": "La pregunta de recuperación rápida"},
                                "expectedAnswer": {"type": "string", "description": "La respuesta esperada — se revela DESPUÉS de que el estudiante recuerde, no es una clave de calificación secreta"}
                            },
                            "required": ["conceptLabel", "prompt", "expectedAnswer"]
                        }
                    }
                },
                "required": ["blockType", "items"],
                "description": "Ver MEMORIA COGNITIVA: SOLO como primer bloque y SOLO con learnerMemory.dueRetrieval. Autocomprobación, nunca un gate."
            },
            {
                "properties": {
                    "blockType": {"const": "anchored_micro_theory"},
                    "title": {"type": "string"},
                    "intuitiveHook": {"type": "string", "description": "🎯 El Gancho Intuitivo"},
                    "analogyBoundary": {"type": "string", "description": "🚧 El Límite de la Analogía"},
                    "systemRule": {"type": "string", "description": "📐 La Regla del Sistema"},
                    "frequentError": {"type": "string", "description": "⚠️ El Error Frecuente"}
                },
                "required": ["blockType", "title", "intuitiveHook", "systemRule", "frequentError"],
                "description": "Capas, contenido y topes de palabras: ver CATÁLOGO DE BLOQUES, bloque 1"
            },
            {
                "properties": {
                    "blockType": {"const": "declarative_visual_diagram"},
                    "title": {"type": "string"},
                    "visualAid": visual_aid_json_schema(),
                    "guidedWalkthrough": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "stepNumber": {"type": "integer"},
                                "targetVisualElement": {"type": "string"},
                                "pedagogicalInsight": {"type": "string"}
                            },
                            "required": ["stepNumber", "targetVisualElement", "pedagogicalInsight"]
                        }
                    }
                },
                "required": ["blockType", "title", "visualAid", "guidedWalkthrough"]
            },
            {
                "properties": {
                    "blockType": {"const": "branching_scenario_challenge"},
                    "scenario": {"type": "string"},
                    "decisionPoint": {"type": "string"},
                    "branches": {
                        "type": "array",
                        "minItems": 2,
                        "maxItems": 4,
                        "description": "Exactamente una branch con isOptimal true — esta es la clave de calificación, no se envía al estudiante",
                        "items": {
                            "type": "object",
                            "properties": {
                                "choice": {"type": "string"},
                                "consequence": {"type": "string"},
                                "isOptimal": {"type": "boolean"}
                            },
                            "required": ["choice", "consequence", "isOptimal"]
                        }
                    },
                    "visualAid": visual_aid_pointer_schema()
                },
                "required": ["blockType", "scenario", "decisionPoint", "branches"],
                "description": "El estudiante debe elegir una branch para desbloquear el siguiente bloque."
            },
            {
                "properties": {
                    "blockType": {"const": "heuristic_error_audit"},
                    "instruction": {"type": "string"},
                    "flawedRepresentation": {
                        "type": "object",
                        "properties": {
                            "context": {"type": "string"},
                            "buggySnippetOrDiagram": {"type": "string"},
                            "errorType": {"type": "string", "enum": ["syntax", "conceptual_misunderstanding", "structural_inversion"]}
                        },
                        "required": ["context", "buggySnippetOrDiagram", "errorType"]
                    },
                    "guidingQuestions": {"type": "array", "items": {"type": "string"}},
                    "modelSolution": {
                        "type": "string",
                        "description": "Clave de calificación — nunca se envía al estudiante hasta que apruebe el diagnóstico que escriba"
                    },
                    "faultyLineNumber": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Línea (desde 1) de buggySnippetOrDiagram donde está la causa raíz. Inclúyelo SIEMPRE que el snippet sea de varias líneas: el estudiante primero hace clic en la línea defectuosa y luego escribe su diagnóstico. Clave de calificación, no se envía al estudiante hasta que apruebe."
                    },
                    "visualAid": visual_aid_pointer_schema()
                },
                "required": ["blockType", "instruction", "flawedRepresentation", "guidingQuestions", "modelSolution"],
                "description": "El estudiante escribe su propio diagnóstico de la causa raíz (texto libre) para desbloquear el siguiente bloque — lo califica notebook_gate_grader contra modelSolution."
            },
            {
                "properties": {
                    "blockType": {"const": "interactive_prediction_gate"},
                    "question": {"type": "string"},
                    "interactionMode": {
                        "type": "string",
                        "enum": ["multiple_choice", "causal_chain_order", "boundary_case_audit"],
                        "description": "Cómo responde el estudiante. Omitir = multiple_choice. Cada modo usa SOLO sus campos (ver CATÁLOGO, bloque 5)."
                    },
                    "options": {"type": "array", "items": {"type": "string"}, "minItems": 2, "maxItems": 4, "description": "SOLO multiple_choice"},
                    "conceptualFeedbackMap": {"type": "object", "description": "SOLO multiple_choice: una entrada por cada opción de options"},
                    "correctOption": {
                        "type": "string",
                        "description": "SOLO multiple_choice. Debe ser EXACTAMENTE uno de los valores de options — clave de calificación, nunca se envía al estudiante"
                    },
                    "shuffledSteps": {"type": "array", "items": {"type": "string"}, "minItems": 3, "maxItems": 4, "description": "SOLO causal_chain_order: los eslabones del mecanismo en un orden DISTINTO al causal"},
                    "correctOrder": {"type": "array", "items": {"type": "string"}, "minItems": 3, "maxItems": 4, "description": "SOLO causal_chain_order: los MISMOS eslabones, textualmente idénticos, en orden causal — clave de calificación, nunca se envía al estudiante"},
                    "boundaryCases": {
                        "type": "array",
                        "minItems": 2,
                        "maxItems": 3,
                        "description": "SOLO boundary_case_audit: al menos un caso que cumple la regla y uno que la rompe, ambos aparentemente parecidos",
                        "items": {
                            "type": "object",
                            "properties": {
                                "statement": {"type": "string", "description": "El micro-caso concreto, autocontenido"},
                                "followsRule": {"type": "boolean", "description": "true si cumple la regla — clave de calificación, nunca se envía al estudiante"}
                            },
                            "required": ["statement", "followsRule"]
                        }
                    },
                    "visualAid": visual_aid_pointer_schema()
                },
                "required": ["blockType", "question"],
                "description": "Bloquea el avance hasta acertar. Campos requeridos por modo: ver CATÁLOGO, bloque 5."
            },
            {
                "properties": {
                    "blockType": {"const": "hands_on_mission"},
                    "challengeStatement": {"type": "string"},
                    "expectedMilestoneArtifact": {"type": "string"},
                    "constraints": {"type": "array", "items": {"type": "string"}, "description": "Restricciones reales, no genéricas"},
                    "scaffoldingHints": {"type": "array", "items": {"type": "string"}},
                    "evaluationRubricSummary": {"type": "array", "items": {"type": "string"}},
                    "isTransfer": {"type": "boolean", "description": "true SOLO en el reto de transferencia (ver RETO DE TRANSFERENCIA). Omitir (false) en una misión normal."},
                    "supportLevel": {
                        "type": "string",
                        "enum": ["full", "guided", "faded", "independent"],
                        "description": "OBLIGATORIO: cuánto apoyo da esta misión (sigue scaffolding.supportLevel; ver ANDAMIAJE, regla 9)."
                    },
                    "workedTemplate": {
                        "type": "string",
                        "description": "guided/faded: plantilla o solución resuelta al ~70% con huecos marcados con ___ (o /* TODO */) — se precarga en el editor del estudiante para que no empiece en blanco. Omitir en independent/transferencia."
                    },
                    "interactiveSteps": {
                        "type": "array",
                        "description": "guided/faded (recomendado): 2-4 micro-pasos de completar un hueco ANTES del reto completo, ordenados de lo más modelado a lo menos. Solo pueden exigir lo ya enseñado en contentSoFar (un paso nuevo a la vez).",
                        "items": {
                            "type": "object",
                            "properties": {
                                "prompt": {"type": "string", "description": "El hueco a completar, con contexto mínimo"},
                                "expectedToken": {"type": "string", "description": "La respuesta corta esperada (una palabra, símbolo o expresión breve)"},
                                "options": {"type": "array", "items": {"type": "string"}, "minItems": 2, "maxItems": 4, "description": "Opcional: fichas para elegir; si las pones deben incluir expectedToken"}
                            },
                            "required": ["prompt", "expectedToken"]
                        }
                    },
                    "visualAid": visual_aid_pointer_schema()
                },
                "required": ["blockType", "challengeStatement", "expectedMilestoneArtifact", "constraints", "scaffoldingHints", "evaluationRubricSummary", "supportLevel"],
                "description": "El estudiante entrega su solución en texto libre para desbloquear el siguiente bloque — lo califica notebook_gate_grader contra evaluationRubricSummary."
            },
            {
                "properties": {
                    "blockType": {"const": "metacognitive_closure"},
                    "synthesisTask": {"type": "string", "description": "Contraste entre el modelo mental inicial del alumno y lo observado en la sesión"},
                    "predictionComparison": {
                        "type": "object",
                        "description": "OBLIGATORIO: contraste explícito entre initialPrediction (te la pasan en el input, verbatim) y finalResult (lo que este notebook realmente demostró)",
                        "properties": {
                            "initialPrediction": {"type": "string"},
                            "finalResult": {"type": "string"},
                            "contrastNarrative": {"type": "string"}
                        },
                        "required": ["initialPrediction", "finalResult", "contrastNarrative"]
                    },
                    "selfEvaluationChecklist": {"type": "array", "items": {"type": "string"}}
                },
                "required": ["blockType", "synthesisTask", "predictionComparison", "selfEvaluationChecklist"],
                "description": "Último bloque de la clase; nunca es un gate (no bloquea nada)."
            }
        ]
    });
    // Hybrid mode: the simple block kinds are also offered as parameter-only templates.
    if let Some(one_of) = schema.get_mut("oneOf").and_then(|o| o.as_array_mut()) {
        one_of.extend(block_template_schemas());
    }
    schema
}

/// Parameter-only schemas for `domain::block_templates::BlockTemplate`. Much
/// shorter than their free-form counterparts: the model supplies the facts,
/// Rust derives the answer key and shuffles the options.
fn block_template_schemas() -> Vec<serde_json::Value> {
    // The `feedback`/`consequence` hints are declared on the first use only
    // (`correct` / `optimal`); the repeated item schemas stay bare.
    let option = |what: &str, hints: bool| {
        let feedback = if hints {
            serde_json::json!({"type": "string", "description": "Retroalimentación conceptual que ve el estudiante al elegir esta opción"})
        } else {
            serde_json::json!({"type": "string"})
        };
        serde_json::json!({
            "type": "object",
            "properties": {"text": {"type": "string", "description": what}, "feedback": feedback},
            "required": ["text", "feedback"]
        })
    };
    let branch = |hints: bool| {
        let consequence = if hints {
            serde_json::json!({"type": "string", "description": "Consecuencia visible tras elegir esta rama"})
        } else {
            serde_json::json!({"type": "string"})
        };
        serde_json::json!({
            "type": "object",
            "properties": {"choice": {"type": "string"}, "consequence": consequence},
            "required": ["choice", "consequence"]
        })
    };
    vec![
        serde_json::json!({
            "properties": {
                "blockType": {"const": "tpl_multiple_choice"},
                "question": {"type": "string"},
                "correct": option("La opción correcta", true),
                "distractors": {"type": "array", "minItems": 1, "maxItems": 3, "items": option("Un distractor plausible (error típico), de longitud parecida a la correcta", false)}
            },
            "required": ["blockType", "question", "correct", "distractors"],
            "description": "PLANTILLA de opción múltiple (compuerta conceptual). Solo das la correcta y los distractores: el sistema mezcla el orden y deriva la clave. Equivale a interactive_prediction_gate multiple_choice."
        }),
        serde_json::json!({
            "properties": {
                "blockType": {"const": "tpl_true_false"},
                "statement": {"type": "string", "description": "Afirmación concreta que es claramente verdadera o falsa"},
                "isTrue": {"type": "boolean"},
                "explanation": {"type": "string", "description": "Por qué es verdadera/falsa (se muestra tras responder)"}
            },
            "required": ["blockType", "statement", "isTrue", "explanation"],
            "description": "PLANTILLA verdadero/falso (compuerta conceptual rápida). Equivale a interactive_prediction_gate multiple_choice con 2 opciones."
        }),
        serde_json::json!({
            "properties": {
                "blockType": {"const": "tpl_ordering"},
                "question": {"type": "string"},
                "stepsInOrder": {"type": "array", "minItems": 3, "maxItems": 4, "items": {"type": "string"}, "description": "Los eslabones en el orden CORRECTO; el sistema los desordena"}
            },
            "required": ["blockType", "question", "stepsInOrder"],
            "description": "PLANTILLA para ordenar un mecanismo o proceso. Equivale a interactive_prediction_gate causal_chain_order."
        }),
        serde_json::json!({
            "properties": {
                "blockType": {"const": "tpl_scenario_choice"},
                "scenario": {"type": "string"},
                "decisionPoint": {"type": "string"},
                "optimal": branch(true),
                "alternatives": {"type": "array", "minItems": 1, "maxItems": 3, "items": branch(false)}
            },
            "required": ["blockType", "scenario", "decisionPoint", "optimal", "alternatives"],
            "description": "PLANTILLA de decisión situacional (seleccionar caso). Das la rama óptima y las alternativas; el sistema mezcla el orden. Equivale a branching_scenario_challenge."
        }),
        serde_json::json!({
            "properties": {
                "blockType": {"const": "tpl_quick_recall"},
                "items": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 2,
                    "items": {
                        "type": "object",
                        "properties": {
                            "conceptLabel": {"type": "string", "description": "Concepto de una clase ANTERIOR"},
                            "prompt": {"type": "string"},
                            "expectedAnswer": {"type": "string"}
                        },
                        "required": ["conceptLabel", "prompt", "expectedAnswer"]
                    }
                }
            },
            "required": ["blockType", "items"],
            "description": "PLANTILLA de repaso rápido; mismas reglas que spaced_interleaved_retrieval (solo como primer bloque y solo con dueRetrieval)."
        }),
    ]
}

/// JSON schema for a `DiagnosticBattery` — used ONLY by
/// `roadmap_tools::PresentDiagnosticBatteryTool` (never a notebook block):
/// the battery belongs to the roadmap itself, generated once alongside the
/// syllabus, not to any one class.
pub(crate) fn diagnostic_battery_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "goalAlignment": {"type": "string", "description": "Cómo estas preguntas miden la brecha hacia targetGoal"},
            "questions": {
                "type": "array",
                "minItems": 3,
                "maxItems": 4,
                "items": {
                    "type": "object",
                    "properties": {
                        "dimension": {"type": "string", "enum": ["intuition", "mechanics", "critical_case", "boundary"]},
                        "prompt": {"type": "string"},
                        "options": {
                            "type": "array",
                            "items": {"type": "string"},
                            "minItems": 2,
                            "maxItems": 4
                        },
                        "correctOption": {"type": "string"},
                        "diagnosticInsight": {"type": "string", "description": "Qué revela sobre su modelo mental elegir una opción incorrecta"}
                    },
                    "required": ["dimension", "prompt", "options", "correctOption", "diagnosticInsight"]
                }
            }
        },
        "required": ["goalAlignment", "questions"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn publish_notebook_block_writes_into_the_shared_capture() {
        let capture: NotebookBlockCapture = Arc::new(Mutex::new(None));
        let tool = PublishNotebookBlockTool(capture.clone());
        let block = GeneratedSectionBlock::AnchoredMicroTheory {
            title: "Título".to_string(),
            intuitive_hook: "Hook".to_string(),
            analogy_boundary: None,
            system_rule: "Regla".to_string(),
            frequent_error: "Error común".to_string(),
        };
        let ack = tool.call(&mut ToolContext::new(), block.into()).await.expect("infallible");
        assert!(ack.saved);
        assert!(capture.lock().unwrap().is_some());
    }

    #[tokio::test]
    async fn submit_block_audit_writes_into_the_shared_capture() {
        let capture: BlockAuditCapture = Arc::new(Mutex::new(None));
        let tool = SubmitBlockAuditTool(capture.clone());
        let audit = BlockAuditResult {
            accepted: false,
            feedback: vec!["el systemRule repite el intuitiveHook".to_string()],
        };
        let ack = tool.call(&mut ToolContext::new(), audit).await.expect("infallible");
        assert!(ack.saved);
        let stored = capture.lock().unwrap();
        assert!(!stored.as_ref().unwrap().accepted);
        assert_eq!(stored.as_ref().unwrap().feedback.len(), 1);
    }

    #[tokio::test]
    async fn grade_gate_submission_writes_into_the_shared_capture() {
        let capture: GateGradingCapture = Arc::new(Mutex::new(None));
        let tool = GradeGateSubmissionTool(capture.clone());
        let result = GateGradingResult {
            passed: false,
            rationale: "No identifica la causa raíz".to_string(),
            criteria: vec![],
            scaffold: Some(GateScaffold { scaffold_type: ScaffoldType::SocraticHint, content: "¿Qué pasa justo antes del error?".to_string() }),
        };
        let ack = tool.call(&mut ToolContext::new(), result).await.expect("infallible");
        assert!(ack.saved);
        assert!(!capture.lock().unwrap().as_ref().unwrap().passed);
    }

    #[tokio::test]
    async fn grade_closure_submission_writes_the_student_facing_feedback_into_the_capture() {
        let capture: ClosureFeedbackCapture = Arc::new(Mutex::new(None));
        let tool = GradeClosureSubmissionTool(capture.clone());
        let result = ClosureFeedbackResult {
            passed: true,
            feedback: "Conectaste tu predicción con lo observado — módulo completado.".to_string(),
        };
        let ack = tool.call(&mut ToolContext::new(), result).await.expect("infallible");
        assert!(ack.saved);
        let stored = capture.lock().unwrap();
        assert!(stored.as_ref().unwrap().passed);
        assert!(!stored.as_ref().unwrap().feedback.is_empty());
    }

    #[test]
    fn escape_repair_only_touches_double_escaped_text() {
        // The real corruption: literal backslash escapes, no real newline,
        // a fence to prove it was meant to be multiline.
        let broken = "Lee el programa:\\n\\n```rust\\nfn main() {}\\n```\\n\\n¿Qué imprime?";
        let repaired = repair_escaped_string(broken);
        assert!(repaired.contains('\n'), "{repaired:?}");
        assert!(!repaired.contains("\\n"), "{repaired:?}");
        assert!(repaired.contains("```rust"), "{repaired:?}");

        // Everything else must stay byte-identical.
        assert_eq!(repair_escaped_string("C:\\new\\file"), "C:\\new\\file");
        assert_eq!(repair_escaped_string("ya trae\nsalto real"), "ya trae\nsalto real");
        assert_eq!(repair_escaped_string("sin escapes ni valla"), "sin escapes ni valla");
        assert_eq!(repair_escaped_string("una sola\\nlínea sin telltale"), "una sola\\nlínea sin telltale");
    }

    #[tokio::test]
    async fn publish_repairs_a_double_escaped_block_before_it_reaches_the_capture() {
        let capture: NotebookBlockCapture = Arc::new(Mutex::new(None));
        let tool = PublishNotebookBlockTool(capture.clone());
        let block = GeneratedSectionBlock::InteractivePredictionGate {
            question: "Lee el programa:\\n\\n```rust\\nfn main() {}\\n```".to_string(),
            options: vec!["A".to_string(), "B".to_string()],
            conceptual_feedback_map: [("A".to_string(), "a".to_string()), ("B".to_string(), "b".to_string())]
                .into_iter()
                .collect(),
            correct_option: "B".to_string(),
            interaction_mode: None,
            shuffled_steps: None,
            correct_order: None,
            boundary_cases: None,
            visual_aid: None,
        };
        tool.call(&mut ToolContext::new(), block.into()).await.expect("infallible");

        let stored = capture.lock().unwrap();
        let value = serde_json::to_value(&stored.as_ref().unwrap().block).expect("serializes");
        let question = value.get("question").and_then(|q| q.as_str()).expect("question");
        assert!(question.contains('\n'), "escapes became real newlines: {question:?}");
        assert!(!question.contains("\\n"), "no backslash-n left: {question:?}");
        assert!(question.contains("```rust"), "the fence survived: {question:?}");
    }

    #[test]
    fn publish_args_dispatch_templates_and_free_blocks_by_block_type() {
        let tpl: PublishBlockArgs = serde_json::from_value(serde_json::json!({
            "blockType": "tpl_true_false", "statement": "s", "isTrue": true, "explanation": "e"
        }))
        .expect("template parses");
        assert!(matches!(tpl, PublishBlockArgs::Template(_)));
        let free: PublishBlockArgs = serde_json::from_value(serde_json::json!({
            "blockType": "anchored_micro_theory", "title": "t", "intuitiveHook": "h", "systemRule": "r", "frequentError": "f"
        }))
        .expect("free block still parses");
        assert!(matches!(free, PublishBlockArgs::Block(_)));
    }

    #[test]
    fn a_malformed_template_fails_with_the_exact_reason() {
        let dup = serde_json::from_value::<PublishBlockArgs>(serde_json::json!({
            "blockType": "tpl_multiple_choice", "question": "q",
            "correct": {"text": "A", "feedback": "f"},
            "distractors": [{"text": "a", "feedback": "g"}]
        }))
        .unwrap_err()
        .to_string();
        assert!(dup.contains("tpl_multiple_choice") && dup.contains("distintas"), "{dup}");
        let missing = serde_json::from_value::<PublishBlockArgs>(serde_json::json!({"blockType": "tpl_ordering", "question": "q"})).unwrap_err().to_string();
        assert!(missing.contains("stepsInOrder"), "{missing}");
        let unknown = serde_json::from_value::<PublishBlockArgs>(serde_json::json!({"blockType": "tpl_nope"})).unwrap_err().to_string();
        assert!(unknown.contains("tpl_nope"), "{unknown}");
    }

    #[tokio::test]
    async fn publishing_a_template_stores_the_expanded_block() {
        let capture: NotebookBlockCapture = Arc::new(Mutex::new(None));
        let tool = PublishNotebookBlockTool(capture.clone());
        let args: PublishBlockArgs = serde_json::from_value(serde_json::json!({
            "blockType": "tpl_scenario_choice", "scenario": "s", "decisionPoint": "d",
            "optimal": {"choice": "bien", "consequence": "ok"},
            "alternatives": [{"choice": "mal", "consequence": "ko"}]
        }))
        .unwrap();
        tool.call(&mut ToolContext::new(), args).await.expect("infallible");
        let stored = capture.lock().unwrap();
        assert!(matches!(&stored.as_ref().unwrap().block, GeneratedSectionBlock::BranchingScenarioChallenge { branches, .. } if branches.len() == 2));
        // scenario_choice is content-heavy, not mechanical: the LLM critic
        // still audits its branches.
        assert!(!stored.as_ref().unwrap().mechanical_template);
    }

    #[tokio::test]
    async fn only_mechanical_templates_are_flagged_for_the_critic_skip() {
        let flag_of = |json: serde_json::Value| async move {
            let capture: NotebookBlockCapture = Arc::new(Mutex::new(None));
            let tool = PublishNotebookBlockTool(capture.clone());
            let args: PublishBlockArgs = serde_json::from_value(json).expect("parses");
            tool.call(&mut ToolContext::new(), args).await.expect("infallible");
            let flag = capture.lock().unwrap().as_ref().unwrap().mechanical_template;
            flag
        };
        // Mechanical: expansion fully determines the grading shape.
        assert!(flag_of(serde_json::json!({
            "blockType": "tpl_true_false", "statement": "s", "isTrue": true, "explanation": "e"
        }))
        .await);
        assert!(flag_of(serde_json::json!({
            "blockType": "tpl_ordering", "question": "q", "stepsInOrder": ["a", "b", "c"]
        }))
        .await);
        // Not mechanical: distractors still warrant a semantic read.
        assert!(!flag_of(serde_json::json!({
            "blockType": "tpl_multiple_choice", "question": "q",
            "correct": {"text": "A", "feedback": "f"},
            "distractors": [{"text": "B", "feedback": "g"}]
        }))
        .await);
        // Free-form blocks are never flagged.
        assert!(!flag_of(serde_json::json!({
            "blockType": "anchored_micro_theory", "title": "t", "intuitiveHook": "h",
            "systemRule": "r", "frequentError": "f"
        }))
        .await);
    }

    #[test]
    fn visual_aid_is_declared_in_full_once_and_every_accepting_variant_still_offers_it() {
        let schema = single_block_json_schema();
        let variants = schema["oneOf"].as_array().expect("oneOf");
        let with_aid: Vec<&str> = variants
            .iter()
            .filter(|v| v["properties"].get("visualAid").is_some())
            .map(|v| v["properties"]["blockType"]["const"].as_str().expect("tag"))
            .collect();
        assert_eq!(
            with_aid,
            ["declarative_visual_diagram", "branching_scenario_challenge", "heuristic_error_audit", "interactive_prediction_gate", "hands_on_mission"]
        );
        let full: Vec<&str> = variants
            .iter()
            .filter(|v| v["properties"]["visualAid"].get("oneOf").is_some())
            .map(|v| v["properties"]["blockType"]["const"].as_str().expect("tag"))
            .collect();
        assert_eq!(full, ["declarative_visual_diagram"], "the engines are spelled out in exactly one place");
        // It stays required where it was required.
        assert!(variants[2]["required"].as_array().unwrap().iter().any(|r| r == "visualAid"));
    }

    #[test]
    fn the_tool_schema_offers_every_template() {
        let schema = single_block_json_schema();
        let tags: Vec<&str> = schema["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v["properties"]["blockType"]["const"].as_str())
            .collect();
        for tpl in ["tpl_multiple_choice", "tpl_true_false", "tpl_ordering", "tpl_scenario_choice", "tpl_quick_recall", "interactive_prediction_gate"] {
            assert!(tags.contains(&tpl), "missing {tpl}");
        }
    }
}
