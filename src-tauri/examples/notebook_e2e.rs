//! Headless end-to-end run of the NOTEBOOK engine against a REAL model.
//!
//! Seeds a realistic absolute-beginner course ("Linux: montar mi primer
//! servidor") straight into SQLite (no roadmap-agent tokens spent), then
//! opens class 1 through the real `NotebookService` API and walks it block by
//! block with a scripted student:
//!   - prediction gates: wrong first (any mode), then correct;
//!   - branching scenario: wrong branch, then the optimal one;
//!   - heuristic error audit: faulty line + diagnosis from `modelSolution`;
//!   - first hands-on mission: ONE voluntary "paso más fácil" request, then
//!     reasonable solutions (up to 3 attempts, escalation accepted);
//!   - closure: a short reflection.
//! It reads raw blocks (answer keys included) straight from the store because
//! this is a harness. Afterwards it prints a CHECKS report. The exit code is
//! 0 even when checks FAIL; only infrastructure errors are non-zero.
//!
//! Like `roadmap_e2e`, it never touches the real vault / agents.json: the key
//! comes from an env var into an in-memory vault and every store is a temp dir.
//!
//! Run (PowerShell):
//!   $env:LEARNKIT_E2E_API_KEY="<key>"; cargo run --example notebook_e2e
//! Optional env: LEARNKIT_E2E_PROVIDER (default xiaomi), LEARNKIT_E2E_MODEL
//! (default mimo-v2.6-flash), LEARNKIT_E2E_MAX_BLOCKS (default 14),
//! LEARNKIT_E2E_WALL_SECS (default 3600), LEARNKIT_PROMPT_TIMEOUT_SECS (default
//! 300 here; the app default stays 120), LEARNKIT_E2E_STALL_RETRIES (default 3).
//! With provider xiaomi only `mimo-v2.6-pro` / `mimo-v2.6-flash` are accepted.

use std::collections::{BTreeMap, HashSet};

#[path = "e2e_support/mod.rs"]
mod e2e_support;
use std::sync::Arc;
use std::time::{Duration, Instant};

use learnkit_lib::agents::{
    analyst_agent, block_generator_agent, closure_feedback_grader_agent, lexical_assistant_agent, notebook_gate_grader_agent,
    pedagogical_critic_agent, roadmap_agent, AgentRegistry,
};
use learnkit_lib::application::NotebookService;
use learnkit_lib::dev_trace;
use learnkit_lib::domain::model::ModelRef;
use learnkit_lib::domain::notebook::{BlockStatus, DynamicBlockType, GateSubmission, NotebookBlock};
use learnkit_lib::domain::roadmap::{
    CapstoneProject, Deliverable, DeliverableArtifactType, Micromodule, Milestone, RoadmapSyllabusPackage,
};
use learnkit_lib::notebook_store::{import_syllabus_into_store, NotebookStore};
use learnkit_lib::orchestration::Orchestrator;
use learnkit_lib::providers::ProviderRegistry;
use learnkit_lib::secrets::{MemoryVault, SecretVault};
use serde_json::Value;

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

const COURSE_GOAL: &str = "Montar mi primer servidor Linux desde cero: soy principiante absoluto, sin conocimiento previo de terminal ni de redes.";

fn micromodule(label: &str, focus: &str, objective: &str, deliverable: &str, blocks: &[&str]) -> Micromodule {
    Micromodule {
        label: label.to_string(),
        hours: 2.0,
        focus: Some(focus.to_string()),
        deliverable: Deliverable { artifact_type: DeliverableArtifactType::WorkingDemo, description: deliverable.to_string() },
        objective: Some(objective.to_string()),
        interactive_blocks: blocks.iter().map(|b| b.to_string()).collect(),
    }
}

