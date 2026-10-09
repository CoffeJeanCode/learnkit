import { useEffect, useState } from "react";
import { useProviders } from "../../stores/providers";
import { StudySettingsCard } from "../study/StudySettingsCard";
import type { ModelInfo } from "../../types";

// Module-level stable fallbacks: selectors must return referentially stable
// values — `?? []` allocates a fresh array per snapshot, which zustand reads
// as "store changed" on every render and spins into Maximum update depth.
const EMPTY_MODELS: ModelInfo[] = [];
const EMPTY_SUGGESTIONS: string[] = [];

// One provider as an accordion row: a clean summary line always visible,
// the wiring (key → model → advanced) revealed in numbered sections only
// when expanded. Unconfigured providers open themselves on first load so
// whatever needs attention is what you see.
function ProviderCard({ id }: { id: string }) {
  const p = useProviders((s) => s.providers.find((x) => x.id === id));
  const busyId = useProviders((s) => s.busyId);
  const models = useProviders((s) => s.modelsCache[id] ?? EMPTY_MODELS);
  const suggestions = useProviders((s) =>
    p ? (s.suggestionsCache[p.provider] ?? EMPTY_SUGGESTIONS) : EMPTY_SUGGESTIONS,
  );
  const { saveConfig, saveKey, removeKey, test, loadModels, loadSuggestions } = useProviders.getState();

  const [apiKey, setApiKey] = useState("");
  const [defaultModel, setDefaultModel] = useState<string | null>(null);
  const [baseUrl, setBaseUrl] = useState<string | null>(null);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (p) {
      setDefaultModel(p.default_model);
      setBaseUrl(p.base_url);
      loadSuggestions(p.provider);
      if (!p.configured) setOpen(true);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [p?.id]);

  if (!p) return null;
  const busy = busyId === id;

  return (
    <div className="card provider">
      <button
        className="provider-head"
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
        title={open ? "Contraer" : "Expandir"}
      >
        <span className="provider-name">{p.name}</span>
        <span className="provider-head-right">
          <span className={p.configured ? "badge ok" : "badge"}>
            {p.configured ? "Lista ✓" : "Sin llave"}
          </span>
          <span className="provider-chevron" aria-hidden="true">
            {open ? "▾" : "▸"}
          </span>
        </span>
      </button>

      {!open && (
        <p className="hint provider-sub">
          {p.configured
            ? `Modelo: ${p.default_model ?? "por defecto del proveedor"}`
            : "Falta la llave — expándelo para conectarlo"}
        </p>
      )}

      {open && (
        <>
          <div className="section-label">1 · Llave de API</div>
          <label>
            Tu llave se guarda cifrada en este equipo y nunca se vuelve a mostrar.
            <input
              type="password"
              placeholder={p.configured ? "•••••••• (guardada)" : "pégala aquí…"}
              value={apiKey}
              autoComplete="off"
              onChange={(e) => setApiKey(e.target.value)}
            />
          </label>
          <div className="row">
            <button
              className="btn-primary"
              disabled={busy || !apiKey.trim()}
              onClick={() => saveKey(id, apiKey).then(() => setApiKey(""))}
            >
              Guardar llave
            </button>
            <button
              className="btn-quiet"
              disabled={busy || !p.configured}
              onClick={() => test(id)}
            >
              Probar conexión
            </button>
            <button
              className="btn-quiet btn-danger-quiet"
              disabled={busy || !p.configured}
              onClick={() => removeKey(id)}
            >
              Borrar
            </button>
          </div>

          <div className="section-label">2 · Modelo</div>
          <label>
            Modelo por defecto
            <input
              value={defaultModel ?? ""}
              placeholder={suggestions[0] ?? "id del modelo (vale manual)"}
              onChange={(e) => setDefaultModel(e.target.value || null)}
            />
          </label>
          {(suggestions.length > 0 || models.length > 0) && (
            <div className="chip-row">
              {suggestions.map((s) => (
                <button key={s} className="chip" title="Usar este modelo" onClick={() => setDefaultModel(s)}>
                  {s}
                </button>
              ))}
              {models.slice(0, 8).map((m) => (
                <button key={m.id} className="chip" title="Usar este modelo" onClick={() => setDefaultModel(m.id)}>
                  {m.id}
                </button>
              ))}
            </div>
          )}
          <details className="advanced">
            <summary>Opciones avanzadas</summary>
            <label>
              URL base (opcional)
              <input
                value={baseUrl ?? ""}
                placeholder="solo si usas un proxy o compatible"
                onChange={(e) => setBaseUrl(e.target.value || null)}
              />
            </label>
            <div className="row">
              <button disabled={busy} onClick={() => saveConfig(id, { default_model: defaultModel, base_url: baseUrl })}>
                Guardar cambios
              </button>
              <button
                className="btn-quiet"
                disabled={busy || !p.configured}
                onClick={() => loadModels(id)}
              >
                Descubrir modelos
              </button>
            </div>
          </details>
        </>
      )}
    </div>
  );
}

export function ProvidersView() {
  const { providers, loading, error, notice, refresh, clearNotice } = useProviders();

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const configured = providers.filter((p) => p.configured).length;

  return (
    <div className="view providers">
      <h2>Proveedores ✎</h2>
      <p className="muted">
        Conecta al menos uno para empezar.{" "}
        {providers.length > 0 && (
          <>
            {configured} de {providers.length} listos.
          </>
        )}
      </p>
      {error && <div className="alert error" onClick={clearNotice}>{error}</div>}
      {notice && <div className="alert ok" onClick={clearNotice}>{notice}</div>}
      {loading && <p className="muted">Cargando…</p>}
      <div className="provider-list">
        {providers.map((p) => (
          <ProviderCard key={p.id} id={p.id} />
        ))}
      </div>
      <StudySettingsCard />
    </div>
  );
}
