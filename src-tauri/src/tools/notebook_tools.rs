use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use rig_agent::tool::{Tool, ToolContext};
use serde::{Deserialize, Serialize};

use crate::domain::notebook::GeneratedSectionBlock;

/// What `PublishNotebookBlockTool` writes to — read back by
/// `notebook_service::generation` once the tool-calling turn completes. ONE
/// block per call now (the whole-notebook `publish_class_notebook` contract
/// this replaces is retired — see `domain::notebook::GeneratedDynamicNotebook`'s
/// removal): the orchestrator calls `notebook_agent` once per block instead
/// of once per class, so the model never has to hold 3-5 blocks' worth of
/// content in one response.
pub type NotebookBlockCapture = Arc<Mutex<Option<GeneratedSectionBlock>>>;

#[derive(Debug, Serialize)]
pub struct PublishedAck {
    pub saved: bool,
}

pub struct PublishNotebookBlockTool(pub NotebookBlockCapture);

impl Tool for PublishNotebookBlockTool {
    const NAME: &'static str = "publish_notebook_block";
    type Args = GeneratedSectionBlock;
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
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(args);
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
                                            "label": {"type": ["string", "null"], "description": "Solo en `text`. En formas geométricas usa null u omite la clave."}
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

/// JSON schema for exactly ONE `GeneratedSectionBlock` — the `Args` shape
/// for `PublishNotebookBlockTool`. Was previously only reachable as the
/// `items` schema inside `dynamic_notebook_json_schema`'s `sections` array
/// (now retired along with the whole-notebook contract); factored out to its
/// own top-level function since it's now a schema in its own right, not a
/// nested one.
pub(crate) fn single_block_json_schema() -> serde_json::Value {
    serde_json::json!({
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
                "description": "SOLO como PRIMER bloque de la clase, y SOLO cuando learnerMemory.dueRetrieval del input trae elementos — 1 a 2 preguntas que reactivan conceptos de clases anteriores antes de tocar material nuevo. Nunca un gate: es autocomprobación (el estudiante recuerda, luego revela la respuesta)."
            },
            {
                "properties": {
                    "blockType": {"const": "anchored_micro_theory"},
                    "title": {"type": "string"},
                    "intuitiveHook": {"type": "string", "description": "🎯 Analogía cotidiana, cero términos técnicos, 1-2 líneas"},
                    "analogyBoundary": {"type": "string", "description": "Dónde deja de aplicar la analogía de intuitiveHook — qué NO cubre técnicamente, 1 línea, para que el estudiante no extienda la metáfora más allá de lo que soporta"},
                    "systemRule": {"type": "string", "description": "📐 La regla o mecanismo explicado paso a paso, con el detalle técnico completo (hasta 180 palabras)"},
                    "frequentError": {"type": "string", "description": "⚠️ Contraste explícito entre un caso VÁLIDO y la confusión típica ('esto SÍ / esto NO, porque...'), no solo describir el error en aislamiento"}
                },
                "required": ["blockType", "title", "intuitiveHook", "systemRule", "frequentError"],
                "description": "intuitiveHook + analogyBoundary + frequentError suman máximo 100 palabras combinadas (terse); systemRule tiene su propio máximo de 180 palabras — nunca texto enciclopédico plano"
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
                    "visualAid": visual_aid_json_schema()
                },
                "required": ["blockType", "scenario", "decisionPoint", "branches"],
                "description": "Ideal para liderazgo, estrategia y gestión — cuando el dominio ES una decisión con consecuencias, no un concepto o procedimiento. El estudiante debe elegir una branch para desbloquear el siguiente bloque. visualAid es opcional — solo inclúyelo si un diagrama (org chart, estado del sistema) realmente aclara el escenario."
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
                    "visualAid": visual_aid_json_schema()
                },
                "required": ["blockType", "instruction", "flawedRepresentation", "guidingQuestions", "modelSolution"],
                "description": "El estudiante escribe su propio diagnóstico de la causa raíz (texto libre) para desbloquear el siguiente bloque — lo califica notebook_gate_grader contra modelSolution. visualAid es opcional — úsalo cuando el fallo es espacial/visual (un diagrama mal etiquetado, un flujo roto), no solo textual."
            },
            {
                "properties": {
                    "blockType": {"const": "interactive_prediction_gate"},
                    "question": {"type": "string"},
                    "options": {"type": "array", "items": {"type": "string"}, "minItems": 2, "maxItems": 4},
                    "conceptualFeedbackMap": {"type": "object", "description": "Una entrada por cada opción de options"},
                    "correctOption": {
                        "type": "string",
                        "description": "Debe ser EXACTAMENTE uno de los valores de options — clave de calificación, nunca se envía al estudiante"
                    },
                    "visualAid": visual_aid_json_schema()
                },
                "required": ["blockType", "question", "options", "conceptualFeedbackMap", "correctOption"],
                "description": "La respuesta se registra ANTES de revelar conceptualFeedbackMap — bloquea el avance hasta que el estudiante elija correctOption. visualAid es opcional — solo si la predicción realmente depende de leer un diagrama primero."
            },
            {
                "properties": {
                    "blockType": {"const": "hands_on_mission"},
                    "challengeStatement": {"type": "string"},
                    "expectedMilestoneArtifact": {"type": "string"},
                    "constraints": {"type": "array", "items": {"type": "string"}, "description": "Restricciones reales, no genéricas"},
                    "scaffoldingHints": {"type": "array", "items": {"type": "string"}},
                    "evaluationRubricSummary": {"type": "array", "items": {"type": "string"}},
                    "visualAid": visual_aid_json_schema()
                },
                "required": ["blockType", "challengeStatement", "expectedMilestoneArtifact", "constraints", "scaffoldingHints", "evaluationRubricSummary"],
                "description": "El estudiante entrega su solución en texto libre para desbloquear el siguiente bloque — lo califica notebook_gate_grader contra evaluationRubricSummary. visualAid es opcional — un diagrama de referencia del objetivo, solo si ayuda."
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
                "description": "SIEMPRE el último bloque del notebook — nunca es un gate (no bloquea nada), pero es obligatorio entre el bloque 3 y el 5"
            }
        ]
    })
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
                        "dimension": {
                            "type": "string",
                            "enum": ["intuition", "mechanics", "critical_case", "boundary"],
                            "description": "intuition/mechanics/critical_case son obligatorias (al menos una pregunta de cada una); boundary es opcional, solo si hay una 4ª pregunta."
                        },
                        "prompt": {"type": "string"},
                        "options": {
                            "type": "array",
                            "items": {"type": "string"},
                            "minItems": 2,
                            "maxItems": 4,
                            "description": "Longitudes de texto similares entre sí (variación menor al 15%) — nunca la opción correcta más larga o más explicada que las demás. Ninguna opción debe contener su propia justificación."
                        },
                        "correctOption": {"type": "string"},
                        "diagnosticInsight": {"type": "string", "description": "Qué revela sobre su modelo mental elegir una opción incorrecta — AQUÍ va la explicación, nunca dentro del texto de una opción"}
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
        let ack = tool.call(&mut ToolContext::new(), block).await.expect("infallible");
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
            visual_aid: None,
        };
        tool.call(&mut ToolContext::new(), block).await.expect("infallible");

        let stored = capture.lock().unwrap();
        let value = serde_json::to_value(stored.as_ref().unwrap()).expect("serializes");
        let question = value.get("question").and_then(|q| q.as_str()).expect("question");
        assert!(question.contains('\n'), "escapes became real newlines: {question:?}");
        assert!(!question.contains("\\n"), "no backslash-n left: {question:?}");
        assert!(question.contains("```rust"), "the fence survived: {question:?}");
    }
}