/// A deterministic beginner syllabus, shaped like what the roadmap agent seals.
fn beginner_syllabus() -> RoadmapSyllabusPackage {
    let blocks = ["anchored_micro_theory", "interactive_prediction_gate", "hands_on_mission", "metacognitive_closure"];
    RoadmapSyllabusPackage {
        course_title: "Linux: montar mi primer servidor".to_string(),
        total_weeks: 6,
        pace_hours_per_week: 5.0,
        milestones: vec![
            Milestone {
                week: 1,
                title: "Perder el miedo a la terminal".to_string(),
                deliverable: "Moverte por el sistema de archivos y crear tus primeros archivos desde la terminal".to_string(),
                weekly_goal: Some("Usar la terminal para navegar, crear y leer archivos sin ayuda".to_string()),
                micromodules: vec![
                    micromodule(
                        "Días 1-2 — Tu primera sesión en la terminal",
                        "Qué es la terminal y cómo navegar carpetas con pwd, ls y cd",
                        "Navegar entre carpetas y listar su contenido desde la terminal",
                        "Una carpeta de práctica creada y navegada desde la terminal",
                        &blocks,
                    ),
                    micromodule(
                        "Días 3-4 — Archivos y permisos básicos",
                        "Crear, copiar y leer archivos; qué significan los permisos rwx",
                        "Crear archivos y explicar quién puede leerlos o ejecutarlos",
                        "Un archivo de texto con permisos ajustados a propósito",
                        &blocks,
                    ),
                ],
            },
            Milestone {
                week: 2,
                title: "Tu servidor en marcha".to_string(),
                deliverable: "Un servidor web mínimo accesible desde otra máquina".to_string(),
                weekly_goal: Some("Instalar un servicio y comprobar que responde".to_string()),
                micromodules: vec![micromodule(
                    "Días 1-2 — Instalar y arrancar un servicio",
                    "Paquetes, servicios y puertos a nivel práctico",
                    "Instalar un servidor web y comprobar que responde",
                    "Un servidor web respondiendo en el puerto 80",
                    &blocks,
                )],
            },
        ],
        capstone_project: CapstoneProject {
            title: "Mi primer servidor publicado".to_string(),
            description: "Configurar un servidor Linux con un sitio web sencillo y acceso seguro por SSH".to_string(),
            verifiable_evidence: "Captura del sitio accesible desde otro dispositivo + registro de comandos usados".to_string(),
        },
    }
}

// --- Content helpers (raw `content_json`, camelCase) -----------------------

fn str_of<'a>(c: &'a Value, key: &str) -> &'a str {
    c.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

fn str_list(c: &Value, key: &str) -> Vec<String> {
    c.get(key).and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default()
}

fn clip(s: &str, n: usize) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= n { flat } else { format!("{}…", flat.chars().take(n).collect::<String>()) }
}

/// `supportLevel` of a mission / `interactionMode` of a prediction gate.
fn mode_of(b: &NotebookBlock) -> String {
    match b.block_type {
        DynamicBlockType::HandsOnMission => {
            let lvl = str_of(&b.content_json, "supportLevel");
            let transfer = b.content_json.get("isTransfer").and_then(|v| v.as_bool()).unwrap_or(false);
            format!("support={}{}", if lvl.is_empty() { "(ausente)" } else { lvl }, if transfer { " transfer" } else { "" })
        }
        DynamicBlockType::InteractivePredictionGate => {
            let m = str_of(&b.content_json, "interactionMode");
            format!("mode={}", if m.is_empty() { "multiple_choice" } else { m })
        }
        _ => String::new(),
    }
}

fn is_unresolved_gate(b: &NotebookBlock) -> bool {
    b.block_type.is_gate() && matches!(b.status, BlockStatus::Ready | BlockStatus::Failed)
}

// --- Scripted student -------------------------------------------------------

fn wrong_submission(b: &NotebookBlock) -> GateSubmission {
    let c = &b.content_json;
    match b.block_type {
        DynamicBlockType::InteractivePredictionGate => match str_of(c, "interactionMode") {
            "causal_chain_order" => {
                let mut steps = str_list(c, "shuffledSteps");
                if steps == str_list(c, "correctOrder") {
                    steps.reverse();
                }
                GateSubmission::InteractivePredictionGate { selected_option: None, ordered_steps: Some(steps), case_verdicts: None }
            }
            "boundary_case_audit" | "boundary_case_Audit" => {
                let key: Vec<bool> = c
                    .get("boundaryCases")
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().map(|x| x.get("followsRule").and_then(|v| v.as_bool()).unwrap_or(false)).collect())
                    .unwrap_or_default();
                GateSubmission::InteractivePredictionGate {
                    selected_option: None,
                    ordered_steps: None,
                    case_verdicts: Some(key.iter().map(|k| !k).collect()),
                }
            }
            _ => {
                let correct = str_of(c, "correctOption");
                let pick = str_list(c, "options").into_iter().find(|o| o != correct).unwrap_or_else(|| "ninguna".to_string());
                GateSubmission::InteractivePredictionGate { selected_option: Some(pick), ordered_steps: None, case_verdicts: None }
            }
        },
        _ => {
            // branching_scenario_challenge: a non-optimal branch.
            let pick = c
                .get("branches")
                .and_then(|v| v.as_array())
                .and_then(|a| a.iter().find(|x| !x.get("isOptimal").and_then(|v| v.as_bool()).unwrap_or(false)))
                .map(|x| str_of(x, "choice").to_string())
                .unwrap_or_else(|| "ninguna".to_string());
            GateSubmission::BranchingScenarioChallenge { selected_choice: pick }
        }
    }
}

