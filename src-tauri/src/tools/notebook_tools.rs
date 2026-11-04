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
        let raw = serde_json::Value::deserialize(deserializer)?;
        let value = normalize_block_value(raw);
        let is_template = value.get("blockType").and_then(|t| t.as_str()).is_some_and(BlockTemplate::is_template_tag);
        if is_template {
            let template: BlockTemplate = serde_json::from_value(value.clone()).map_err(|e| {
                let bt = value.get("blockType").and_then(|t| t.as_str()).unwrap_or("tpl");
                D::Error::custom(format!("Error en plantilla '{bt}': {e}. Revisa los parámetros requeridos."))
            })?;
            template.validate().map_err(|e| {
                D::Error::custom(format!("Validación fallida en plantilla: {e}"))
            })?;
            Ok(Self::Template(template))
        } else {
            match serde_json::from_value::<GeneratedSectionBlock>(value.clone()) {
                Ok(block) => Ok(Self::Block(block)),
                Err(err) => {
                    let bt = value.get("blockType").and_then(|t| t.as_str()).unwrap_or("desconocido");
                    Err(D::Error::custom(format!(
                        "Error en publish_notebook_block para '{bt}': {err}. Asegúrate de suministrar todos los campos requeridos en camelCase con sus tipos exactos."
                    )))
                }
            }
        }
    }
}

/// Normalizes model payloads: unwraps wrapper objects (e.g. `{"arguments": {...}}`),
/// parses stringified JSON, standardizes `blockType` casing to snake_case, and
/// coerces string numbers to numeric JSON values.
pub fn normalize_block_value(mut value: serde_json::Value) -> serde_json::Value {
    // 1. If wrapped in stringified JSON, parse it (with fault-tolerant auto-repair)
    if let serde_json::Value::String(ref s) = value {
        let trimmed = s.trim();
        if (trimmed.starts_with('{') && trimmed.ends_with('}'))
            || (trimmed.starts_with('[') && trimmed.ends_with(']'))
            || trimmed.contains('{')
        {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed) {
                value = parsed;
            } else if let Ok(parsed) = crate::tools::json_repair::parse_repaired_value(trimmed) {
                value = parsed;
            }
        }
    }

    // 2. Unwrap wrapper objects {"arguments": ...}, {"block": ...}, {"parameters": ...}, {"payload": ...}, {"data": ...}
    for key in ["arguments", "block", "parameters", "payload", "data"] {
        if let Some(inner) = value.get(key) {
            if inner.is_object() {
                value = inner.clone();
                break;
            } else if let Some(s) = inner.as_str() {
                let trimmed = s.trim();
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    value = parsed;
                    break;
                } else if let Ok(parsed) = crate::tools::json_repair::parse_repaired_value(trimmed) {
                    value = parsed;
                    break;
                }
            }
        }
    }

    // 3. Normalize blockType casing to snake_case
    if let Some(bt) = value.get("blockType").and_then(|v| v.as_str()).map(str::to_string) {
        let normalized = match bt.as_str() {
            "anchoredMicroTheory" | "anchored-micro-theory" => "anchored_micro_theory",
            "spacedInterleavedRetrieval" | "spaced-interleaved-retrieval" => "spaced_interleaved_retrieval",
            "declarativeVisualDiagram" | "declarative-visual-diagram" => "declarative_visual_diagram",
            "branchingScenarioChallenge" | "branching-scenario-challenge" => "branching_scenario_challenge",
            "heuristicErrorAudit" | "heuristic-error-audit" => "heuristic_error_audit",
            "interactivePredictionGate" | "interactive-prediction-gate" => "interactive_prediction_gate",
            "handsOnMission" | "hands-on-mission" => "hands_on_mission",
            "metacognitiveClosure" | "metacognitive-closure" => "metacognitive_closure",
            "tplMultipleChoice" | "tpl-multiple-choice" => "tpl_multiple_choice",
            "tplTrueFalse" | "tpl-true-false" => "tpl_true_false",
            "tplOrdering" | "tpl-ordering" => "tpl_ordering",
            "tplScenarioChoice" | "tpl-scenario-choice" => "tpl_scenario_choice",
            "tplQuickRecall" | "tpl-quick-recall" => "tpl_quick_recall",
            "tplTheory" | "tpl-theory" => "tpl_theory",
            "tplBoundaryCases" | "tpl-boundary-cases" => "tpl_boundary_cases",
            other => other,
        };
        if normalized != bt {
            if let Some(obj) = value.as_object_mut() {
                obj.insert("blockType".to_string(), serde_json::Value::String(normalized.to_string()));
            }
        }
    }

    // 3.5 Unwrap stringified inner fields (e.g. models emitting "predictionComparison": "{\"initialPrediction\": ...}")
    unwrap_stringified_inner_fields(&mut value);

    // 4. Coerce common stringified numeric fields to numbers to prevent Serde rejection
    coerce_numeric_fields(&mut value);

    // 5. Ensure required block fields for truncated or incomplete payloads
    ensure_required_block_fields(&mut value);

    value
}

fn normalize_visual_aid_object(va: &mut serde_json::Map<String, serde_json::Value>) {
    if !va.contains_key("renderEngine") {
        if let Some(engine) = va.remove("diagramType").or_else(|| va.remove("type")) {
            va.insert("renderEngine".to_string(), engine);
        } else {
            va.insert("renderEngine".to_string(), serde_json::json!("mermaid"));
        }
    }
    va.entry("chartType").or_insert_with(|| serde_json::json!("flowchart"));
    if !va.contains_key("code") {
        if let Some(c) = va.remove("source").or_else(|| va.remove("spec")) {
            va.insert("code".to_string(), c);
        } else {
            va.insert("code".to_string(), serde_json::json!("graph TD\nA[Inicio] --> B[Fin]"));
        }
    }
    va.entry("caption").or_insert_with(|| serde_json::json!("Diagrama"));
}

