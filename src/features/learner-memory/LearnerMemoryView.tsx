import { useEffect, useState } from "react";
import { getLearnerMemory, type LearnerCognitiveMemory } from "../../lib/tauri";

const BASELINE_LABEL: Record<string, string> = {
  novice_zero: "Principiante total",
  novice_intuitive: "Principiante con intuición",
  intermediate: "Intermedio",
  advanced: "Avanzado",
};

const ABSTRACTION_LABEL: Record<string, string> = {
  visual_analogy: "Analogías visuales",
  causal_mechanics: "Mecánica causal",
  formal_symbolic: "Formalismo simbólico",
};

const FRICTION_LABEL: Record<string, string> = {
  low_frustration: "Baja tolerancia a la frustración",
  resilient: "Resiliente ante el error",
};

function pct(rate: number) {
  return `${Math.round(rate * 100)}%`;
}

// Read-only viewer for `LearnerCognitiveMemory` — always learner "local"
// (single-learner app, see the Rust `domain::learner_memory` module doc).
// No editing here: this data is only ever written by gate grading and class
// closure, never by the user directly.
export function LearnerMemoryView() {
  const [memory, setMemory] = useState<LearnerCognitiveMemory | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    getLearnerMemory()
      .then((m) => {
        if (!cancelled) setMemory(m);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (loading) {
    return (
      <div className="view learner-memory">
        <h2>Memoria del alumno 🧠</h2>
        <p className="muted">Cargando…</p>
      </div>
    );
  }

  if (error || !memory) {
    return (
      <div className="view learner-memory">
        <h2>Memoria del alumno 🧠</h2>
        <div className="alert error">{error ?? "No se pudo cargar la memoria del alumno."}</div>
      </div>
    );
  }

  const { calibratedBaseline, cognitiveMetrics, recurringMisconceptions, retrievalSpacedQueue } = memory;
  const now = Date.now();
  const openCount = recurringMisconceptions.filter((m) => !m.resolved).length;

  return (
    <div className="view learner-memory">
      <h2>Memoria del alumno 🧠</h2>
      <p className="muted">
        Lo que el sistema ha aprendido sobre cómo aprendes — se actualiza sola tras cada gate y cada clase cerrada.
      </p>

      <div className="card">
        <div className="card-head">
          <h3>Perfil cognitivo</h3>
          <span className="badge">{BASELINE_LABEL[calibratedBaseline] ?? calibratedBaseline}</span>
        </div>
        <p className="hint">
          Precisión de predicción: {pct(cognitiveMetrics.predictionAccuracyRate)} · Detección de errores:{" "}
          {pct(cognitiveMetrics.errorAuditDetectionRate)}
        </p>
        <p className="hint">
          Nivel de abstracción preferido:{" "}
          {ABSTRACTION_LABEL[cognitiveMetrics.preferredAbstractionLevel] ?? cognitiveMetrics.preferredAbstractionLevel}
        </p>
        <p className="hint">
          Tolerancia a la fricción cognitiva:{" "}
          {FRICTION_LABEL[cognitiveMetrics.cognitiveFrictionTolerance] ?? cognitiveMetrics.cognitiveFrictionTolerance}
        </p>
      </div>

      <div className="card">
        <div className="card-head">
          <h3>Errores recurrentes</h3>
          <span className={openCount > 0 ? "badge warn" : "badge ok"}>
            {openCount === 0 ? "todo resuelto" : `${openCount} sin resolver`}
          </span>
        </div>
        {recurringMisconceptions.length === 0 ? (
          <p className="muted">Todavía no se ha registrado ningún error recurrente.</p>
        ) : (
          <ul className="dod-list">
            {recurringMisconceptions.map((m, i) => (
              <li key={`${m.domainConcept}-${m.identifiedErrorPattern}-${i}`} className={m.resolved ? "done" : undefined}>
                <strong>{m.domainConcept}</strong> — {m.identifiedErrorPattern}{" "}
                <span className="muted">({m.lastEncounteredDate})</span>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="card">
        <div className="card-head">
          <h3>Cola de repaso espaciado</h3>
          <span className="badge">{retrievalSpacedQueue.length} conceptos</span>
        </div>
        {retrievalSpacedQueue.length === 0 ? (
          <p className="muted">Todavía no hay conceptos en la cola de repaso.</p>
        ) : (
          <ul className="dod-list">
            {retrievalSpacedQueue.map((item) => {
              const due = item.nextDueAtMs <= now;
              return (
                <li key={item.conceptId}>
                  <strong>{item.conceptLabel}</strong> — dominio {pct(item.masteryLevel)}{" "}
                  <span className={due ? "hint-warn" : "muted"}>
                    {due ? "listo para repasar" : `próximo repaso: ${new Date(item.nextDueAtMs).toLocaleDateString()}`}
                  </span>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>
  );
}
