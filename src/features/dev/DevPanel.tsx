import { useMemo, useState } from "react";
import { buildTimeline, filterTimeline, isProblem, type DevTraceEntry, type TimelineItem } from "../../lib/devTrace";
import { useDev } from "../../stores/dev";

function pretty(v: unknown): string {
  if (typeof v === "string") return v;
  try {
    return JSON.stringify(v, null, 2);
  } catch {
    return String(v);
  }
}

function time(ms: number): string {
  return new Date(ms).toLocaleTimeString();
}

function Section({ title, text }: { title: string; text: string }) {
  return (
    <details className="dev-section">
      <summary>
        {title} <span className="muted">({text.length.toLocaleString()} car.)</span>
      </summary>
      <pre className="dev-pre">{text}</pre>
    </details>
  );
}

function Title({ e }: { e: DevTraceEntry }) {
  if (e.kind === "agent_run") return <>🤖 {e.agentId ?? "agente"} · {e.flow}</>;
  if (e.kind === "tool_call") return <>🔧 {e.tool}</>;
  if (e.kind === "retry") return <>↻ reintento {e.attempt ?? ""} · {e.verdict}</>;
  if (e.kind === "critique") return <>🔍 {e.flow} · {e.verdict}</>;
  if (e.kind === "loop_result") return <>🏁 bucle · {e.verdict} ({e.attempt} intento(s))</>;
  return <>{e.kind}</>;
}

function Row({ item }: { item: TimelineItem }) {
  const { entry: e, tools } = item;
  const bad = isProblem(e) || tools.some(isProblem);
  return (
    <details className={"dev-row" + (bad ? " bad" : "")}>
      <summary>
        <span className="muted">{time(e.tsMs)}</span> <Title e={e} />
        {e.model && (
          <span className="badge">
            {e.provider}/{e.model}
          </span>
        )}
        {e.durationMs !== undefined && <span className="muted"> {(e.durationMs / 1000).toFixed(1)}s</span>}
        {e.ok === false && <span className="badge warn">{e.errorCode ?? "error"}</span>}
        {e.transient && <span className="badge">transitorio</span>}
        {e.budgetExhausted && <span className="badge warn">sin espacio de salida</span>}
      </summary>
      <div className="dev-body">
        {e.scope && <p className="hint">Clase/sesión: <code>{e.scope}</code></p>}
        {e.error && <div className="alert error">{e.error}</div>}
        {e.notes && e.notes.length > 0 && (
          <ul className="dev-notes">
            {e.notes.map((n, i) => (
              <li key={i}>{n}</li>
            ))}
          </ul>
        )}
        {e.toolCalls && e.toolCalls.length > 0 && <p className="hint">Tools declaradas: {e.toolCalls.join(", ")}</p>}
        {e.systemPrompt && <Section title="System prompt" text={e.systemPrompt} />}
        {e.input && <Section title="Input (prompt del turno)" text={e.input} />}
        {e.output && <Section title="Salida del modelo" text={e.output} />}
        {e.toolArgs !== undefined && <Section title={`Argumentos${e.tool ? ` · ${e.tool}` : ""}`} text={pretty(e.toolArgs)} />}
        {tools.map((t) => (
          <div key={t.id} className="dev-tool">
            <strong>🔧 {t.tool}</strong> <span className="muted">{time(t.tsMs)}</span>
            <Section title="Argumentos recibidos" text={pretty(t.toolArgs)} />
          </div>
        ))}
        {e.kind === "agent_run" && tools.length === 0 && e.ok && (
          <p className="hint hint-warn">El modelo no llamó a ninguna tool en este turno.</p>
        )}
      </div>
    </details>
  );
}

// Docked inspector for the generation pipeline. Only mounted while dev mode is
// on (see `App`). Shows what the metadata-only logs omit: prompts, tool args,
// critic verdicts, retries and classified model errors.
export function DevPanel() {
  const entries = useDev((s) => s.entries);
  const panelOpen = useDev((s) => s.panelOpen);
  const setPanelOpen = useDev((s) => s.setPanelOpen);
  const clear = useDev((s) => s.clear);
  const [onlyProblems, setOnlyProblems] = useState(false);
  const [scope, setScope] = useState<string>("");
  const [copied, setCopied] = useState(false);

  const timeline = useMemo(() => buildTimeline(entries), [entries]);
  const scopes = useMemo(() => [...new Set(entries.map((e) => e.scope).filter((s): s is string => !!s))], [entries]);
  const shown = useMemo(
    () => filterTimeline(timeline, { onlyProblems, scope: scope || null }).slice().reverse(),
    [timeline, onlyProblems, scope],
  );
  const problems = useMemo(() => timeline.filter((i) => isProblem(i.entry) || i.tools.some(isProblem)).length, [timeline]);

  if (!panelOpen) return null;

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(JSON.stringify(entries, null, 2));
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard can be denied by the webview; the panel stays usable.
    }
  };

  return (
    <aside className="dev-panel" aria-label="Panel de desarrollador">
      <div className="dev-bar">
        <strong>🛠 Desarrollador</strong>
        <span className="muted">
          {entries.length} eventos · {problems} con incidencias
        </span>
        <label className="hint">
          <input type="checkbox" checked={onlyProblems} onChange={(e) => setOnlyProblems(e.target.checked)} /> solo
          incidencias
        </label>
        {scopes.length > 1 && (
          <select value={scope} onChange={(e) => setScope(e.target.value)} aria-label="Filtrar por clase o sesión">
            <option value="">todas las clases/sesiones</option>
            {scopes.map((s) => (
              <option key={s} value={s}>
                {s}
              </option>
            ))}
          </select>
        )}
        <button onClick={() => void copy()}>{copied ? "Copiado" : "Copiar JSON"}</button>
        <button onClick={() => void clear()}>Limpiar</button>
        <button onClick={() => setPanelOpen(false)} aria-label="Cerrar panel">
          ✕
        </button>
      </div>
      <p className="hint dev-note">
        Se ve el prompt inicial, cada llamada a tool y la salida final; los turnos intermedios internos del modelo no
        son interceptables. Las tools se asocian a su ejecución por ventana de tiempo.
      </p>
      <div className="dev-list">
        {shown.length === 0 ? (
          <p className="muted">Sin eventos todavía: genera un bloque o conversa con el agente.</p>
        ) : (
          shown.map((item) => <Row key={item.entry.id} item={item} />)
        )}
      </div>
    </aside>
  );
}
