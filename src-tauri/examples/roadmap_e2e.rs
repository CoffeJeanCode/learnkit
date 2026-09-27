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

use std::sync::Arc;

use learnkit_lib::agents::{roadmap_agent, AgentRegistry};
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
    ];
    let model = MODEL_FOR
        .iter()
        .find(|(p, _)| *p == provider_id)
        .map(|(_, m)| *m)
        .unwrap_or_else(|| panic!("no model pin for provider '{provider_id}'"));

    let memory_vault = MemoryVault::new();
    memory_vault.save_provider_key(&provider_id, &api_key).expect("seed in-memory vault");
    let vault: Arc<dyn SecretVault> = Arc::new(memory_vault);
    let providers = Arc::new(ProviderRegistry::with_defaults());
    println!("Using provider: {provider_id} (model: {model})\n");

    let agents = Arc::new(AgentRegistry::new());
    let mut def = roadmap_agent::definition();
    def.model = learnkit_lib::domain::model::ModelRef::new(provider_id.clone(), model);
    agents.register(def).expect("register roadmap agent");

    let orchestrator = Arc::new(Orchestrator::with_rig_runner(Arc::clone(&agents), providers, vault));
    let tmp_dir = std::env::temp_dir().join(format!("learnkit-e2e-{}", uuid::Uuid::new_v4()));
    let store = FileStore::new(tmp_dir.clone());
    let notebook_store = NotebookStore::open(tmp_dir.join("notebook.sqlite3"))?;
    let service = RoadmapService::new(orchestrator, store, notebook_store.clone());

    let mut all_jargon_hits = 0usize;
    let mut all_json_leaks = 0usize;
    let mut sealed_count = 0usize;
    let mut sealed_within_2_turns = 0usize;
    let total_scenarios = scenarios().len();

    for scenario in scenarios() {
        println!("======================================================================");
        println!("ESCENARIO: {}", scenario.name);
        println!("======================================================================\n");

        let mut log: Vec<TurnLog> = Vec::new();
        let r = service.start_session(None).await?;
        log.push(TurnLog {
            label: "start_session".to_string(),
            user_said: "(apertura de sesión)".to_string(),
            agent_said: r.message.clone(),
            status_after: r.session.status,
        });
        let session_id = r.session.session_id.clone();
        let mut session = r.session;

        let mut i = 0;
        while session.status != SessionStatus::Sealed && i < scenario.student_turns.len() {
            let user_msg = scenario.student_turns[i];
            let r = service.send_message(None, &session_id, user_msg).await?;
            log.push(TurnLog {
                label: format!("send_message #{}", i + 1),
                user_said: user_msg.to_string(),
                agent_said: r.message.clone(),
                status_after: r.session.status,
            });
            session = r.session;
            i += 1;
        }

        // --- Transcript ---------------------------------------------------
        for t in &log {
            println!("[{}]", t.label);
            println!("> ESTUDIANTE: {}", t.user_said);
            println!("< AGENTE: {}", t.agent_said);
            println!("  [estado -> status={:?}]\n", t.status_after);
        }

        // --- Per-scenario meta-analysis ------------------------------------
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

    let _ = std::fs::remove_dir_all(&tmp_dir);
    Ok(())
}
