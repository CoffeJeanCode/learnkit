import { useEffect, useState } from "react";
import { useProviders } from "../../stores/providers";

function ProviderCard({ id }: { id: string }) {
  const p = useProviders((s) => s.providers.find((x) => x.id === id));
  const busyId = useProviders((s) => s.busyId);
  const models = useProviders((s) => s.modelsCache[id] ?? []);
  const suggestions = useProviders((s) =>
    p ? s.suggestionsCache[p.provider] ?? [] : [],
  );
  const { saveConfig, saveKey, removeKey, test, loadModels, loadSuggestions } = useProviders.getState();

  const [apiKey, setApiKey] = useState("");
  const [defaultModel, setDefaultModel] = useState<string | null>(null);
  const [baseUrl, setBaseUrl] = useState<string | null>(null);

  useEffect(() => {
    if (p) {
      setDefaultModel(p.default_model);
      setBaseUrl(p.base_url);
      loadSuggestions(p.provider);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [p?.id]);

  if (!p) return null;
  const busy = busyId === id;

  return (
    <div className="card">
      <div className="card-head">
        <h3>{p.name}</h3>
        <span className={p.configured ? "badge ok" : "badge"}>
          {p.configured ? "Configured ✓" : "No key"}
        </span>
      </div>

      <label>
        API key
        <input
          type="password"
          placeholder={p.configured ? "•••••••• (stored, never shown)" : "sk-…"}
          value={apiKey}
          autoComplete="off"
          onChange={(e) => setApiKey(e.target.value)}
        />
      </label>
      <div className="row">
        <button disabled={busy || !apiKey.trim()} onClick={() => saveKey(id, apiKey).then(() => setApiKey(""))}>
          Save key
        </button>
        <button disabled={busy || !p.configured} onClick={() => removeKey(id)}>
          Delete key
        </button>
        <button disabled={busy || !p.configured} onClick={() => test(id)}>
          Test connection
        </button>
      </div>

      <label>
        Default model
        <input
          value={defaultModel ?? ""}
          placeholder={suggestions[0] ?? "model id (manual allowed)"}
          onChange={(e) => setDefaultModel(e.target.value || null)}
        />
      </label>
      {suggestions.length > 0 && (
        <div className="hint">Examples: {suggestions.join(", ")}</div>
      )}
      {models.length > 0 && (
        <div className="hint">Discovered: {models.slice(0, 8).map((m) => m.id).join(", ")}</div>
      )}

      <label>
        Base URL (optional)
        <input
          value={baseUrl ?? ""}
          placeholder={p.provider ? undefined : undefined}
          onChange={(e) => setBaseUrl(e.target.value || null)}
        />
      </label>

      <div className="row">
        <button
          disabled={busy}
          onClick={() => saveConfig(id, { default_model: defaultModel, base_url: baseUrl })}
        >
          Save config
        </button>
        <button disabled={busy || !p.configured} onClick={() => loadModels(id)}>
          Discover models
        </button>
      </div>
    </div>
  );
}

export function ProvidersView() {
  const { providers, loading, error, notice, refresh, clearNotice } = useProviders();

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="view">
      <h2>Settings › Providers (BYOK)</h2>
      <p className="muted">
        Keys are stored in Stronghold and never displayed again. Only non-sensitive
        config is persisted.
      </p>
      {error && <div className="alert error" onClick={clearNotice}>{error}</div>}
      {notice && <div className="alert ok" onClick={clearNotice}>{notice}</div>}
      {loading && <p>Loading…</p>}
      <div className="grid">
        {providers.map((p) => (
          <ProviderCard key={p.id} id={p.id} />
        ))}
      </div>
    </div>
  );
}
