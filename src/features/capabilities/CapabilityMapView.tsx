import { useEffect, useState } from "react";
import { getCapabilityMap, type CapabilityEntry, type CapabilityMap, type SkillEvidence } from "../../lib/tauri";
import { useNotebookNav } from "../../stores/notebook";
import { celebrationsFor, evidenceDetail, evidenceTitle, nextStepsFor } from "./capabilities";

// Read-only capability map of the ACTIVE course. Nothing on this screen is a
// score: each skill shows three independent achievements (Lo resolví / Lo
// recuerdo / Lo aplico) and every celebration expands to the exact attempts it
// stands on. Finishing a class and demonstrating its skill are shown apart.

const CHIPS: { key: "solved" | "retained" | "applied"; label: string }[] = [
  { key: "solved", label: "Lo resolví" },
  { key: "retained", label: "Lo recuerdo" },
  { key: "applied", label: "Lo aplico" },
];

function EvidenceList({ evidence, highlight }: { evidence: SkillEvidence[]; highlight: Set<string> }) {
  if (evidence.length === 0) return <p className="muted">Todavía no hay intentos registrados.</p>;
  return (
    <ul className="cap-evidence">
      {evidence.map((e) => (
        <li key={e.id} className={highlight.has(e.id) ? "cap-evidence-key" : undefined}>
          <strong>{evidenceTitle(e)}</strong>{" "}
          <span className="muted">({new Date(e.createdAtMs).toLocaleDateString()})</span>
          <div className="hint">{evidenceDetail(e)}</div>
          {e.rubric.length > 0 && (
            <ul className="cap-rubric">
              {e.rubric.map((c, i) => (
                <li key={i} className={c.met ? "met" : "pending"}>
                  {c.met ? "✓" : "○"} {c.criterion}
                  {c.evidence ? <span className="muted"> — {c.evidence}</span> : null}
                </li>
              ))}
            </ul>
          )}
        </li>
      ))}
    </ul>
  );
}

function SkillCard({ entry, now }: { entry: CapabilityEntry; now: number }) {
  const celebrations = celebrationsFor(entry);
  const next = nextStepsFor(entry, now);
  const earned = new Set(celebrations.flatMap((c) => c.evidenceIds));
  return (
    <div className="card cap-skill">
      <div className="card-head">
        <h3>{entry.title}</h3>
        <span className="badge">{entry.classComplete ? "clase terminada" : "en curso"}</span>
      </div>
      {entry.objective && <p className="hint">Sabrás: {entry.objective}</p>}
      <div className="cap-chips" role="list">
        {CHIPS.map(({ key, label }) => (
          <span
            key={key}
            role="listitem"
            className={"badge" + (entry.status[key].achieved ? " ok" : " cap-chip-off")}
            title={entry.status[key].achieved ? "Respaldado por evidencia — ábrela abajo" : "Aún sin evidencia"}
          >
            {entry.status[key].achieved ? "✓ " : ""}
            {label}
          </span>
        ))}
      </div>
      {celebrations.map((c) => (
        <p key={c.kind} className="cap-celebration">
          {c.text}
        </p>
      ))}
      {next.map((n) => (
        <p key={n.text} className="hint">
          Siguiente: {n.text}
        </p>
      ))}
      <details>
        <summary>Ver evidencia ({entry.evidence.length})</summary>
        <EvidenceList evidence={entry.evidence} highlight={earned} />
      </details>
    </div>
  );
}

export function CapabilityMapView() {
  const courseId = useNotebookNav((s) => s.courseId);
  const [map, setMap] = useState<CapabilityMap | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(Boolean(courseId));

  useEffect(() => {
    if (!courseId) return;
    let cancelled = false;
    setLoading(true);
    getCapabilityMap(courseId)
      .then((m) => !cancelled && setMap(m))
      .catch((e) => !cancelled && setError(String(e)))
      .finally(() => !cancelled && setLoading(false));
    return () => {
      cancelled = true;
    };
  }, [courseId]);

  if (!courseId) {
    return (
      <div className="view capabilities">
        <h2>Mapa de capacidades 🗺</h2>
        <p className="muted">Abre un curso para ver qué has demostrado en cada habilidad.</p>
      </div>
    );
  }
  if (loading) {
    return (
      <div className="view capabilities">
        <h2>Mapa de capacidades 🗺</h2>
        <p className="muted">Cargando…</p>
      </div>
    );
  }
  if (error || !map) {
    return (
      <div className="view capabilities">
        <h2>Mapa de capacidades 🗺</h2>
        <div className="alert error">{error ?? "No se pudo cargar el mapa de capacidades."}</div>
      </div>
    );
  }

  const now = Date.now();
  const weeks = [...new Set(map.entries.map((e) => e.weekNumber))].sort((a, b) => a - b);
  const { totals } = map;

  return (
    <div className="view capabilities">
      <h2>Mapa de capacidades 🗺</h2>
      <p className="muted">
        Lo que has demostrado, no cuánto tiempo pasaste: cada logro se apoya en intentos concretos que puedes abrir.
      </p>
      <div className="card">
        <div className="card-head">
          <h3>Resumen</h3>
          <span className="badge">{totals.skills} habilidades</span>
        </div>
        <p className="hint">
          Resueltas: {totals.solved} · Recordadas tras un intervalo: {totals.retained} · Aplicadas en un caso nuevo:{" "}
          {totals.applied}
        </p>
      </div>
      {weeks.map((week) => {
        const entries = map.entries.filter((e) => e.weekNumber === week);
        return (
          <section key={week} className="cap-week">
            <h3 className="cap-week-title">
              {week > 0 ? `Semana ${week}` : "Sin semana"}
              {entries[0]?.milestoneTitle ? ` — ${entries[0].milestoneTitle}` : ""}
            </h3>
            {entries.map((e) => (
              <SkillCard key={e.skillId} entry={e} now={now} />
            ))}
          </section>
        );
      })}
    </div>
  );
}
