//! Pedagogical QA harness: runs WHOLE classes through the real app pipeline
//! (`AppState` with the user's provider/model-tier config and the `.env`
//! keys — block planner, generator, repairs, grounding, critic, gate grader,
//! closure) while a simulated student, played by a DIFFERENT model, answers
//! every gate from what the class actually showed, never from the answer key.
//! The transcript of each class lands in `eval-results/class-qa/` for review.
//!
//! Opt-in and paid (real API calls):
//!   LEARNKIT_CLASS_QA=1 cargo test --lib class_qa -- --ignored --nocapture
//! `LEARNKIT_CLASS_QA_TOPICS=integrales,guitarra` runs a subset.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::domain::notebook::{BlockStatus, DynamicBlockType, GateSubmission, NotebookBlock};
use crate::state::app_state::AppState;

struct Topic {
    key: &'static str,
    course: &'static str,
    goal: &'static str,
    milestone: &'static str,
    deliverable: &'static str,
    class: &'static str,
}

const TOPICS: &[Topic] = &[
    Topic {
        key: "integrales",
        course: "Cálculo integral desde cero",
        goal: "Aprobar Cálculo II en la universidad",
        milestone: "La integral definida",
        deliverable: "Calcular áreas con el teorema fundamental",
        class: "Semana 1: Días 1-2 — La integral como área acumulada y el teorema fundamental del cálculo",
    },
    Topic {
        key: "derivadas",
        course: "Cálculo diferencial para ingeniería",
        goal: "Derivar con soltura funciones compuestas",
        milestone: "Reglas de derivación",
        deliverable: "Derivar funciones compuestas sin errores de orden",
        class: "Semana 1: Días 3-4 — La regla de la cadena: derivar funciones dentro de funciones",
    },
    Topic {
        key: "algebra_abstracta",
        course: "Álgebra abstracta: grupos",
        goal: "Aprobar Álgebra Moderna de la licenciatura en matemáticas",
        milestone: "Subgrupos y clases laterales",
        deliverable: "Demostrar propiedades de subgrupos finitos",
        class: "Semana 1: Días 1-3 — Subgrupos, clases laterales y el teorema de Lagrange",
    },
    Topic {
        key: "relatividad_general",
        course: "Relatividad general para estudiantes de física",
        goal: "Entender la gravedad como geometría",
        milestone: "Curvatura y geodésicas",
        deliverable: "Explicar la caída libre como movimiento geodésico",
        class: "Semana 1: Días 1-2 — Curvatura del espacio-tiempo y geodésicas: por qué cae una manzana",
    },
    Topic {
        key: "guitarra",
        course: "Guitarra acústica desde cero",
        goal: "Acompañar canciones sencillas con acordes abiertos",
        milestone: "Acordes abiertos y cambios",
        deliverable: "Tocar Sol, Do y Re cambiando a tiempo con un rasgueo",
        class: "Semana 1: Días 3-4 — Cambiar entre Sol, Do y Re a tiempo con el rasgueo",
    },
    Topic {
        key: "quimica_organica",
        course: "Química orgánica I",
        goal: "Predecir productos de reacciones orgánicas",
        milestone: "Sustitución nucleofílica",
        deliverable: "Elegir entre SN1 y SN2 para un sustrato dado",
        class: "Semana 2: Días 1-2 — SN1 frente a SN2: sustrato, nucleófilo y disolvente",
    },
];

const STUDENT_MODEL: &str = "deepseek-chat";
const BLOCK_WAIT: Duration = Duration::from_secs(8 * 60);
const MAX_STEPS: usize = 16;

fn load_dotenv(root: &Path) {
    let Ok(text) = std::fs::read_to_string(root.join(".env")) else { return };
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let (k, v) = (k.trim(), v.trim().trim_matches('"'));
            if !k.is_empty() && std::env::var(k).is_err() {
                // SAFETY: single-threaded at this point (before any runtime work).
                unsafe { std::env::set_var(k, v) };
            }
        }
    }
}