fn right_submission(b: &NotebookBlock) -> GateSubmission {
    let c = &b.content_json;
    match b.block_type {
        DynamicBlockType::InteractivePredictionGate => match str_of(c, "interactionMode") {
            "causal_chain_order" => {
                GateSubmission::InteractivePredictionGate { selected_option: None, ordered_steps: Some(str_list(c, "correctOrder")), case_verdicts: None }
            }
            "boundary_case_audit" | "boundary_case_Audit" => {
                let key: Vec<bool> = c
                    .get("boundaryCases")
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().map(|x| x.get("followsRule").and_then(|v| v.as_bool()).unwrap_or(false)).collect())
                    .unwrap_or_default();
                GateSubmission::InteractivePredictionGate { selected_option: None, ordered_steps: None, case_verdicts: Some(key) }
            }
            _ => GateSubmission::InteractivePredictionGate {
                selected_option: Some(str_of(c, "correctOption").to_string()),
                ordered_steps: None,
                case_verdicts: None,
            },
        },
        DynamicBlockType::BranchingScenarioChallenge => {
            let pick = c
                .get("branches")
                .and_then(|v| v.as_array())
                .and_then(|a| a.iter().find(|x| x.get("isOptimal").and_then(|v| v.as_bool()).unwrap_or(false)))
                .map(|x| str_of(x, "choice").to_string())
                .unwrap_or_default();
            GateSubmission::BranchingScenarioChallenge { selected_choice: pick }
        }
        DynamicBlockType::HeuristicErrorAudit => GateSubmission::HeuristicErrorAudit {
            diagnosis_text: str_of(c, "modelSolution").to_string(),
            selected_line: c.get("faultyLineNumber").and_then(|v| v.as_u64()).map(|n| n as u32),
        },
        _ => GateSubmission::HandsOnMission { submission_text: mission_solution(c), hints_shown: Some(0) },
    }
}

/// A plausible beginner solution derived from the mission's own scaffolding.
fn mission_solution(c: &Value) -> String {
    let mut text = format!("Mi solución para: {}\n", clip(str_of(c, "challengeStatement"), 160));
    let template = str_of(c, "workedTemplate");
    if !template.is_empty() {
        let tokens: Vec<String> = c
            .get("interactiveSteps")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().map(|s| str_of(s, "expectedToken").to_string()).collect())
            .unwrap_or_default();
        let mut filled = String::new();
        let mut it = tokens.iter();
        for (i, part) in template.split("___").enumerate() {
            if i > 0 {
                filled.push_str(it.next().map(String::as_str).unwrap_or("completado"));
            }
            filled.push_str(part);
        }
        text.push_str(&filled);
        text.push('\n');
    }
    text.push_str(&format!("Resultado obtenido: {}\n", str_of(c, "expectedMilestoneArtifact")));
    for constraint in str_list(c, "constraints") {
        text.push_str(&format!("Cumplo la restricción: {constraint}\n"));
    }
    text
}

// --- Check helpers ------------------------------------------------------------

/// `n` followed by `.` or `)` and not preceded by another digit ("1." / "2)").
fn has_step_marker(text: &str, n: u32) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let digit = char::from_digit(n, 10).unwrap_or('1');
    (0..chars.len().saturating_sub(1)).any(|i| {
        chars[i] == digit && matches!(chars[i + 1], '.' | ')') && (i == 0 || !chars[i - 1].is_ascii_digit())
    })
}

fn normalize(s: &str) -> String {
    s.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

fn verdict(ok: Option<bool>) -> &'static str {
    match ok {
        Some(true) => "PASS",
        Some(false) => "FAIL",
        None => "N-A",
    }
}

