//! Headless end-to-end runs of the Roadmap & Syllabus Diagnostic Agent,
//! against a REAL model (no UI, no ScriptedRunner) — for reviewing actual
//! prompt compliance (autonomy, meta-language, turn caps, tone) rather than
//! just gate *logic*, which the unit tests already cover with canned
//! responses.
//!
//! The agent is now fully autonomous (real tool-calling, no confirmation
//! step): the moment it has enough — topic, timeframe, weekly hours, entry
//! level — it must call `submit_diagnostic_assessment` then, once the
//! student confirms the proposed plan, `confirm_syllabus_plan`, sealing the
//! session and persisting the course (classes, no notebook content yet —
//! that's generated lazily per class, not at seal time) into SQLite, with
//! no "¿lo guardamos?" turn in between. This harness checks exactly that.
//!
//! Runs several DIFFERENT scenarios: a learner who gives everything up
//! front, one who needs a follow-up turn, one with prior experience in a
//! different domain, and a deliberately vague one meant to exercise the
//! hard turn-cap fallback for real. Prints a per-scenario transcript plus
//! an aggregate meta-analysis across all of them at the end.
//!
//! Deliberately does NOT touch the real app's Stronghold vault, agents.json,
//! or roadmap_sessions/ — the API key comes from an env var into a throwaway
//! in-memory vault that dies with this process, and the agent registry /
//! session store / notebook SQLite store are all fresh and disposable.
//!
//! Run: `LEARNKIT_E2E_API_KEY=<key> cargo run --example roadmap_e2e`
//! Optionally override the provider (default: deepseek): `LEARNKIT_E2E_PROVIDER=anthropic`
//! (e.g. `xiaomi` pins `mimo-v2.6-flash`), the model with `LEARNKIT_E2E_MODEL`,
//! and run a single scenario with `LEARNKIT_E2E_SCENARIO=<name substring>`
//! (case-insensitive). Live-run knobs: LEARNKIT_PROMPT_TIMEOUT_SECS (default 300
//! here; the app default stays 120), LEARNKIT_E2E_TURN_RETRIES (default 2).
//! With provider xiaomi only `mimo-v2.6-pro` / `mimo-v2.6-flash` are accepted.

use std::sync::Arc;

#[path = "e2e_support/mod.rs"]
mod e2e_support;

use learnkit_lib::agents::{analyst_agent, roadmap_agent, AgentRegistry};
use learnkit_lib::application::RoadmapService;
use learnkit_lib::domain::roadmap::SessionStatus;
use learnkit_lib::notebook_store::NotebookStore;
use learnkit_lib::orchestration::Orchestrator;
use learnkit_lib::persistence::FileStore;
use learnkit_lib::providers::ProviderRegistry;
use learnkit_lib::secrets::{MemoryVault, SecretVault};

const BANNED_TERMS: &[&str] = &[
    "fase 1", "fase 2", "fase 3", "compuerta", "gate 1", "gate 2", "gate 3",
    "dod", "definition of done", "backward design", "diseño inverso",
    "alineamiento constructivo", "tool call", "herramienta interna",
    // Agile/scrum jargon — these are study weeks for one person, not a
    // dev team's sprints.
    "sprint", "backlog", "standup", "daily", "retro ", "retrospectiva", "mvp",
    "kickoff", "historia de usuario", "user story",
    // The exact passive-confirmation phrasing this redesign was meant to kill.
    "¿lo guardamos", "¿empezamos", "¿seguimos", "¿lo confirmas",
];

// camelCase tool-arg field names — if any show up in the conversational
// text, the model leaked its internal data structure to the student instead
// of describing it in prose.
const FIELD_NAME_LEAKS: &[&str] = &[
    "targetGoal", "timeframeWeeks", "weeklyCommitmentHours", "entryLevel",
    "coreFocus", "identifiedNeeds", "learningStrategy", "courseTitle",
    "totalWeeks", "paceHoursPerWeek", "interactiveBlocks", "sectionType", "blockType",
];

fn looks_like_json(s: &str) -> bool {
    let brace_signal = s.contains('{') && s.contains('}') && s.contains(':');
    let field_leak = FIELD_NAME_LEAKS.iter().any(|f| s.contains(f));
    brace_signal || field_leak
}

struct Scenario {
    name: &'static str,
    student_turns: Vec<&'static str>,
}

fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "Todo el dato de una vez (principiante absoluto, proyecto universitario)",
            student_turns: vec![
                "Necesito terminar un proyecto universitario. Tengo 4 semanas y puedo dedicarle \
                 30 horas por semana. Soy principiante absoluto, no sé nada del tema: bases de \
                 datos con SQL.",
                "sí",
                "ok",
            ],
        },
        Scenario {
            name: "Dato repartido en 2 turnos, con experiencia previa (entrevista SQL)",
            student_turns: vec![
                "Quiero prepararme para una entrevista técnica de bases de datos. Ya sé \
                 programar pero de SQL solo lo básico.",
                "Tengo 4 semanas y puedo dedicarle unas 5 horas por semana.",
                "ok",
            ],
        },
        Scenario {
            name: "Dominio distinto — proyecto personal sin examen (Python/Excel)",
            student_turns: vec![
                "Quiero automatizar reportes de Excel con Python para mi trabajo. Ya sé algo \
                 de macros de Excel pero nunca programé en Python. Tengo 8 semanas y puedo \
                 dedicarle unas 5 horas semanales.",
                "ok",
            ],
        },
        Scenario {
            // The real-user beginner case: a messy first message, then plausible
            // follow-ups. After sealing we check the course/profile title is
            // clean and the diagnostic battery has no self-perception options.
            name: "Principiante real — primer servidor sin conocimiento previo",
            student_turns: vec![
                "Quiero montar mi primer servidor, y no tengo nada de conocimiento",
                "Tengo 6 semanas, 5 horas por semana",
                "sí",
                "ok",
            ],
        },
        Scenario {
            name: "Vago/monosilábico — fuerza el tope estricto de turnos",
            student_turns: vec!["no sé", "lo que sea", "cualquier cosa"],
        },
    ]
}

struct TurnLog {
    label: String,
    user_said: String,
    agent_said: String,
    status_after: SessionStatus,
}

/// Phrases that mean the battery asks the learner to rate themselves instead
/// of testing knowledge (compared lowercase).
const SELF_PERCEPTION_OPTIONS: &[&str] = &["no estoy seguro", "no todavía", "una idea concreta", "no tengo ninguna intuición"];