fn ensure_required_block_fields(value: &mut serde_json::Value) {
    let Some(map) = value.as_object_mut() else { return };
    if let Some(va) = map.get_mut("visualAid").and_then(|v| v.as_object_mut()) {
        normalize_visual_aid_object(va);
    }
    let bt = map.get("blockType").and_then(|v| v.as_str()).unwrap_or("").to_string();
    match bt.as_str() {
        "anchored_micro_theory" => {
            map.entry("title").or_insert_with(|| serde_json::json!("Concepto Clave"));
            map.entry("intuitiveHook").or_insert_with(|| serde_json::json!(""));
            map.entry("systemRule").or_insert_with(|| serde_json::json!(""));
            map.entry("frequentError").or_insert_with(|| serde_json::json!(""));
        }
        "declarative_visual_diagram" => {
            map.entry("title").or_insert_with(|| serde_json::json!("Diagrama Conceptual"));
            map.entry("guidedWalkthrough").or_insert_with(|| serde_json::json!([]));
            if !map.contains_key("visualAid") || !map["visualAid"].is_object() {
                map.insert("visualAid".to_string(), serde_json::json!({
                    "renderEngine": "mermaid",
                    "chartType": "flowchart",
                    "code": "graph TD\nA[Inicio] --> B[Fin]",
                    "caption": "Diagrama conceptual"
                }));
            }
        }
        "branching_scenario_challenge" => {
            map.entry("scenario").or_insert_with(|| serde_json::json!("Escenario de decisión"));
            map.entry("decisionPoint").or_insert_with(|| serde_json::json!("¿Qué camino decides tomar?"));
            if !map.contains_key("branches") || !map["branches"].is_array() || map["branches"].as_array().unwrap().is_empty() {
                map.insert("branches".to_string(), serde_json::json!([
                    {"choice": "Opción A (Recomendada)", "consequence": "Solución con mejores prácticas.", "isOptimal": true},
                    {"choice": "Opción B (Alternativa)", "consequence": "Resultado con efectos secundarios a considerar.", "isOptimal": false}
                ]));
            } else if let Some(branches) = map.get_mut("branches").and_then(|v| v.as_array_mut()) {
                let mut has_optimal = false;
                for b in branches.iter_mut() {
                    if let Some(obj) = b.as_object_mut() {
                        obj.entry("choice").or_insert_with(|| serde_json::json!("Opción"));
                        obj.entry("consequence").or_insert_with(|| serde_json::json!("Consecuencia de la decisión."));
                        if obj.get("isOptimal").and_then(|v| v.as_bool()) == Some(true) {
                            has_optimal = true;
                        } else if !obj.contains_key("isOptimal") {
                            obj.insert("isOptimal".to_string(), serde_json::json!(false));
                        }
                    }
                }
                if !has_optimal && !branches.is_empty() {
                    if let Some(first) = branches.first_mut().and_then(|v| v.as_object_mut()) {
                        first.insert("isOptimal".to_string(), serde_json::json!(true));
                    }
                }
            }
        }
        "heuristic_error_audit" => {
            map.entry("instruction").or_insert_with(|| serde_json::json!("Audita el siguiente fragmento e identifica el problema conceptual:"));
            if !map.contains_key("flawedRepresentation") || !map["flawedRepresentation"].is_object() {
                let context = map.remove("context").unwrap_or_else(|| serde_json::json!("Contexto del caso"));
                let snippet = map.remove("buggySnippetOrDiagram")
                    .or_else(|| map.remove("flawedArtifact"))
                    .or_else(|| map.remove("flawedSnippet"))
                    .unwrap_or_else(|| serde_json::json!("// Fragmento para auditar\n"));
                let err_type = map.remove("errorType").unwrap_or_else(|| serde_json::json!("conceptual_misunderstanding"));
                map.insert("flawedRepresentation".to_string(), serde_json::json!({
                    "context": context,
                    "buggySnippetOrDiagram": snippet,
                    "errorType": err_type,
                }));
            } else if let Some(fr) = map.get_mut("flawedRepresentation").and_then(|v| v.as_object_mut()) {
                fr.entry("context").or_insert_with(|| serde_json::json!("Contexto del caso"));
                fr.entry("buggySnippetOrDiagram").or_insert_with(|| serde_json::json!("// Fragmento para auditar\n"));
                fr.entry("errorType").or_insert_with(|| serde_json::json!("conceptual_misunderstanding"));
            }
            map.entry("guidingQuestions").or_insert_with(|| serde_json::json!(["¿Cuál es la causa raíz del problema?", "¿Cómo lo corregirías adecuadamente?"]));
            map.entry("modelSolution").or_insert_with(|| serde_json::json!("Solución modelo identificando el error y la corrección recomendada."));
        }
        "interactive_prediction_gate" => {
            map.entry("question").or_insert_with(|| serde_json::json!("Pregunta de predicción conceptual:"));
            if !map.contains_key("options") || !map["options"].is_array() || map["options"].as_array().unwrap().is_empty() {
                map.insert("options".to_string(), serde_json::json!(["Opción A", "Opción B"]));
            }
            if !map.contains_key("correctOption") || map["correctOption"].as_str().unwrap_or("").is_empty() {
                let first_opt = map["options"].as_array().and_then(|a| a.first()).and_then(|v| v.as_str()).unwrap_or("Opción A");
                map.insert("correctOption".to_string(), serde_json::json!(first_opt));
            }
            map.entry("conceptualFeedbackMap").or_insert_with(|| serde_json::json!({}));
            map.entry("boundaryCases").or_insert_with(|| serde_json::json!([]));
        }
        "hands_on_mission" => {
            map.entry("challengeStatement").or_insert_with(|| serde_json::json!("Misión práctica guiada:"));
            map.entry("expectedMilestoneArtifact").or_insert_with(|| serde_json::json!("Artefacto de solución completado y verificado"));
            map.entry("constraints").or_insert_with(|| serde_json::json!([]));
            if !map.contains_key("scaffoldingHints") {
                if let Some(hints) = map.remove("hints") {
                    map.insert("scaffoldingHints".to_string(), hints);
                } else {
                    map.insert("scaffoldingHints".to_string(), serde_json::json!([]));
                }
            }
            map.entry("evaluationRubricSummary").or_insert_with(|| serde_json::json!(["Completitud del artefacto", "Cumplimiento de restricciones clave"]));
            map.entry("isTransfer").or_insert_with(|| serde_json::json!(false));
        }
        "metacognitive_closure" => {
            map.entry("synthesisTask").or_insert_with(|| serde_json::json!("Reflexión de cierre y síntesis de lo aprendido:"));
            if !map.contains_key("predictionComparison") || !map["predictionComparison"].is_object() {
                map.insert("predictionComparison".to_string(), serde_json::json!({
                    "initialPrediction": "Predicción inicial sobre el funcionamiento del concepto",
                    "finalResult": "Resultado real observado durante la práctica",
                    "contrastNarrative": "Contraste entre lo esperado y lo verificado empíricamente"
                }));
            } else if let Some(pc) = map.get_mut("predictionComparison").and_then(|v| v.as_object_mut()) {
                pc.entry("initialPrediction").or_insert_with(|| serde_json::json!("Predicción inicial"));
                pc.entry("finalResult").or_insert_with(|| serde_json::json!("Resultado real"));
                pc.entry("contrastNarrative").or_insert_with(|| serde_json::json!("Contraste observado"));
            }
            map.entry("selfEvaluationChecklist").or_insert_with(|| serde_json::json!(["He comprendido los conceptos principales y su aplicación práctica"]));
        }
        "spaced_interleaved_retrieval" => {
            if !map.contains_key("items") || !map["items"].is_array() || map["items"].as_array().unwrap().is_empty() {
                map.insert("items".to_string(), serde_json::json!([
                    {
                        "conceptLabel": "Recuperación activa",
                        "prompt": "¿Recuerdas el concepto principal abordado anteriormente?",
                        "expectedAnswer": "Respuesta clave de recuperación"
                    }
                ]));
            } else if let Some(items) = map.get_mut("items").and_then(|v| v.as_array_mut()) {
                for item in items.iter_mut() {
                    if let Some(obj) = item.as_object_mut() {
                        obj.entry("conceptLabel").or_insert_with(|| serde_json::json!("Concepto previo"));
                        obj.entry("prompt").or_insert_with(|| serde_json::json!("Pregunta de recuperación"));
                        obj.entry("expectedAnswer").or_insert_with(|| serde_json::json!("Respuesta esperada"));
                    }
                }
            }
        }
        "tpl_theory" => {
            map.entry("title").or_insert_with(|| serde_json::json!("Concepto Clave"));
            map.entry("intuitiveHook").or_insert_with(|| serde_json::json!(""));
            map.entry("analogyBoundary").or_insert_with(|| serde_json::json!("Límite de la analogía"));
            map.entry("systemRule").or_insert_with(|| serde_json::json!(""));
            map.entry("frequentError").or_insert_with(|| serde_json::json!(""));
        }
        "tpl_multiple_choice" => {
            map.entry("question").or_insert_with(|| serde_json::json!("Pregunta conceptual:"));
            if !map.contains_key("correct") || !map["correct"].is_object() {
                let correct_text = map.get("correctOption").and_then(|v| v.as_str()).unwrap_or("Opción Correcta");
                let feedback = map.get("explanation").and_then(|v| v.as_str()).unwrap_or("Explicación de la opción correcta");
                map.insert("correct".to_string(), serde_json::json!({ "text": correct_text, "feedback": feedback }));
            }
            if !map.contains_key("distractors") || !map["distractors"].is_array() || map["distractors"].as_array().unwrap().is_empty() {
                map.insert("distractors".to_string(), serde_json::json!([
                    { "text": "Opción Incorrecta", "feedback": "Esta opción no satisface el criterio requerido." }
                ]));
            }
        }
        "tpl_true_false" => {
            map.entry("statement").or_insert_with(|| serde_json::json!("Afirmación conceptual"));
            map.entry("isTrue").or_insert_with(|| serde_json::json!(true));
            map.entry("explanation").or_insert_with(|| serde_json::json!("Explicación"));
        }
        "tpl_ordering" => {
            map.entry("question").or_insert_with(|| serde_json::json!("Ordena los pasos correctamente:"));
            map.entry("stepsInOrder").or_insert_with(|| serde_json::json!(["Paso 1", "Paso 2"]));
        }
        "tpl_boundary_cases" => {
            map.entry("question").or_insert_with(|| serde_json::json!("Identifica los casos límite:"));
            map.entry("cases").or_insert_with(|| serde_json::json!([]));
        }
        "tpl_scenario_choice" => {
            map.entry("scenario").or_insert_with(|| serde_json::json!("Escenario de decisión:"));
            map.entry("decisionPoint").or_insert_with(|| serde_json::json!("¿Qué camino decides tomar?"));
            if !map.contains_key("optimal") || !map["optimal"].is_object() {
                map.insert("optimal".to_string(), serde_json::json!({
                    "choice": "Decisión Óptima",
                    "consequence": "Resultado positivo esperado"
                }));
            }
            if !map.contains_key("alternatives") || !map["alternatives"].is_array() || map["alternatives"].as_array().unwrap().is_empty() {
                map.insert("alternatives".to_string(), serde_json::json!([
                    { "choice": "Decisión Alternativa", "consequence": "Consecuencia desfavorable a considerar" }
                ]));
            }
        }
        "tpl_quick_recall" => {
            if !map.contains_key("items") || !map["items"].is_array() || map["items"].as_array().unwrap().is_empty() {
                map.insert("items".to_string(), serde_json::json!([
                    {
                        "conceptLabel": "Recuperación activa",
                        "prompt": "Pregunta de recuperación rápida",
                        "expectedAnswer": "Respuesta clave"
                    }
                ]));
            }
        }
        _ => {}
    }
}


