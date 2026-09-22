import { useEffect, useState } from "react";
import type { RoadmapPhase } from "../../lib/schemas";
import { useProviders } from "../../stores/providers";
import { useRoadmap } from "../../stores/roadmap";

const PHASES: { id: RoadmapPhase; label: string }[] = [
  { id: "exploration", label: "1. Exploración" },
  { id: "diagnostic", label: "2. Diagnóstico" },
  { id: "syllabus", label: "3. Syllabus" },
  { id: "negotiation", label: "4. Negociación" },
];

function dodLabel(id: string): string {
  return id.replace(/_/g, " ");
}

// Step 0 of onboarding: connect a provider before the diagnostic conversation
// can start. Shown only while no provider has a key configured — env-var
// auto-detection (see backend `seed_keys_from_env`) usually skips this
// entirely for a developer who already has e.g. `ANTHROPIC_API_KEY` set.
function ProviderSetup() {
  const { providers, busyId, error, saveKey } = useProviders();
  const [providerId, setProviderId] = useState("");
  const [apiKey, setApiKey] = useState("");
  const busy = busyId === providerId;

  // Saving the key updates the shared providers store; RoadmapView reacts to
  // that (its `anyConfigured` flips true) and starts the conversation on its
  // own — starting it here too would race it (and `saveKey` swallows its own
  // errors, so it would even fire after a failed save).
  const submit = async () => {
    if (!providerId || !apiKey.trim()) return;
    await saveKey(providerId, apiKey);
    setApiKey("");
  };

  return (
    <div className="view roadmap">
      <h2>Bienvenido a LearnKit</h2>
      <p className="muted">
        Antes de empezar tu diagnóstico de aprendizaje, conecta un proveedor de IA. Tu API key se
        guarda cifrada en este equipo (Stronghold) y nunca se comparte ni se vuelve a mostrar.
      </p>

      <label>
        Proveedor
        <select value={providerId} onChange={(e) => setProviderId(e.target.value)}>
          <option value="">— elige uno —</option>
          {providers.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </label>

      <label>
        API key
        <input
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && submit()}
          placeholder="sk-…"
          autoComplete="off"
        />
      </label>

      {error && <div className="alert error">{error}</div>}

      <div className="row">
        <button disabled={busy || !providerId || !apiKey.trim()} onClick={submit}>
          {busy ? "Guardando…" : "Guardar y comenzar"}
        </button>
      </div>
    </div>
  );
}

export function RoadmapView() {
  const { session, messages, sending, error, start, send, reset } = useRoadmap();
  const { providers, loading: providersLoading, refresh: refreshProviders } = useProviders();
  const [input, setInput] = useState("");

  useEffect(() => {
    refreshProviders();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const anyConfigured = providers.some((p) => p.configured);

  // Onboarding begins on its own — no click required — but only once we know
  // a provider is actually configured; otherwise ProviderSetup below owns
  // the "start" trigger (right after the key is saved).
  useEffect(() => {
    if (!providersLoading && anyConfigured && !session && !sending && messages.length === 0) {
      start();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [providersLoading, anyConfigured]);

  const submit = () => {
    if (!input.trim()) return;
    send(input);
    setInput("");
  };

  if (providersLoading) {
    return (
      <div className="view roadmap">
        <p className="muted">Cargando…</p>
      </div>
    );
  }

  if (!anyConfigured) {
    return <ProviderSetup />;
  }

  const phaseIndex = session ? PHASES.findIndex((p) => p.id === session.phase) : -1;
  const dodEntries = session ? Object.entries(session.dod) : [];
  const sealed = session?.status === "sealed";
  const pkg = session?.final_package ?? null;

  return (
    <div className="view roadmap">
      <h2>Roadmap & Syllabus Diagnostic</h2>
      <p className="muted">
        Diagnóstico conversacional con diseño inverso (Backward Design). El estado de fase y las
        compuertas DoD se validan en el backend, no solo en el modelo.
      </p>

      {!session && (
        <div className="row">
          <button onClick={start} disabled={sending}>
            {sending ? "Iniciando…" : "Iniciar sesión"}
          </button>
        </div>
      )}

      {session && (
        <>
          <div className="phase-tracker">
            {PHASES.map((p, i) => (
              <div
                key={p.id}
                className={
                  "phase-step" +
                  (i === phaseIndex ? " active" : "") +
                  (i < phaseIndex || sealed ? " done" : "")
                }
              >
                {p.label}
              </div>
            ))}
          </div>

          {dodEntries.length > 0 && (
            <div className="dod-panel">
              <div className="dod-title">Definition of Done — fase actual</div>
              <ul className="dod-list">
                {dodEntries.map(([id, done]) => (
                  <li key={id} className={done ? "done" : ""}>
                    <span className="dod-mark">{done ? "✓" : "○"}</span> {dodLabel(id)}
                  </li>
                ))}
              </ul>
            </div>
          )}

          <div className="messages">
            {messages.map((m, i) => (
              <div key={i} className={`msg ${m.role}`}>
                <div className="msg-role">{m.role}</div>
                <div className="msg-body">{m.content}</div>
              </div>
            ))}
            {sending && (
              <div className="msg status">
                <div className="msg-body">Pensando…</div>
              </div>
            )}
          </div>

          {error && <div className="alert error">{error}</div>}

          {!sealed && (
            <div className="row composer">
              <input
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && submit()}
                placeholder="Responde al mentor…"
                disabled={sending}
              />
              <button onClick={submit} disabled={sending || !input.trim()}>
                Enviar
              </button>
            </div>
          )}

          {sealed && pkg && (
            <div className="card final-package">
              <div className="card-head">
                <h3>RoadmapPackageFinal sellado</h3>
                <span className="badge ok">listo para Notebook Builder</span>
              </div>
              <p>{pkg.executive_summary}</p>
              <p className="hint">Aprobación: “{pkg.approval.approved_verbatim_quote}”</p>
              <ul className="dod-list">
                {pkg.syllabus.milestones.map((m) => (
                  <li key={m.id} className="done">
                    <strong>{m.title}</strong> — {m.learning_objective} ({m.bloom_level},{" "}
                    {m.estimated_hours}h)
                  </li>
                ))}
              </ul>
              <details>
                <summary>Ver payload completo (JSON)</summary>
                <pre className="output">{JSON.stringify(pkg, null, 2)}</pre>
              </details>
            </div>
          )}

          <div className="row">
            <button onClick={reset}>Nueva sesión</button>
          </div>
        </>
      )}
    </div>
  );
}