/// A fresh, isolated app data dir carrying ONLY the user's non-secret model
/// configuration (which provider, which strong/cheap models): keys come from
/// `.env`, the learner memory starts empty.
fn isolated_state(key: &str) -> AppState {
    let dir = std::env::temp_dir().join(format!("learnkit-class-qa-{key}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let app_dir = dir.join("learnkit");
    std::fs::create_dir_all(&app_dir).expect("qa dir");
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let user = PathBuf::from(local).join("com.learnkit.app").join("learnkit");
        for f in ["active_provider.json", "model_tiers.json", "providers.json"] {
            let _ = std::fs::copy(user.join(f), app_dir.join(f));
        }
    }
    AppState::new(dir).expect("app state")
}

/// What the student can see of a block: its content as the UI gets it (answer keys redacted).
fn visible(block: &NotebookBlock) -> Value {
    let mut c = block.content_json.clone();
    if let Some(o) = c.as_object_mut() {
        for k in ["presentation", "studentState", "pedagogicalRationale"] {
            o.remove(k);
        }
    }
    json!({ "blockType": block.block_type.as_str(), "content": c })
}

async fn ask_student(http: &reqwest::Client, key: &str, topic: &Topic, seen: &[Value], gate: &NotebookBlock, history: &[Value]) -> Value {
    let task = match gate.block_type {
        DynamicBlockType::InteractivePredictionGate => match gate.content_json.get("interactionMode").and_then(|v| v.as_str()) {
            Some("causal_chain_order") => r#"{"orderedSteps": ["…los eslabones de shuffledSteps, COPIADOS TEXTUALMENTE, en el orden que crees correcto"]}"#,
            Some("boundary_case_audit") => r#"{"verdicts": [true, false, "… un booleano por caso de boundaryCases, en orden: true = cumple la regla"]}"#,
            _ => r#"{"selectedOption": "… una de options, COPIADA TEXTUALMENTE"}"#,
        },
        DynamicBlockType::BranchingScenarioChallenge => r#"{"selectedChoice": "… el texto de choice de una de branches, COPIADO TEXTUALMENTE"}"#,
        DynamicBlockType::HeuristicErrorAudit => r#"{"line": 1, "diagnosis": "… tu diagnóstico de la causa raíz, en tus palabras"}"#,
        DynamicBlockType::HandsOnMission => r#"{"submission": "… tu entrega completa (si hay workedTemplate, complétalo; no dejes ___)"}"#,
        _ => r#"{"reflection": "… tu reflexión honesta en 2-4 frases"}"#,
    };
    let system = "Eres un ESTUDIANTE real (adulto, motivado, nivel inicial en el tema) haciendo una clase en una app. \
        Respondes SOLO con lo que entendiste del material que la clase te mostró: no conoces la clave, no eres experto, \
        y si algo no se explicó bien, te equivocas como lo haría alguien real (no a propósito ni al azar: razonas y a veces \
        razonas mal). No haces trampa ni adivinas metadatos. Responde SOLO un objeto JSON con la forma pedida.";
    let user = json!({
        "curso": topic.course,
        "clase": topic.class,
        "loQueYaViste": seen,
        "intentosPreviosEnEstaCompuerta": history,
        "compuertaActual": visible(gate),
        "formaDeTuRespuesta": task,
    });
    let body = json!({
        "model": STUDENT_MODEL,
        "temperature": 0.7,
        "response_format": {"type": "json_object"},
        "messages": [{"role": "system", "content": system}, {"role": "user", "content": user.to_string()}],
    });
    for _ in 0..3 {
        let Ok(res) = http.post("https://api.deepseek.com/chat/completions").bearer_auth(key).json(&body).send().await else { continue };
        let Ok(v) = res.json::<Value>().await else { continue };
        if let Some(text) = v["choices"][0]["message"]["content"].as_str() {
            if let Ok(answer) = serde_json::from_str::<Value>(text) {
                return answer;
            }
        }
    }
    json!({})
}

fn submission_for(gate: &NotebookBlock, a: &Value) -> Option<GateSubmission> {
    let s = |k: &str| a.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    Some(match gate.block_type {
        DynamicBlockType::InteractivePredictionGate => GateSubmission::InteractivePredictionGate {
            selected_option: a.get("selectedOption").and_then(|v| v.as_str()).map(String::from),
            ordered_steps: a.get("orderedSteps").and_then(|v| serde_json::from_value(v.clone()).ok()),
            case_verdicts: a.get("verdicts").and_then(|v| v.as_array()).map(|xs| xs.iter().filter_map(Value::as_bool).collect()),
        },
        DynamicBlockType::BranchingScenarioChallenge => GateSubmission::BranchingScenarioChallenge { selected_choice: s("selectedChoice") },
        DynamicBlockType::HeuristicErrorAudit => GateSubmission::HeuristicErrorAudit {
            diagnosis_text: s("diagnosis"),
            selected_line: a.get("line").and_then(Value::as_u64).map(|n| n as u32),
        },
        DynamicBlockType::HandsOnMission => GateSubmission::HandsOnMission { submission_text: s("submission"), hints_shown: Some(0) },
        _ => return None,
    })
}

async fn run_topic(topic: &'static Topic, out_dir: PathBuf, student_key: String) -> Value {
    let started = Instant::now();
    let state = isolated_state(topic.key);
    let svc = &state.notebook_service;
    let course = svc.store.create_course(topic.course, topic.goal, 6).expect("course");
    let milestone = svc.store.create_milestone(&course.id, 1, topic.milestone, topic.deliverable).expect("milestone");
    let class = svc.store.create_class(&milestone.id, 1, topic.class, 0, 3.0).expect("class");
    let http = reqwest::Client::builder().timeout(Duration::from_secs(180)).build().expect("http");
    let mut steps: Vec<Value> = Vec::new();
    let mut error: Option<String> = None;

    if let Err(e) = svc.start_class_notebook(None, &class.id).await {
        error = Some(format!("start: {e}"));
    }
    let mut attempts_on: (String, Vec<Value>) = (String::new(), Vec::new());
    'class: for _ in 0..(MAX_STEPS * 3) {
        if error.is_some() {
            break;
        }
        // Wait for the active block (the chain generates in the background).
        let wait = Instant::now();
        let active = loop {
            let payload = match svc.get_class_notebook_progress(&class.id).map(Some) {
                Ok(Some(p)) => p,
                _ => {
                    error = Some("notebook vanished".into());
                    break 'class;
                }
            };
            if let Some(b) = payload.blocks.iter().find(|b| b.order_index == payload.document.current_block_index).cloned() {
                break (b, payload.blocks);
            }
            // The closure is revealed with the cursor already past it (it is the last block).
            if let Some(b) = payload.blocks.iter().find(|b| b.block_type == DynamicBlockType::MetacognitiveClosure && b.status != BlockStatus::Passed).cloned() {
                break (b, payload.blocks);
            }
            if wait.elapsed() > BLOCK_WAIT {
                if svc.retry_pending_block(None, &payload.document.id).await.is_err() || wait.elapsed() > BLOCK_WAIT * 2 {
                    error = Some(format!("no block after {}s at position {}", wait.elapsed().as_secs(), payload.document.current_block_index));
                    break 'class;
                }
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        };
        let (block, all) = active;
        let seen: Vec<Value> = all.iter().filter(|b| b.order_index < block.order_index).map(|b| visible(&grading_view(b))).collect();
        if attempts_on.0 != block.id {
            attempts_on = (block.id.clone(), Vec::new());
        }
        if block.block_type == DynamicBlockType::MetacognitiveClosure {
            let answer = ask_student(&http, &student_key, topic, &seen, &grading_view(&block), &[]).await;
            let reflection = answer.get("reflection").and_then(|v| v.as_str()).unwrap_or("Aprendí cosas nuevas hoy en esta clase sobre el tema.").to_string();
            let graded = svc.submit_closure_feedback(None, &block.id, &reflection).await;
            steps.push(json!({
                "blockId": block.id, "order": block.order_index + 1, "blockType": "metacognitive_closure", "studentAnswer": reflection,
                "passed": graded.as_ref().map(|g| g.passed).ok(), "feedback": graded.as_ref().map(|g| g.feedback.clone()).ok(),
                "error": graded.err().map(|e| e.to_string()),
            }));
            break;
        }
        if !block.block_type.is_gate() {
            // A non-gate block that is still the cursor means its successor is generating.
            tokio::time::sleep(Duration::from_secs(3)).await;
            continue;
        }
        let answer = ask_student(&http, &student_key, topic, &seen, &grading_view(&block), &attempts_on.1).await;
        let Some(submission) = submission_for(&block, &answer) else {
            error = Some(format!("no submission shape for {}", block.block_type.as_str()));
            break;
        };
        let t = Instant::now();
        let result = svc.submit_gate_response(None, &block.id, submission).await;
        let step = match &result {
            Ok(r) => json!({
                "blockId": block.id, "order": block.order_index + 1, "blockType": block.block_type.as_str(),
                "attempt": block.attempt_count + 1, "studentAnswer": answer, "passed": r.passed, "feedback": r.feedback,
                "escalated": r.escalation_block.is_some(), "gradeSeconds": t.elapsed().as_secs(),
            }),
            Err(e) => json!({ "blockId": block.id, "order": block.order_index + 1, "blockType": block.block_type.as_str(), "studentAnswer": answer, "error": e.to_string() }),
        };
        attempts_on.1.push(json!({ "answer": answer, "feedback": step.get("feedback") }));
        println!("[{}] #{} {} intento {} → {}", topic.key, block.order_index + 1, block.block_type.as_str(), block.attempt_count + 1, step.get("passed").map_or("error".into(), |p| p.to_string()));
        steps.push(step);
        if let Err(e) = result {
            error = Some(format!("submit: {e}"));
        }
        if steps.len() >= MAX_STEPS * 2 {
            error = Some("too many steps".into());
        }
    }

    // The full, UNREDACTED class as stored — what the reviewer reads.
    let payload = svc.store.load_notebook_by_class(&class.id).ok().flatten();
    let blocks: Vec<Value> = payload
        .as_ref()
        .map(|p| {
            p.blocks
                .iter()
                .map(|b| json!({ "order": b.order_index + 1, "blockType": b.block_type.as_str(), "status": status_name(b.status), "attempts": b.attempt_count, "content": b.content_json }))
                .collect()
        })
        .unwrap_or_default();
    let report = json!({
        "topic": topic.key, "course": topic.course, "class": topic.class,
        "minutes": (started.elapsed().as_secs_f64() / 60.0 * 10.0).round() / 10.0,
        "error": error, "sequence": blocks.iter().map(|b| format!("{}:{}", b["blockType"].as_str().unwrap_or("?"), b["status"].as_str().unwrap_or("?"))).collect::<Vec<_>>(),
        "steps": steps, "blocks": blocks,
        "rationale": payload.as_ref().and_then(|p| p.document.pedagogical_rationale.clone()),
    });
    std::fs::write(out_dir.join(format!("{}.json", topic.key)), serde_json::to_string_pretty(&report).expect("json")).expect("write report");
    println!("[{}] terminado en {} min — {:?}", topic.key, report["minutes"], report["sequence"]);
    report
}

/// The block as the UI receives it (answer keys removed).
fn grading_view(b: &NotebookBlock) -> NotebookBlock {
    super::grading::redact_block(b)
}

fn status_name(s: BlockStatus) -> &'static str {
    match s {
        BlockStatus::Passed => "passed",
        BlockStatus::Failed => "failed",
        BlockStatus::Escalated => "escalated",
        _ => "ready",
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "real model calls: LEARNKIT_CLASS_QA=1 cargo test --lib class_qa -- --ignored --nocapture"]
async fn class_qa() {
    if std::env::var("LEARNKIT_CLASS_QA").is_err() {
        return;
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    load_dotenv(&root);
    let student_key = std::env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY for the simulated student");
    let out_dir = root.join("eval-results").join("class-qa");
    std::fs::create_dir_all(&out_dir).expect("out dir");
    let only: Option<Vec<String>> = std::env::var("LEARNKIT_CLASS_QA_TOPICS").ok().map(|s| s.split(',').map(|t| t.trim().to_string()).collect());
    let selected: Vec<&'static Topic> = TOPICS.iter().filter(|t| only.as_ref().is_none_or(|o| o.iter().any(|k| k == t.key))).collect();
    let runs = selected.into_iter().map(|t| tokio::spawn(run_topic(t, out_dir.clone(), student_key.clone())));
    for r in futures::future::join_all(runs).await {
        if let Err(e) = r {
            println!("topic panicked: {e}");
        }
    }
}