fn unwrap_stringified_inner_fields(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (_k, v) in map.iter_mut() {
                if let serde_json::Value::String(s) = v {
                    let trimmed = s.trim();
                    if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
                    {
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed) {
                            *v = parsed;
                        } else if let Ok(parsed) = crate::tools::json_repair::parse_repaired_value(trimmed) {
                            *v = parsed;
                        }
                    }
                }
                if v.is_object() || v.is_array() {
                    unwrap_stringified_inner_fields(v);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                unwrap_stringified_inner_fields(item);
            }
        }
        _ => {}
    }
}
fn coerce_numeric_fields(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                if k == "stepNumber" || k == "faultyLineNumber" || k == "durationMinutes" || k == "targetStep" {
                    if let serde_json::Value::String(s) = v {
                        if let Ok(num) = s.trim().parse::<i64>() {
                            *v = serde_json::Value::Number(num.into());
                        }
                    }
                } else {
                    coerce_numeric_fields(v);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                coerce_numeric_fields(item);
            }
        }
        _ => {}
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

/// Fallback extractor inspired by OpenCode and Pi-Agent harnesses:
/// When a model does not emit a native tool call (or emits JSON in prose/markdown),
/// this scans the generated text for a valid `PublishBlockArgs` JSON payload.
pub fn extract_block_from_text(text: &str) -> Option<PublishedBlock> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    // 1. Try direct deserialization of the entire text (if model returned clean JSON).
    if let Ok(args) = serde_json::from_str::<PublishBlockArgs>(trimmed) {
        let args = repair_double_escaped(args);
        return Some(args.into_published());
    }

    // 1b. Try auto-repaired JSON on trimmed text
    if let Ok(args) = crate::tools::json_repair::parse_repaired_json::<PublishBlockArgs>(trimmed) {
        let args = repair_double_escaped(args);
        return Some(args.into_published());
    }

    // 2. Check for <tool_call>...</tool_call> tags (common in raw tool-calling outputs).
    if let Some(start) = text.find("<tool_call>") {
        let after = &text[start + "<tool_call>".len()..];
        if let Some(end) = after.find("</tool_call>") {
            let inner = after[..end].trim();
            if let Ok(val) = crate::tools::json_repair::parse_repaired_value(inner) {
                if let Some(args_val) = val.get("arguments") {
                    if let Ok(args) = serde_json::from_value::<PublishBlockArgs>(args_val.clone()) {
                        let args = repair_double_escaped(args);
                        return Some(args.into_published());
                    }
                    if let Some(args_str) = args_val.as_str() {
                        if let Ok(args) = crate::tools::json_repair::parse_repaired_json::<PublishBlockArgs>(args_str) {
                            let args = repair_double_escaped(args);
                            return Some(args.into_published());
                        }
                    }
                }
                if let Ok(args) = serde_json::from_value::<PublishBlockArgs>(val) {
                    let args = repair_double_escaped(args);
                    return Some(args.into_published());
                }
            }
        }
    }

    // 3. Scan for markdown code fences (```json ... ``` or ``` ... ```).
    let mut cursor = 0;
    while let Some(start_fence) = text[cursor..].find("```") {
        let block_start = cursor + start_fence + 3;
        let content_start = if let Some(newline) = text[block_start..].find('\n') {
            block_start + newline + 1
        } else {
            block_start
        };
        if let Some(end_fence) = text[content_start..].find("```") {
            let candidate = text[content_start..content_start + end_fence].trim();
            if candidate.contains("blockType") {
                if let Ok(args) = crate::tools::json_repair::parse_repaired_json::<PublishBlockArgs>(candidate) {
                    let args = repair_double_escaped(args);
                    return Some(args.into_published());
                }
            }
            cursor = content_start + end_fence + 3;
        } else {
            // Cut-off code fence at EOF!
            let candidate = text[content_start..].trim();
            if candidate.contains("blockType") {
                if let Ok(args) = crate::tools::json_repair::parse_repaired_json::<PublishBlockArgs>(candidate) {
                    let args = repair_double_escaped(args);
                    return Some(args.into_published());
                }
            }
            break;
        }
    }

    // 4. Scan for balanced OR truncated JSON object containing "blockType".
    let mut search_idx = 0;
    while let Some(rel_bt) = text[search_idx..].find("blockType") {
        let bt_idx = search_idx + rel_bt;
        if let Some(open_rel) = text[..bt_idx].rfind('{') {
            let mut depth = 0;
            let mut in_string = false;
            let mut escape = false;
            let mut close_idx = None;
            for (idx, ch) in text[open_rel..].char_indices() {
                if escape {
                    escape = false;
                    continue;
                }
                if ch == '\\' {
                    escape = true;
                    continue;
                }
                if ch == '"' {
                    in_string = !in_string;
                    continue;
                }
                if in_string {
                    continue;
                }
                if ch == '{' {
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                    if depth == 0 {
                        close_idx = Some(open_rel + idx);
                        break;
                    }
                }
            }
            if let Some(end) = close_idx {
                let candidate = &text[open_rel..=end];
                if let Ok(args) = crate::tools::json_repair::parse_repaired_json::<PublishBlockArgs>(candidate) {
                    let args = repair_double_escaped(args);
                    return Some(args.into_published());
                }
            } else {
                // Truncated object at EOF! (The exact "EOF while parsing an object at line 1 column ..." error)
                let candidate = &text[open_rel..];
                if let Ok(args) = crate::tools::json_repair::parse_repaired_json::<PublishBlockArgs>(candidate) {
                    let args = repair_double_escaped(args);
                    return Some(args.into_published());
                }
            }
        }
        search_idx = bt_idx + "blockType".len();
    }

    // 5. Final fallback: try partial_json::parse_partial on the text
    if let Some(partial_val) = crate::tools::partial_json::parse_partial(trimmed) {
        if let Ok(args) = serde_json::from_value::<PublishBlockArgs>(partial_val.clone()) {
            let args = repair_double_escaped(args);
            return Some(args.into_published());
        }
        if let Some(salvaged) = salvage_fallback_block(&partial_val) {
            return Some(salvaged);
        }
    }

    // 6. Direct fallback block construction if a known blockType is present
    if let Ok(val) = crate::tools::json_repair::parse_repaired_value(trimmed) {
        if let Some(published) = salvage_fallback_block(&val) {
            return Some(published);
        }
    }

    if let Some(bt_idx) = trimmed.find("blockType") {
        if let Some(open_rel) = trimmed[..bt_idx].rfind('{') {
            let candidate = &trimmed[open_rel..];
            if let Ok(val) = crate::tools::json_repair::parse_repaired_value(candidate) {
                if let Some(published) = salvage_fallback_block(&val) {
                    return Some(published);
                }
            }
        }
    }

    None
}

