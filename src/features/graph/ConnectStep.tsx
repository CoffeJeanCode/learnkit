import { useEffect, useState } from "react";
import { useGraph } from "../../stores/graph";
import { useNotebookNav } from "../../stores/notebook";
import { useUi } from "../../stores/ui";
import type { ClassRecord, Concept } from "../../lib/schemas";
import { RELATION_HINTS, earlierNodes, truncate } from "./graph";
import type { NodeOption } from "./graph";

const MAX_NEW_IDEAS = 3;

export interface ConnectDraft {
  idea: string;
  /** Id of an earlier class/concept to relate the idea to ("" = none). */
  targetId: string;
  relation: string;
}

export const emptyDraft = (): ConnectDraft => ({ idea: "", targetId: "", relation: "" });

/** A draft is saveable when it has an idea; if it names a target, the
 *  relation needs the student's own words too (never a bare, unexplained link). */
export function draftProblem(d: ConnectDraft): string | null {
  if (!d.idea.trim()) return null;
  if (d.targetId && !d.relation.trim()) return "Escribe cómo se relaciona (con tus palabras) o quita la conexión.";
  if (!d.targetId && d.relation.trim()) return "Elige con qué idea la relacionas.";
  return null;
}

/** The optional extra step after a passed closure: the STUDENT writes the ideas
 *  they take from the class and relates them to what they already knew.
 *  Presentational — `ConnectStep` below wires it to the stores. Nothing is
 *  suggested or generated here, and skipping it never blocks anything. */
