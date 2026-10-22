//! Shared helpers for the live e2e harnesses (included via `#[path]`):
//! provider timeout/model guards and the model-call statistics report built
//! from the developer trace. Nothing here reads credentials.

#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::time::Instant;

use learnkit_lib::dev_trace::{self, TraceEntry};

/// Models allowed against Xiaomi (limited billing: never ultraspeed).
pub const XIAOMI_ALLOWED_MODELS: &[&str] = &["mimo-v2.6-pro", "mimo-v2.6-flash"];

/// Sets `LEARNKIT_PROMPT_TIMEOUT_SECS=300` when unset and returns the
/// effective value (as the provider factory will read it). Call before any
/// provider call and before spawning threads.
pub fn ensure_prompt_timeout() -> u64 {
    if std::env::var("LEARNKIT_PROMPT_TIMEOUT_SECS").map(|v| v.trim().is_empty()).unwrap_or(true) {
        // SAFETY-ish: single-threaded at startup, before any runtime work.
        std::env::set_var("LEARNKIT_PROMPT_TIMEOUT_SECS", "300");
    }
    let raw = std::env::var("LEARNKIT_PROMPT_TIMEOUT_SECS").unwrap_or_default();
    let effective = raw.trim().parse::<u64>().map(|v| v.clamp(30, 900)).unwrap_or(120);
    println!("Timeout por llamada al proveedor: {effective}s (LEARNKIT_PROMPT_TIMEOUT_SECS={raw})");
    effective
}