/// Fallback reconstruction when serde strictly rejects a malformed or cut-off payload,
/// ensuring that streamed content with a valid `blockType` is never discarded.
pub fn salvage_fallback_block(value: &serde_json::Value) -> Option<PublishedBlock> {
    let map = if let Some(obj) = value.as_object() {
        obj
    } else {
        return None;
    };

    let bt = map.get("blockType").and_then(|v| v.as_str())?;
    let normalized = match bt {
        "anchoredMicroTheory" | "anchored-micro-theory" => "anchored_micro_theory",
        "declarativeVisualDiagram" | "declarative-visual-diagram" => "declarative_visual_diagram",
        "branchingScenarioChallenge" | "branching-scenario-challenge" => "branching_scenario_challenge",
        "heuristicErrorAudit" | "heuristic-error-audit" => "heuristic_error_audit",
        "interactivePredictionGate" | "interactive-prediction-gate" => "interactive_prediction_gate",
        "handsOnMission" | "hands-on-mission" => "hands_on_mission",
        "metacognitiveClosure" | "metacognitive-closure" => "metacognitive_closure",
        "spacedInterleavedRetrieval" | "spaced-interleaved-retrieval" => "spaced_interleaved_retrieval",
        other => other,
    };

    match normalized {
        "anchored_micro_theory" => {
            let title = map.get("title").and_then(|v| v.as_str()).unwrap_or("Concepto Clave").to_string();
            let intuitive_hook = map.get("intuitiveHook").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let system_rule = map.get("systemRule").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let frequent_error = map.get("frequentError").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let analogy_boundary = map.get("analogyBoundary").and_then(|v| v.as_str()).map(String::from);
            Some(PublishedBlock {
                block: GeneratedSectionBlock::AnchoredMicroTheory {
                    title,
                    intuitive_hook,
                    analogy_boundary,
                    system_rule,
                    frequent_error,
                },
                mechanical_template: false,
            })
        }
        "hands_on_mission" => {
            let challenge_statement = map.get("challengeStatement").and_then(|v| v.as_str()).unwrap_or("Misión práctica").to_string();
            let expected_milestone_artifact = map.get("expectedMilestoneArtifact").and_then(|v| v.as_str()).unwrap_or("Artefacto de solución completado").to_string();
            let constraints = map.get("constraints").and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let scaffolding_hints = map.get("scaffoldingHints").or_else(|| map.get("hints")).and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let evaluation_rubric_summary = map.get("evaluationRubricSummary").and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_else(|| vec!["Completitud del artefacto".to_string(), "Cumplimiento de restricciones".to_string()]);
            Some(PublishedBlock {
                block: GeneratedSectionBlock::HandsOnMission {
                    challenge_statement,
                    expected_milestone_artifact,
                    constraints,
                    scaffolding_hints,
                    evaluation_rubric_summary,
                    is_transfer: false,
                    support_level: None,
                    worked_template: None,
                    interactive_steps: None,
                    visual_aid: None,
                },
                mechanical_template: false,
            })
        }
        "heuristic_error_audit" => {
            let instruction = map.get("instruction").and_then(|v| v.as_str()).unwrap_or("Audita el siguiente fragmento e identifica el problema conceptual:").to_string();
            let (context, snippet) = if let Some(fr) = map.get("flawedRepresentation").and_then(|v| v.as_object()) {
                (
                    fr.get("context").and_then(|v| v.as_str()).unwrap_or("Contexto del caso").to_string(),
                    fr.get("buggySnippetOrDiagram").and_then(|v| v.as_str()).unwrap_or("// Código a auditar\n").to_string(),
                )
            } else {
                (
                    map.get("context").and_then(|v| v.as_str()).unwrap_or("Contexto del caso").to_string(),
                    map.get("buggySnippetOrDiagram").or_else(|| map.get("flawedArtifact")).or_else(|| map.get("flawedSnippet")).and_then(|v| v.as_str()).unwrap_or("// Código a auditar\n").to_string(),
                )
            };
            Some(PublishedBlock {
                block: GeneratedSectionBlock::HeuristicErrorAudit {
                    instruction,
                    flawed_representation: crate::domain::notebook::FlawedRepresentation {
                        context,
                        buggy_snippet_or_diagram: snippet,
                        error_type: crate::domain::notebook::FlawErrorType::ConceptualMisunderstanding,
                    },
                    guiding_questions: vec!["¿Cuál es la causa del error?".to_string(), "¿Cómo lo corregirías?".to_string()],
                    model_solution: "Solución modelo identificando el error.".to_string(),
                    faulty_line_number: None,
                    visual_aid: None,
                },
                mechanical_template: false,
            })
        }
        "metacognitive_closure" => {
            let synthesis_task = map.get("synthesisTask").and_then(|v| v.as_str()).unwrap_or("Reflexión de cierre").to_string();
            Some(PublishedBlock {
                block: GeneratedSectionBlock::MetacognitiveClosure {
                    synthesis_task,
                    prediction_comparison: crate::domain::notebook::PredictionComparison {
                        initial_prediction: "Predicción inicial".to_string(),
                        final_result: "Resultado real observado".to_string(),
                        contrast_narrative: "Contraste entre lo esperado y lo evidenciado".to_string(),
                    },
                    self_evaluation_checklist: vec!["He comprendido los conceptos principales".to_string()],
                },
                mechanical_template: false,
            })
        }
        "branching_scenario_challenge" => {
            let scenario = map.get("scenario").and_then(|v| v.as_str()).unwrap_or("Escenario de decisión").to_string();
            let decision_point = map.get("decisionPoint").and_then(|v| v.as_str()).unwrap_or("¿Qué camino decides tomar?").to_string();
            Some(PublishedBlock {
                block: GeneratedSectionBlock::BranchingScenarioChallenge {
                    scenario,
                    decision_point,
                    branches: vec![
                        crate::domain::notebook::ScenarioBranch {
                            choice: "Opción A (Recomendada)".to_string(),
                            consequence: "Solución con mejores prácticas.".to_string(),
                            is_optimal: true,
                        },
                        crate::domain::notebook::ScenarioBranch {
                            choice: "Opción B (Alternativa)".to_string(),
                            consequence: "Efectos secundarios a considerar.".to_string(),
                            is_optimal: false,
                        },
                    ],
                    visual_aid: None,
                },
                mechanical_template: false,
            })
        }
        "interactive_prediction_gate" => {
            let question = map.get("question").and_then(|v| v.as_str()).unwrap_or("Pregunta de predicción").to_string();
            let options = map.get("options").and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_str().map(String::from)).collect::<Vec<_>>())
                .filter(|opts| !opts.is_empty())
                .unwrap_or_else(|| vec!["Opción A".to_string(), "Opción B".to_string()]);
            let correct_option = map.get("correctOption").and_then(|v| v.as_str()).unwrap_or(&options[0]).to_string();
            Some(PublishedBlock {
                block: GeneratedSectionBlock::InteractivePredictionGate {
                    question,
                    options,
                    conceptual_feedback_map: std::collections::HashMap::new(),
                    correct_option,
                    interaction_mode: None,
                    shuffled_steps: None,
                    correct_order: None,
                    boundary_cases: None,
                    visual_aid: None,
                },
                mechanical_template: false,
            })
        }
        "declarative_visual_diagram" => {
            let title = map.get("title").and_then(|v| v.as_str()).unwrap_or("Diagrama Conceptual").to_string();
            Some(PublishedBlock {
                block: GeneratedSectionBlock::DeclarativeVisualDiagram {
                    title,
                    visual_aid: crate::domain::notebook::StaticVisualSpec::Mermaid {
                        chart_type: crate::domain::notebook::MermaidChartType::Flowchart,
                        code: "graph TD\nA[Inicio] --> B[Fin]".to_string(),
                        caption: "Diagrama conceptual".to_string(),
                    },
                    guided_walkthrough: vec![],
                },
                mechanical_template: false,
            })
        }
        _ => None,
    }
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
    // Cheapest-to-write first: the model reads the variants in this order.
    let variable = serde_json::json!({
        "type": "object",
        "properties": {
            "sym": {"type": "string", "description": "Símbolo de la fórmula: letra + letras/dígitos/_ (p. ej. \"k\", \"masa\")"},
            "label": {"type": "string"},
            "unit": {"type": "string"},
            "min": {"type": "number"}, "max": {"type": "number"}, "step": {"type": "number"},
            "init": {"type": "number", "description": "Valor inicial, dentro de [min, max]"}
        },
        "required": ["sym", "label", "min", "max", "init"]
    });
    serde_json::json!({
        "type": "object",
        "description": "Visual estático o parametrizable. ORDEN DE PREFERENCIA: interactive_module (relación entre variables, función, vectores, gradiente, línea de tiempo) > diagram_spec (flujos, estados, secuencias, ciclos) > conceptual_matrix (comparaciones) > declarative_svg (SOLO fenómenos físicos/biológicos que ningún módulo cubre) > mermaid (evítalo).",
        "oneOf": [
            {
                "properties": {
                    "renderEngine": {"const": "interactive_module"},
                    "spec": {
                        "type": "object",
                        "description": "Módulo del catálogo cerrado; el estudiante mueve sliders y el sistema calcula y dibuja — tú solo rellenas parámetros. Elige por `module`.",
                        "properties": {
                            "module": {"type": "string", "enum": ["relation_explorer", "function_plot", "vector_sum", "particle_gradient", "timeline"]},
                            "output": {
                                "type": "object",
                                "description": "relation_explorer: lo que se calcula. expr = fórmula SOLO con los `sym` de variables (+ - * / ^ ( ), sin/cos/sqrt/abs/ln/exp/min/max, pi, e). Sin `=`.",
                                "properties": {"sym": {"type": "string"}, "label": {"type": "string"}, "unit": {"type": "string"}, "expr": {"type": "string"}},
                                "required": ["sym", "label", "expr"]
                            },
                            "variables": {"type": "array", "description": "relation_explorer: 1-4 sliders de entrada", "items": variable},
                            "expr": {"type": "string", "description": "function_plot: y = f(x) en `x` y los `params` (p. ej. \"a*x^2 + b\")"},
                            "domain": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2, "description": "function_plot: [xMin, xMax]"},
                            "params": {"type": "array", "description": "function_plot: 0-3 sliders de parámetros (\"x\" está reservado)", "items": variable},
                            "tangent": {"type": "boolean", "description": "function_plot: muestra una tangente movible (derivadas)"},
                            "vectors": {"type": "array", "description": "vector_sum: 2-4 vectores, x e y entre -10 y 10", "items": {"type": "object", "properties": {"label": {"type": "string"}, "x": {"type": "number"}, "y": {"type": "number"}}, "required": ["label", "x", "y"]}},
                            "showResultant": {"type": "boolean"},
                            "highLabel": {"type": "string"}, "lowLabel": {"type": "string"},
                            "highCount": {"type": "integer", "description": "particle_gradient: partículas en la zona densa (≤60) — debe ser mayor que lowCount"},
                            "lowCount": {"type": "integer"},
                            "hasBarrier": {"type": "boolean"},
                            "channels": {"type": "integer", "description": "particle_gradient: canales de la barrera (1-8 si hay barrera)"},
                            "events": {"type": "array", "description": "timeline: 2-8 eventos en orden", "items": {"type": "object", "properties": {"label": {"type": "string"}, "detail": {"type": "string"}}, "required": ["label", "detail"]}}
                        },
                        "required": ["module"]
                    },
                    "challenge": {
                        "type": "object",
                        "description": "Opcional (relation_explorer / function_plot): meta que el estudiante logra moviendo los sliders. `sym` = una variable, la salida o un parámetro; `target` debe ser alcanzable.",
                        "properties": {"prompt": {"type": "string"}, "sym": {"type": "string"}, "target": {"type": "number"}, "tolerance": {"type": "number"}},
                        "required": ["prompt", "sym", "target"]
                    },
                    "caption": {"type": "string"}
                },
                "required": ["renderEngine", "spec", "caption"]
            },
            {
                "properties": {
                    "renderEngine": {"const": "diagram_spec"},
                    "kind": {"type": "string", "enum": ["flowchart", "sequence", "state", "cycle", "compare"]},
                    "direction": {"type": "string", "enum": ["TD", "LR", "BT", "RL"]},
                    "title": {"type": "string"},
                    "nodes": {
                        "type": "array",
                        "description": "Máximo 10. `id` sin espacios (letras/dígitos/_). `explanation` (1-2 frases) se muestra al tocar el nodo — úsala para enseñar sin saturar el dibujo.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {"type": "string"},
                                "label": {"type": "string"},
                                "shape": {"type": "string", "enum": ["rectangle", "round", "diamond", "cylinder", "circle"]},
                                "subtext": {"type": "string"},
                                "explanation": {"type": "string"}
                            },
                            "required": ["id", "label"]
                        }
                    },
                    "edges": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "from": {"type": "string"}, "to": {"type": "string"}, "label": {"type": "string"},
                                "style": {"type": "string", "enum": ["solid", "dotted", "thick"]}
                            },
                            "required": ["from", "to"]
                        }
                    },
                    "caption": {"type": "string"}
                },
                "required": ["renderEngine", "kind", "nodes", "edges"]
            },
            {
                "properties": {
                    "renderEngine": {"const": "conceptual_matrix"},
                    "headers": {"type": "array", "items": {"type": "string"}},
                    "rows": {"type": "array", "items": {"type": "array", "items": {"type": "string"}}},
                    "contrastFocus": {"type": "string"}
                },
                "required": ["renderEngine", "headers", "rows", "contrastFocus"]
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
                    "renderEngine": {"const": "mermaid"},
                    "chartType": {"type": "string", "enum": ["flowchart", "sequenceDiagram", "classDiagram", "stateDiagram", "erDiagram"]},
                    "code": {"type": "string", "description": "Código Mermaid válido. Solo si diagram_spec no alcanza (classDiagram/erDiagram). En stateDiagram no uses '::' en etiquetas"},
                    "caption": {"type": "string"}
                },
                "required": ["renderEngine", "chartType", "code", "caption"]
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
        "description": "Opcional. Mismo shape que `visualAid` de declarative_visual_diagram (renderEngine interactive_module | diagram_spec | conceptual_matrix | declarative_svg | mermaid)"
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
        "properties": {
            "blockType": {
                "type": "string",
                "description": "Tipo de bloque pedagógico a generar o template",
                "enum": [
                    "spaced_interleaved_retrieval",
                    "anchored_micro_theory",
                    "declarative_visual_diagram",
                    "branching_scenario_challenge",
                    "heuristic_error_audit",
                    "interactive_prediction_gate",
                    "hands_on_mission",
                    "metacognitive_closure",
                    "tpl_multiple_choice",
                    "tpl_true_false",
                    "tpl_ordering",
                    "tpl_scenario_choice",
                    "tpl_quick_recall",
                    "tpl_theory",
                    "tpl_boundary_cases"
                ]
            }
        },
        "required": ["blockType"],
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
                    "intuitiveHook": {"type": "string", "description": "🎯 El Gancho Intuitivo (≤30 palabras)"},
                    "analogyBoundary": {"type": "string", "description": "🚧 El Límite de la Analogía (≤25 palabras)"},
                    "systemRule": {"type": "string", "description": "📐 La Regla del Sistema (≤240 palabras)"},
                    "frequentError": {"type": "string", "description": "⚠️ El Error Frecuente (≤40 palabras)"}
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
        serde_json::json!({
            "properties": {
                "blockType": {"const": "tpl_theory"},
                "title": {"type": "string"},
                "intuitiveHook": {"type": "string", "description": "🎯 El Gancho Intuitivo (≤30 palabras)"},
                "analogyBoundary": {"type": "string", "description": "🚧 El Límite de la Analogía (≤25 palabras)"},
                "systemRule": {"type": "string", "description": "📐 La Regla del Sistema: micro-pasos numerados con un ejemplo entrada/salida (≤240 palabras)"},
                "frequentError": {"type": "string", "description": "⚠️ esto SÍ [válido] / esto NO [confusión], porque [razón] (≤40 palabras)"}
            },
            "required": ["blockType", "title", "intuitiveHook", "analogyBoundary", "systemRule", "frequentError"],
            "description": "PLANTILLA de microteoría en capas (sin visualAid). Los topes de palabras se validan en esta misma llamada. Equivale a anchored_micro_theory."
        }),
        serde_json::json!({
            "properties": {
                "blockType": {"const": "tpl_boundary_cases"},
                "question": {"type": "string"},
                "cases": {
                    "type": "array",
                    "minItems": 2,
                    "maxItems": 3,
                    "items": {
                        "type": "object",
                        "properties": {
                            "statement": {"type": "string", "description": "Micro-caso concreto al borde de la regla"},
                            "followsRule": {"type": "boolean"}
                        },
                        "required": ["statement", "followsRule"]
                    }
                }
            },
            "required": ["blockType", "question", "cases"],
            "description": "PLANTILLA de casos límite (compuerta conceptual): al menos un caso que cumple la regla y uno que la rompe; el sistema mezcla el orden. Equivale a interactive_prediction_gate boundary_case_audit."
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
        for tpl in ["tpl_multiple_choice", "tpl_true_false", "tpl_ordering", "tpl_scenario_choice", "tpl_quick_recall", "tpl_theory", "tpl_boundary_cases", "interactive_prediction_gate"] {
            assert!(tags.contains(&tpl), "missing {tpl}");
        }
    }

    #[test]
    fn extract_block_from_text_parses_clean_json_and_markdown_blocks() {
        let json_str = r#"{
            "blockType": "anchored_micro_theory",
            "title": "Concepto",
            "intuitiveHook": "Gancho intuitivo",
            "systemRule": "Regla del sistema",
            "frequentError": "Error frecuente"
        }"#;

        // 1. Direct clean JSON
        let extracted = extract_block_from_text(json_str).expect("clean json should extract");
        assert_eq!(extracted.block.block_type().as_str(), "anchored_micro_theory");

        // 2. Wrapped in markdown code block with surrounding reasoning prose
        let prose = format!("Pensando en la lección...\n```json\n{json_str}\n```\nEspero que ayude.");
        let extracted_prose = extract_block_from_text(&prose).expect("markdown fence should extract");
        assert_eq!(extracted_prose.block.block_type().as_str(), "anchored_micro_theory");

        // 3. Template extraction
        let tpl_str = r#"```json
        {
            "blockType": "tpl_true_false",
            "statement": "El agua hierve a 100°C al nivel del mar",
            "isTrue": true,
            "explanation": "Por la presión atmosférica estándar"
        }
        ```"#;
        let extracted_tpl = extract_block_from_text(tpl_str).expect("template block should extract");
        assert!(extracted_tpl.mechanical_template);
    }

    #[test]
    fn normalize_block_value_unwraps_arguments_and_normalizes_casing() {
        let nested = serde_json::json!({
            "arguments": {
                "blockType": "anchoredMicroTheory",
                "title": "Prueba",
                "intuitiveHook": "Gancho",
                "systemRule": "Regla",
                "frequentError": "Error"
            }
        });
        let args: PublishBlockArgs = serde_json::from_value(nested).expect("should unwrap and parse");
        assert!(matches!(args, PublishBlockArgs::Block(GeneratedSectionBlock::AnchoredMicroTheory { .. })));

        let stringified = serde_json::Value::String(r#"{"blockType":"tplOrdering","question":"q","stepsInOrder":["a","b","c"]}"#.to_string());
        let args_str: PublishBlockArgs = serde_json::from_value(stringified).expect("should parse stringified json");
        assert!(matches!(args_str, PublishBlockArgs::Template(BlockTemplate::Ordering { .. })));
    }

    #[test]
    fn deserialize_coerces_string_numbers() {
        let json_with_str_num = serde_json::json!({
            "blockType": "declarative_visual_diagram",
            "title": "Diagrama",
            "visualAid": {
                "diagramType": "mermaid",
                "chartType": "flowchart",
                "source": "graph TD; A-->B;"
            },
            "guidedWalkthrough": [
                {
                    "stepNumber": "1",
                    "targetVisualElement": "A",
                    "pedagogicalInsight": "Insight"
                }
            ]
        });
        let args: PublishBlockArgs = serde_json::from_value(json_with_str_num).expect("should coerce string stepNumber");
        assert!(matches!(args, PublishBlockArgs::Block(GeneratedSectionBlock::DeclarativeVisualDiagram { .. })));
    }
}