export function ConnectStepView({
  className,
  existing,
  targets,
  error,
  saved,
  onSave,
  onOpenGraph,
}: {
  className: string;
  existing: Concept[];
  targets: NodeOption[];
  error: string | null;
  saved: boolean;
  onSave: (drafts: ConnectDraft[]) => Promise<boolean>;
  onOpenGraph: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [drafts, setDrafts] = useState<ConnectDraft[]>([emptyDraft()]);
  const [busy, setBusy] = useState(false);
  const [skipped, setSkipped] = useState(false);

  if (skipped) return null;
  if (!open) {
    return (
      <div className="connect-step">
        <strong>🕸 Paso extra (opcional): conecta lo aprendido</strong>
        <p className="hint">
          Escribe con tus palabras una idea que te llevas de «{className}» y relaciónala con algo que ya sabías. Explicar <em>por qué</em> se conectan te ayuda a
          recordarlo y a entenderlo mejor.
        </p>
        {saved && <p className="hint hint-ok">Conexiones guardadas ✓</p>}
        <div className="row">
          <button className="btn-primary" onClick={() => setOpen(true)}>
            {saved ? "Añadir más conexiones" : "Conectar ideas"}
          </button>
          {saved && (
            <button className="btn-quiet" onClick={onOpenGraph}>
              Ver en el grafo
            </button>
          )}
          {!saved && (
            <button className="btn-quiet" onClick={() => setSkipped(true)}>
              Omitir
            </button>
          )}
        </div>
      </div>
    );
  }

  const set = (i: number, patch: Partial<ConnectDraft>) => setDrafts((ds) => ds.map((d, j) => (j === i ? { ...d, ...patch } : d)));
  const problems = drafts.map(draftProblem);
  const filled = drafts.filter((d) => d.idea.trim());
  const canSave = filled.length > 0 && problems.every((p) => p === null) && !busy;

  const save = async () => {
    setBusy(true);
    const ok = await onSave(filled);
    setBusy(false);
    if (ok) {
      setDrafts([emptyDraft()]);
      setOpen(false);
    }
  };

  return (
    <div className="connect-step">
      <strong>🕸 Conecta lo aprendido</strong>
      {existing.length > 0 && (
        <p className="hint">
          Ya anotaste: {existing.map((c) => `«${truncate(c.label, 40)}»`).join(", ")}.
        </p>
      )}
      {targets.length === 0 && <p className="hint">Es tu primera clase: guarda tus ideas ahora y podrás conectarlas con las siguientes.</p>}
      {drafts.map((d, i) => (
        <div key={i} className="connect-item">
          <label>
            Una idea que te llevas, con tus palabras
            <input value={d.idea} maxLength={120} placeholder="Como se la explicarías a alguien…" onChange={(e) => set(i, { idea: e.target.value })} />
          </label>
          {targets.length > 0 && d.idea.trim() && (
            <>
              <label>
                ¿Con qué que ya sabías se conecta?
                <select value={d.targetId} onChange={(e) => set(i, { targetId: e.target.value })}>
                  <option value="">No la conecto ahora</option>
                  {targets.map((t) => (
                    <option key={t.id} value={t.id}>
                      {t.kind === "class" ? "Clase · " : "↳ "}
                      {truncate(t.label, 60)}
                    </option>
                  ))}
                </select>
              </label>
              {d.targetId && (
                <>
                  <label>
                    ¿Cómo se relacionan?
                    <input value={d.relation} maxLength={120} placeholder="Por qué o cómo se conectan…" onChange={(e) => set(i, { relation: e.target.value })} />
                  </label>
                  <div className="chip-row">
                    {RELATION_HINTS.map((h) => (
                      <button key={h} type="button" className="chip" onClick={() => set(i, { relation: d.relation.trim() ? d.relation : h })}>
                        {h}
                      </button>
                    ))}
                  </div>
                </>
              )}
            </>
          )}
          {problems[i] && <p className="hint hint-warn">{problems[i]}</p>}
        </div>
      ))}
      {drafts.length < MAX_NEW_IDEAS && drafts[drafts.length - 1]!.idea.trim() && (
        <button className="btn-quiet" onClick={() => setDrafts((ds) => [...ds, emptyDraft()])}>
          ＋ Otra idea
        </button>
      )}
      {error && <p className="hint hint-warn">{error}</p>}
      <div className="row">
        <button className="btn-primary" onClick={() => void save()} disabled={!canSave}>
          {busy ? "Guardando…" : "Guardar conexiones"}
        </button>
        <button className="btn-quiet" onClick={() => setOpen(false)} disabled={busy}>
          Cancelar
        </button>
      </div>
    </div>
  );
}

/** Wires the step to the active class and the graph store. Renders nothing
 *  until the class and its course are known. */
export function ConnectStep() {
  const classId = useNotebookNav((s) => s.activeClassId);
  const courseId = useNotebookNav((s) => s.courseId);
  const navClasses = useNotebookNav((s) => s.classes);
  const setView = useUi((s) => s.setView);
  const { graph, error, load, addConcept, addLink } = useGraph();
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    if (courseId) void load(courseId);
  }, [courseId, load]);

  const current: ClassRecord | undefined = navClasses.find((c) => c.id === classId);
  if (!courseId || !classId || !current) return null;

  const existing = graph.concepts.filter((c) => c.classId === classId);
  const targets = earlierNodes(navClasses, graph.concepts, classId);

  const onSave = async (drafts: ConnectDraft[]): Promise<boolean> => {
    for (const d of drafts) {
      // A retry after a failed link must not write the same idea twice.
      const wording = d.idea.split(/\s+/).filter(Boolean).join(" ");
      const prior = useGraph.getState().graph.concepts.find((c) => c.classId === classId && c.label === wording);
      const concept = prior ?? (await addConcept(classId, d.idea));
      if (!concept) return false;
      if (d.targetId && !(await addLink(concept.id, d.targetId, d.relation))) return false;
    }
    setSaved(true);
    return true;
  };

  return (
    <ConnectStepView
      className={current.title.replace(/^Semana\s+\d+\s*:\s*/i, "")}
      existing={existing}
      targets={targets}
      error={error}
      saved={saved}
      onSave={onSave}
      onOpenGraph={() => setView("graph")}
    />
  );
}