fn check(label: &str, ok: Option<bool>, detail: &str) {
    println!("[{}] {label} — {detail}", verdict(ok));
}

#[tokio::main]
async fn main() -> AnyResult<()> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::new("learnkit=warn")).init();
    println!("== Notebook engine — headless e2e (clase 1, estudiante scripteado) ==");

    let Ok(api_key) = std::env::var("LEARNKIT_E2E_API_KEY") else {
        eprintln!("Set LEARNKIT_E2E_API_KEY (and optionally LEARNKIT_E2E_PROVIDER / LEARNKIT_E2E_MODEL) to run this.");
        return Ok(());
    };
    let provider_id = std::env::var("LEARNKIT_E2E_PROVIDER").unwrap_or_else(|_| "xiaomi".to_string());
    let model = std::env::var("LEARNKIT_E2E_MODEL").ok().filter(|m| !m.trim().is_empty()).unwrap_or_else(|| "mimo-v2.6-flash".to_string());
    e2e_support::enforce_model_policy(&provider_id, &model);
    let timeout_secs = e2e_support::ensure_prompt_timeout();
    let stall_retries: u32 = std::env::var("LEARNKIT_E2E_STALL_RETRIES").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    // One full loop_runner budget (3 attempts) plus slack before declaring a stall.
    let stall_window = Duration::from_secs(timeout_secs * 3 + 60);
    println!("Ventana de estancamiento: {}s; reintentos de estancamiento: {stall_retries}", stall_window.as_secs());
    let max_blocks: usize = std::env::var("LEARNKIT_E2E_MAX_BLOCKS").ok().and_then(|v| v.parse().ok()).unwrap_or(14);
    let wall_cap = Duration::from_secs(std::env::var("LEARNKIT_E2E_WALL_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(3600));
    let started = Instant::now();

    let memory_vault = MemoryVault::new();
    memory_vault.save_provider_key(&provider_id, &api_key).expect("seed in-memory vault");
    let vault: Arc<dyn SecretVault> = Arc::new(memory_vault);
    let providers = Arc::new(ProviderRegistry::with_defaults());
    println!("Proveedor: {provider_id} (modelo: {model}); máx. bloques: {max_blocks}\n");

    // Every agent the notebook path (and anything near it) can touch follows the provider/model.
    let agents = Arc::new(AgentRegistry::new());
    for mut def in [
        block_generator_agent::definition(),
        pedagogical_critic_agent::definition(),
        notebook_gate_grader_agent::definition(),
        closure_feedback_grader_agent::definition(),
        roadmap_agent::definition(),
        analyst_agent::definition(),
        lexical_assistant_agent::definition(),
    ] {
        def.model = ModelRef::new(provider_id.clone(), model.as_str());
        agents.register(def).expect("register agent");
    }
    let orchestrator = Arc::new(Orchestrator::with_rig_runner(agents, providers, vault));

    let tmp_dir = std::env::temp_dir().join(format!("learnkit-nb-e2e-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp_dir)?;
    let store = NotebookStore::open(tmp_dir.join("notebook.sqlite3"))?;
    let service = NotebookService::new(store.clone(), orchestrator);

    // Developer-mode traces give us model-call counts and critic verdicts.
    dev_trace::clear();
    dev_trace::set_enabled(true);
    let mut collector = e2e_support::TraceCollector::new();

    let (course_id, first_class_id, _) = import_syllabus_into_store(&store, &beginner_syllabus(), COURSE_GOAL)?;
    let class_id = first_class_id.ok_or("seeded course has no first class")?;
    println!("Curso sembrado ({course_id}), clase 1 = {class_id}\n");

    // --- Drive the notebook -------------------------------------------------
    let mut reported = 0usize;
    let mut gate_log: Vec<String> = Vec::new();
    let mut easier_used = false;
    let mut easier_escalation: Option<(bool, String)> = None; // (produced, description)
    let mut stop_reason = "cierre completado".to_string();
    let mut last_growth = Instant::now();
    let mut stall_retries_used = 0u32;

    let first = service.start_class_notebook(None, &class_id).await?;
    println!("start_class_notebook -> {} bloque(s) iniciales ({:.1}s)\n", first.blocks.len(), started.elapsed().as_secs_f32());

    let doc_id = loop {
        if started.elapsed() > wall_cap {
            stop_reason = "tope de tiempo (LEARNKIT_E2E_WALL_SECS)".to_string();
            break first.document.id.clone();
        }
        collector.poll();
        let Some(mut payload) = store.load_notebook_by_class(&class_id)? else {
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        };
        payload.blocks.sort_by_key(|b| b.order_index);
        let blocks = payload.blocks.clone();

        for b in blocks.iter().skip(reported) {
            println!(
                "BLOQUE #{} {} {} (estado={:?}, +{:.0}s)",
                b.order_index,
                b.block_type.as_str(),
                mode_of(b),
                b.status,
                started.elapsed().as_secs_f32()
            );
            reported += 1;
            last_growth = Instant::now();
            stall_retries_used = 0;
        }

        if blocks.len() >= max_blocks {
            stop_reason = format!("tope de {max_blocks} bloques");
            break payload.document.id.clone();
        }

        // 1) An unresolved gate is the active block: answer it.
        if let Some(gate) = blocks.iter().find(|b| is_unresolved_gate(b)) {
            let tag = format!("#{} {} {}", gate.order_index, gate.block_type.as_str(), mode_of(gate));
            let is_mission = gate.block_type == DynamicBlockType::HandsOnMission;

            if is_mission && !easier_used {
                easier_used = true;
                let r = service.request_easier_step(None, &gate.id, Some(1), None).await?;
                let desc = r
                    .escalation_block
                    .as_ref()
                    .map(|e| format!("#{} {} {}", e.order_index, e.block_type.as_str(), mode_of(e)))
                    .unwrap_or_else(|| "(sin bloque)".to_string());
                easier_escalation = Some((r.escalation_block.is_some(), desc.clone()));
                let line = format!("{tag}: paso más fácil voluntario (hints_shown=1) -> escalación: {desc}");
                println!("  {line}");
                gate_log.push(line);
                continue;
            }

            let wrong_first = gate.attempt_count == 0
                && matches!(
                    gate.block_type,
                    DynamicBlockType::InteractivePredictionGate | DynamicBlockType::BranchingScenarioChallenge
                );
            let submission = if wrong_first { wrong_submission(gate) } else { right_submission(gate) };
            let r = service.submit_gate_response(None, &gate.id, submission).await?;
            let line = format!(
                "{tag}: intento {} ({}) -> {}{}{}",
                gate.attempt_count + 1,
                if wrong_first { "deliberadamente incorrecto" } else { "correcto/razonable" },
                if r.passed { "APROBADO" } else { "no aprobado" },
                r.feedback.as_deref().map(|f| format!(" | pista: {}", clip(f, 110))).unwrap_or_default(),
                r.escalation_block
                    .as_ref()
                    .map(|e| format!(" | ESCALACIÓN -> #{} {}", e.order_index, e.block_type.as_str()))
                    .unwrap_or_default(),
            );
            println!("  {line}");
            gate_log.push(line);
            continue;
        }

        // 2) The closing block: submit a reflection (max 2 tries).
        if let Some(closure) = blocks.iter().find(|b| b.block_type == DynamicBlockType::MetacognitiveClosure) {
            if closure.status != BlockStatus::Passed && closure.attempt_count < 2 {
                let reflection = "Al principio pensaba que bastaba con memorizar comandos. Ahora entiendo que primero hay que \
                    saber dónde estoy (pwd), qué hay aquí (ls) y moverme (cd). Pude completar la misión con ayuda y me quedó claro \
                    el orden de los pasos; todavía debo practicar los permisos.";
                let r = service.submit_closure_feedback(None, &closure.id, reflection).await?;
                let line = format!("cierre: intento {} -> {} | {}", closure.attempt_count + 1, if r.passed { "APROBADO" } else { "no aprobado" }, clip(&r.feedback, 140));
                println!("  {line}");
                gate_log.push(line);
                continue;
            }
            break payload.document.id.clone();
        }

        // 3) Nothing to answer: wait for the background chain (bounded retries on a stall).
        if last_growth.elapsed() > stall_window {
            if stall_retries_used >= stall_retries {
                stop_reason = format!("estancado: no se generó el siguiente bloque tras {stall_retries} reintento(s)");
                break payload.document.id.clone();
            }
            stall_retries_used += 1;
            println!(
                "  [{}] (sin bloques nuevos en {}s) retry_pending_block {stall_retries_used}/{stall_retries} (+{:.0}s)",
                e2e_support::stamp(),
                stall_window.as_secs(),
                started.elapsed().as_secs_f32()
            );
            if let Err(e) = service.retry_pending_block(None, &payload.document.id).await {
                println!("  retry_pending_block falló: {e}");
            }
            last_growth = Instant::now();
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    };

    // --- Final state ----------------------------------------------------------
    let mut payload = store.load_notebook_by_class(&class_id)?.ok_or("notebook disappeared")?;
    payload.blocks.sort_by_key(|b| b.order_index);
    let blocks = payload.blocks;
    let attempts = store.list_gate_attempts(&doc_id)?;

    println!("\n======================================================================");
    println!("RESUMEN POR BLOQUE (parada: {stop_reason})");
    println!("======================================================================");
    for b in &blocks {
        println!(
            "#{:<2} {:<30} {:<28} estado={:?} intentos={}",
            b.order_index,
            b.block_type.as_str(),
            mode_of(b),
            b.status,
            b.attempt_count
        );
    }
    println!("\n--- resultados de compuertas (por intento) ---");
    for l in &gate_log {
        println!("  {l}");
    }

    // --- Trace summary (model calls, critic verdicts) -------------------------
    let trace = dev_trace::snapshot();
    let mut verdicts: BTreeMap<String, u32> = BTreeMap::new();
    for e in &trace {
        if matches!(e.kind.as_str(), "critique" | "retry") {
            *verdicts.entry(format!("{}:{}", e.kind, e.verdict.clone().unwrap_or_default())).or_default() += 1;
        }
    }
    collector.report();
    println!("--- veredictos de crítica / reintentos ---");
    for (v, n) in &verdicts {
        println!("  {v}: {n}");
    }
    println!("--- cronología de críticas (bloque candidato: veredicto — primera nota) ---");
    for e in trace.iter().filter(|e| e.kind == "critique" && e.flow.as_deref() == Some("block_critique")) {
        println!("  {}: {} — {}", e.tool.clone().unwrap_or_default(), e.verdict.clone().unwrap_or_default(), clip(e.notes.first().map(String::as_str).unwrap_or(""), 140));
    }

    // --- CHECKS -----------------------------------------------------------------
    println!("\n======================================================================");
    println!("CHECKS");
    println!("======================================================================");

    // a) first block: theory (or retrieval) with numbered steps and an example.
    match blocks.first() {
        None => check("a) primer bloque", None, "no se generó ningún bloque"),
        Some(first_block) => {
            let t = first_block.block_type;
            let rule = str_of(&first_block.content_json, "systemRule");
            let type_ok = matches!(t, DynamicBlockType::AnchoredMicroTheory | DynamicBlockType::SpacedInterleavedRetrieval);
            let steps_ok = has_step_marker(rule, 1) && has_step_marker(rule, 2);
            let lower = rule.to_lowercase();
            // A literal input/output transcript (a "$ cmd" or "> expr" line) is
            // the strongest form of concrete example, so it counts too.
            let shows_transcript = rule.lines().any(|l| l.trim_start().starts_with("$ ") || l.trim_start().starts_with("> "));
            let example_ok = lower.contains("ejemplo")
                || lower.contains("example")
                || rule.contains('`')
                || lower.contains("p. ej")
                || shows_transcript;
            if t == DynamicBlockType::AnchoredMicroTheory {
                check(
                    "a) primer bloque = teoría anclada con pasos numerados y ejemplo",
                    Some(type_ok && steps_ok && example_ok),
                    &format!("tipo={}, pasos 1./2.={steps_ok}, ejemplo={example_ok}", t.as_str()),
                );
            } else {
                check(
                    "a) primer bloque = teoría o retrieval",
                    Some(type_ok),
                    &format!("tipo={} (pasos/ejemplo solo aplican a teoría)", t.as_str()),
                );
            }
        }
    }

    // b) the block right after the first theory.
    let theory_idx = blocks.iter().position(|b| b.block_type == DynamicBlockType::AnchoredMicroTheory);
    match theory_idx.and_then(|i| blocks.get(i + 1)) {
        None => check("b) bloque tras la primera teoría", None, "no existe teoría o bloque siguiente"),
        Some(next) => {
            let independent = next.block_type == DynamicBlockType::HandsOnMission
                && (str_of(&next.content_json, "supportLevel") == "independent"
                    || next.content_json.get("isTransfer").and_then(|v| v.as_bool()).unwrap_or(false));
            let bad = next.block_type == DynamicBlockType::HeuristicErrorAudit || independent;
            check("b) tras la primera teoría no hay audit ni misión independiente", Some(!bad), &format!("siguiente = {} {}", next.block_type.as_str(), mode_of(next)));
        }
    }

    // c) missions declare supportLevel; guided/faded carry scaffolding; independent only for transfer.
    let missions: Vec<&NotebookBlock> = blocks.iter().filter(|b| b.block_type == DynamicBlockType::HandsOnMission).collect();
    if missions.is_empty() {
        check("c) misiones", None, "no se generó ninguna misión");
    } else {
        let mut problems: Vec<String> = Vec::new();
        for m in &missions {
            let c = &m.content_json;
            let lvl = str_of(c, "supportLevel");
            let transfer = c.get("isTransfer").and_then(|v| v.as_bool()).unwrap_or(false);
            if lvl.is_empty() {
                problems.push(format!("#{} sin supportLevel", m.order_index));
            }
            if matches!(lvl, "guided" | "faded") {
                let has_template = !str_of(c, "workedTemplate").is_empty();
                let has_steps = c.get("interactiveSteps").and_then(|v| v.as_array()).is_some_and(|a| !a.is_empty());
                if !has_template && !has_steps {
                    problems.push(format!("#{} {lvl} sin workedTemplate ni interactiveSteps", m.order_index));
                }
            }
            if lvl == "independent" && !transfer {
                problems.push(format!("#{} independent sin ser transfer", m.order_index));
            }
        }
        check("c) misiones: supportLevel y andamiaje coherentes", Some(problems.is_empty()), &format!("{} misión(es); problemas: {problems:?}", missions.len()));
    }

    // d) voluntary easier step produced an escalation block.
    match &easier_escalation {
        None => check("d) paso más fácil voluntario", None, "nunca apareció una misión para pedirlo"),
        Some((produced, desc)) => check("d) paso más fácil voluntario generó bloque de escalación", Some(*produced), desc),
    }

    // e) gate mechanics variety.
    let kinds: HashSet<String> = blocks
        .iter()
        .filter(|b| b.block_type.is_gate())
        .map(|b| match b.block_type {
            DynamicBlockType::InteractivePredictionGate => mode_of(b),
            t => t.as_str().to_string(),
        })
        .collect();
    let mut sorted_kinds: Vec<&String> = kinds.iter().collect();
    sorted_kinds.sort();
    check("e) variedad de mecánicas de compuerta (umbral >= 3)", Some(kinds.len() >= 3), &format!("{} distintas: {sorted_kinds:?}", kinds.len()));

    // f) closure's initialPrediction quotes the student's real first gate attempt.
    let closure = blocks.iter().find(|b| b.block_type == DynamicBlockType::MetacognitiveClosure);
    match (closure, attempts.first()) {
        (Some(cl), Some(first_attempt)) => {
            let initial = normalize(cl.content_json.get("predictionComparison").and_then(|p| p.get("initialPrediction")).and_then(|v| v.as_str()).unwrap_or(""));
            let real = normalize(&first_attempt.answer_text);
            let fragment: String = real.chars().take(30).collect::<String>().trim().to_string();
            let ok = !fragment.is_empty() && (initial.contains(&fragment) || initial.contains(&real));
            check(
                "f) initialPrediction del cierre cita el primer intento real",
                Some(ok),
                &format!("intento real=\"{}\" | initialPrediction=\"{}\"", clip(&first_attempt.answer_text, 80), clip(&initial, 120)),
            );
        }
        (None, _) => check("f) initialPrediction del cierre", None, "no se llegó al bloque de cierre"),
        (_, None) => check("f) initialPrediction del cierre", None, "no hay intentos de compuerta registrados"),
    }

    // g) cost.
    let total_calls = collector.len();
    println!(
        "[INFO] g) tiempo total {:.0}s; llamadas al modelo (agent_run en traza) = {total_calls}; bloques = {}; parada = {stop_reason}",
        started.elapsed().as_secs_f32(),
        blocks.len()
    );

    dev_trace::set_enabled(false);
    let _ = std::fs::remove_dir_all(&tmp_dir);
    Ok(())
}