/// Post-seal checks: clean titles and an objective (or absent) diagnostic
/// battery. Reads the battery from the sealed package and from the SQLite
/// course row (the copy the app actually serves).
fn report_beginner_checks(session: &learnkit_lib::domain::roadmap::RoadmapSession, store: &NotebookStore) {
    println!("--- checks de título y batería diagnóstica ---");
    let Some(package) = session.roadmap_package.as_ref() else {
        println!("[N/A] sin paquete sellado.");
        return;
    };
    let titles = [("course_title", package.syllabus.course_title.as_str()), ("topic", package.learner_profile.topic.as_str())];
    for (label, title) in titles {
        let lower = title.to_lowercase();
        let dirty = lower.contains("quiero") || lower.contains("no tengo nada de conocimiento");
        println!("[{}] {label} limpio: \"{title}\"", if dirty { "FALLO" } else { "OK" });
    }

    let stored = session.imported_course_id.as_deref().and_then(|id| store.get_course_diagnostic_battery(id).ok().flatten());
    let batteries = [("paquete sellado", package.diagnostic_battery.clone()), ("SQLite", stored.map(|s| s.battery))];
    for (source, battery) in batteries {
        let Some(battery) = battery else {
            println!("[OK] batería ({source}): None.");
            continue;
        };
        let mut offending: Vec<String> = Vec::new();
        for q in &battery.questions {
            for opt in &q.options {
                let lower = opt.trim().to_lowercase();
                let bare_yes = matches!(lower.trim_end_matches(['.', '!']), "sí" | "si");
                if bare_yes || SELF_PERCEPTION_OPTIONS.iter().any(|p| lower.contains(p)) {
                    offending.push(format!("\"{opt}\" (en: {})", q.prompt));
                }
            }
        }
        if offending.is_empty() {
            println!("[OK] batería ({source}): {} preguntas, sin opciones de autopercepción.", battery.questions.len());
        } else {
            println!("[FALLO] batería ({source}): opciones de autopercepción:");
            for o in &offending {
                println!("  - {o}");
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new("learnkit=warn"))
        .init();

    println!("== Roadmap & Syllabus Diagnostic Agent — headless e2e (autonomous, multi-scenario) ==");

    let Ok(api_key) = std::env::var("LEARNKIT_E2E_API_KEY") else {
        eprintln!("Set LEARNKIT_E2E_API_KEY (and optionally LEARNKIT_E2E_PROVIDER) to run this.");
        return Ok(());
    };
    let provider_id = std::env::var("LEARNKIT_E2E_PROVIDER").unwrap_or_else(|_| "deepseek".to_string());

    const MODEL_FOR: &[(&str, &str)] = &[
        ("anthropic", "claude-sonnet-4-6"),
        ("deepseek", "deepseek-v4-flash"),
        ("openai", "gpt-5"),
        ("gemini", "gemini-2.5-flash"),
        ("openrouter", "anthropic/claude-sonnet-4"),
        ("xiaomi", "mimo-v2.6-flash"),
    ];
    let model: String = match std::env::var("LEARNKIT_E2E_MODEL") {
        Ok(m) if !m.trim().is_empty() => m.trim().to_string(),
        _ => MODEL_FOR
            .iter()
            .find(|(p, _)| *p == provider_id)
            .map(|(_, m)| m.to_string())
            .unwrap_or_else(|| panic!("no model pin for provider '{provider_id}'")),
    };
    e2e_support::enforce_model_policy(&provider_id, &model);
    e2e_support::ensure_prompt_timeout();
    let turn_retries: u32 = std::env::var("LEARNKIT_E2E_TURN_RETRIES").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
    println!("Reintentos por turno fallido: {turn_retries}");
    let scenario_filter = std::env::var("LEARNKIT_E2E_SCENARIO").ok().map(|f| f.to_lowercase()).filter(|f| !f.is_empty());

    let memory_vault = MemoryVault::new();
    memory_vault.save_provider_key(&provider_id, &api_key).expect("seed in-memory vault");
    let vault: Arc<dyn SecretVault> = Arc::new(memory_vault);
    let providers = Arc::new(ProviderRegistry::with_defaults());
    println!("Using provider: {provider_id} (model: {model})\n");

    let agents = Arc::new(AgentRegistry::new());
    // Gate 1 runs through the dedicated `diagnostic_analyst` agent, so both
    // agents must be registered on the same provider/model.
    for mut def in [roadmap_agent::definition(), analyst_agent::definition()] {
        def.model = learnkit_lib::domain::model::ModelRef::new(provider_id.clone(), model.as_str());
        agents.register(def).expect("register roadmap agents");
    }

    let orchestrator = Arc::new(Orchestrator::with_rig_runner(Arc::clone(&agents), providers, vault));
    let tmp_dir = std::env::temp_dir().join(format!("learnkit-e2e-{}", uuid::Uuid::new_v4()));
    let store = FileStore::new(tmp_dir.clone());
    let notebook_store = NotebookStore::open(tmp_dir.join("notebook.sqlite3"))?;
    let service = RoadmapService::new(orchestrator, store, notebook_store.clone());

    learnkit_lib::dev_trace::set_enabled(true);

    let mut all_jargon_hits = 0usize;
    let mut all_json_leaks = 0usize;
    let mut sealed_count = 0usize;
    let mut sealed_within_2_turns = 0usize;
    let selected: Vec<Scenario> = scenarios()
        .into_iter()
        .filter(|sc| scenario_filter.as_ref().map_or(true, |f| sc.name.to_lowercase().contains(f)))
        .collect();
    if selected.is_empty() {
        eprintln!("LEARNKIT_E2E_SCENARIO matched no scenario.");
        return Ok(());
    }
    let total_scenarios = selected.len();

    for scenario in selected {
        println!("======================================================================");
        println!("ESCENARIO: {}", scenario.name);
        println!("======================================================================\n");

        // Fresh trace + collector per scenario so the stats are scenario-scoped.
        learnkit_lib::dev_trace::clear();
        let mut collector = e2e_support::TraceCollector::new();
        let mut log: Vec<TurnLog> = Vec::new();
        let mut aborted: Option<String> = None;
        let started_session = service.start_session(None).await;
        let (session_id, mut session) = match started_session {
            Ok(r) => {
                log.push(TurnLog {
                    label: "start_session".to_string(),
                    user_said: "(apertura de sesión)".to_string(),
                    agent_said: r.message.clone(),
                    status_after: r.session.status,
                });
                (r.session.session_id.clone(), r.session)
            }
            Err(e) => {
                println!("[FALLO] start_session falló: {e}");
                println!("
--- estadísticas del escenario (abortado) ---");
                collector.report();
                println!();
                continue;
            }
        };

        let mut i = 0;
        while session.status != SessionStatus::Sealed && i < scenario.student_turns.len() {
            let user_msg = scenario.student_turns[i];
            let mut result = service.send_message(None, &session_id, user_msg).await;
            let mut attempt = 0u32;
            while let Err(e) = &result {
                attempt += 1;
                if attempt > turn_retries {
                    break;
                }
                println!("  [{}] turno #{} falló ({e}); retry_pending_turn {attempt}/{turn_retries}", e2e_support::stamp(), i + 1);
                result = service.retry_pending_turn(None, &session_id).await;
            }
            match result {
                Ok(r) => {
                    log.push(TurnLog {
                        label: format!("send_message #{}", i + 1),
                        user_said: user_msg.to_string(),
                        agent_said: r.message.clone(),
                        status_after: r.session.status,
                    });
                    session = r.session;
                    i += 1;
                }
                Err(e) => {
                    aborted = Some(format!("turno #{} falló tras {turn_retries} reintento(s): {e}", i + 1));
                    break;
                }
            }
            collector.poll();
        }

        // --- Transcript ---------------------------------------------------
        for t in &log {
            println!("[{}]", t.label);
            println!("> ESTUDIANTE: {}", t.user_said);
            println!("< AGENTE: {}", t.agent_said);
            println!("  [estado -> status={:?}]\n", t.status_after);
        }

        // --- Per-scenario meta-analysis ------------------------------------
        if let Some(reason) = &aborted {
            println!("[FALLO] Escenario abortado: {reason}");
        }
        println!("--- meta-análisis del escenario ---");

        let jargon_hits: Vec<(usize, &str)> = log
            .iter()
            .enumerate()
            .flat_map(|(idx, t)| {
                let lower = t.agent_said.to_lowercase();
                BANNED_TERMS.iter().filter(move |term| lower.contains(**term)).map(move |term| (idx + 1, *term))
            })
            .collect();
        if jargon_hits.is_empty() {
            println!("[OK] Cero meta-lenguaje / confirmaciones pasivas detectadas.");
        } else {
            println!("[FALLO] Jerga o confirmación pasiva filtrada al estudiante:");
            for (turn, term) in &jargon_hits {
                println!("  - turno {turn}: \"{term}\"");
            }
        }
        all_jargon_hits += jargon_hits.len();

        let json_leaks: Vec<usize> =
            log.iter().enumerate().filter(|(_, t)| looks_like_json(&t.agent_said)).map(|(i, _)| i + 1).collect();
        if json_leaks.is_empty() {
            println!("[OK] Ningún mensaje parece exponer estructura de datos/JSON.");
        } else {
            println!("[FALLO] Mensajes con pinta de estructura de datos: turnos {json_leaks:?}");
        }
        all_json_leaks += json_leaks.len();

        if session.status == SessionStatus::Sealed {
            sealed_count += 1;
            // start_session + at most 1 follow-up turn = sealed within 2 log entries.
            if log.len() <= 2 {
                sealed_within_2_turns += 1;
                println!("[OK] Sellado sin turno de confirmación de más (<= 2 turnos totales).");
            } else {
                println!("[ALERTA] Selló, pero tardó {} turnos — revisar si hubo preguntas de más.", log.len());
            }
        } else {
            println!("[FALLO] La sesión nunca selló (debería, aun con el tope estricto de turnos).");
        }

        println!(
            "\nResultado del escenario: status={:?}, perfil={}, roadmap={}, course_id={:?}, first_class_id={:?}",
            session.status,
            session.learner_profile_card.is_some(),
            session.roadmap_package.is_some(),
            session.imported_course_id,
            session.first_class_id,
        );

        match (&session.imported_course_id, &session.first_class_id) {
            (Some(course_id), Some(_class_id)) => {
                let classes = notebook_store.list_classes_for_course(course_id).unwrap_or_default();
                // Notebooks are lazy now (see `notebook_service::generation::
                // start_class_notebook`) — no class, including the first,
                // has any blocks yet right after sealing. This harness only
                // exercises `roadmap_agent`, so it doesn't drive that flow;
                // it just confirms the course/class skeleton imported.
                println!("Curso importado en SQLite: {} clases (notebooks se generan bajo demanda, no aquí).", classes.len());
            }
            _ => println!("[FALLO] Selló sin course_id/first_class_id — el import a SQLite no ocurrió."),
        }

        if session.status == SessionStatus::Sealed {
            report_beginner_checks(&session, &notebook_store);
        }
        println!("
--- estadísticas del escenario ---");
        collector.report();
        println!();
    }

    // --- Aggregate meta-analysis -------------------------------------------
    println!("======================================================================");
    println!("META-ANÁLISIS AGREGADO ({total_scenarios} escenarios)");
    println!("======================================================================");
    println!("Escenarios sellados exitosamente: {sealed_count}/{total_scenarios}");
    println!("Sellados sin turno de confirmación de más: {sealed_within_2_turns}/{total_scenarios}");
    println!("Total de fugas de jerga/confirmación pasiva: {all_jargon_hits}");
    println!("Total de fugas de estructura de datos/JSON: {all_json_leaks}");

    learnkit_lib::dev_trace::set_enabled(false);
    let _ = std::fs::remove_dir_all(&tmp_dir);
    Ok(())
}