/// Exits with a clear message when a Xiaomi model outside the allow-list is requested.
pub fn enforce_model_policy(provider_id: &str, model: &str) {
    if provider_id == "xiaomi" && !XIAOMI_ALLOWED_MODELS.contains(&model) {
        eprintln!("Modelo '{model}' no permitido para xiaomi. Solo: {}. (facturación limitada; nunca ultraspeed)", XIAOMI_ALLOWED_MODELS.join(", "));
        std::process::exit(2);
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub fn stamp() -> String {
    let secs = (now_ms() / 1000) % 86_400;
    format!("{:02}:{:02}:{:02}Z", secs / 3600, (secs / 60) % 60, secs % 60)
}

/// One model call, slimmed (no prompts/outputs).
#[derive(Clone, Debug)]
pub struct Call {
    pub agent: String,
    /// Start (trace timestamp minus duration), epoch ms.
    pub start_ms: i64,
    pub duration_ms: u64,
    /// `ok` | `error` | `timeout`
    pub outcome: String,
    pub error_code: Option<String>,
    /// Block type published by a successful generator call, when attributable.
    pub block: Option<String>,
}

/// Accumulates trace entries across polls so the 300-entry ring buffer
/// cannot drop calls during a long run.
pub struct TraceCollector {
    origin_ms: i64,
    origin: Instant,
    seen: HashMap<u64, ()>,
    calls: Vec<Call>,
    pending_block: Option<String>,
}

fn find_block_type(v: &serde_json::Value, depth: u8) -> Option<String> {
    if depth > 4 {
        return None;
    }
    match v {
        serde_json::Value::Object(m) => {
            if let Some(t) = m.get("blockType").and_then(|t| t.as_str()) {
                return Some(t.to_string());
            }
            m.values().find_map(|x| find_block_type(x, depth + 1))
        }
        serde_json::Value::Array(a) => a.iter().find_map(|x| find_block_type(x, depth + 1)),
        _ => None,
    }
}

impl TraceCollector {
    pub fn new() -> Self {
        Self { origin_ms: now_ms(), origin: Instant::now(), seen: HashMap::new(), calls: Vec::new(), pending_block: None }
    }

    pub fn len(&self) -> usize {
        self.calls.len()
    }

    /// Merges any new entries from the global dev trace.
    pub fn poll(&mut self) {
        let mut entries: Vec<TraceEntry> = dev_trace::snapshot();
        entries.sort_by_key(|e| e.id);
        for e in entries {
            if self.seen.insert(e.id, ()).is_some() {
                continue;
            }
            match e.kind.as_str() {
                "tool_call" if e.tool.as_deref() == Some("publish_notebook_block") => {
                    self.pending_block = e.tool_args.as_ref().and_then(|a| find_block_type(a, 0));
                }
                "agent_run" => {
                    let dur = e.duration_ms.unwrap_or(0);
                    let agent = e.agent_id.clone().unwrap_or_else(|| "?".to_string());
                    let timed_out = e.error_code.as_deref() == Some("Timeout")
                        || e.error.as_deref().is_some_and(|m| m.contains("no respondió"));
                    let outcome = if e.ok == Some(false) { if timed_out { "timeout" } else { "error" } } else { "ok" };
                    let block = if outcome == "ok" && agent.contains("generator") { self.pending_block.take() } else { None };
                    self.calls.push(Call {
                        agent,
                        start_ms: e.ts_ms - dur as i64,
                        duration_ms: dur,
                        outcome: outcome.to_string(),
                        error_code: e.error_code.clone(),
                        block,
                    });
                }
                _ => {}
            }
        }
    }

    /// Prints per-agent statistics and the chronological call timeline.
    pub fn report(&mut self) {
        self.poll();
        println!("\n--- estadísticas de llamadas al modelo (dev trace acumulado) ---");
        if self.calls.is_empty() {
            println!("  (0 llamadas registradas: el modelo NO fue invocado, o la traza estaba apagada)");
            return;
        }
        // agent -> (ok, err, timeout, min, max, total)
        let mut per: BTreeMap<&str, (u32, u32, u32, u64, u64, u64)> = BTreeMap::new();
        for c in &self.calls {
            let s = per.entry(c.agent.as_str()).or_insert((0, 0, 0, u64::MAX, 0, 0));
            match c.outcome.as_str() {
                "ok" => s.0 += 1,
                "timeout" => s.2 += 1,
                _ => s.1 += 1,
            }
            s.3 = s.3.min(c.duration_ms);
            s.4 = s.4.max(c.duration_ms);
            s.5 += c.duration_ms;
        }
        let secs = |ms: u64| ms as f32 / 1000.0;
        for (agent, (ok, err, to, min, max, total)) in &per {
            let n = (ok + err + to) as u64;
            println!(
                "  {agent}: {ok} ok / {err} error / {to} timeout | min {:.1}s avg {:.1}s max {:.1}s | total {:.1}s",
                secs(*min),
                secs(total / n.max(1)),
                secs(*max),
                secs(*total)
            );
        }
        let total_calls = self.calls.len();
        let total_ms: u64 = self.calls.iter().map(|c| c.duration_ms).sum();
        println!("  TOTAL: {total_calls} llamadas, {:.1}s acumulados de modelo, {:.0}s de pared", secs(total_ms), self.origin.elapsed().as_secs_f32());
        println!("  tokens: por turno en la traza de desarrollo (kind=usage, verdict=model_turn_usage) y en tracing `model_turn_usage`; no se acumulan en AgentOutput");
        println!("--- cronología de llamadas (inicio relativo, duración, resultado) ---");
        let mut calls = self.calls.clone();
        calls.sort_by_key(|c| c.start_ms);
        for (i, c) in calls.iter().enumerate() {
            println!(
                "  {:>3}. +{:>6.0}s  {:<28} {:>6.1}s  {}{}{}",
                i + 1,
                (c.start_ms - self.origin_ms) as f32 / 1000.0,
                c.agent,
                secs(c.duration_ms),
                c.outcome,
                c.error_code.as_deref().map(|e| format!(" ({e})")).unwrap_or_default(),
                c.block.as_deref().map(|b| format!(" -> bloque {b}")).unwrap_or_default(),
            );
        }
    }
}
