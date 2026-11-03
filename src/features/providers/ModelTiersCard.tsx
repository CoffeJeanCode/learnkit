import { useEffect, useState } from "react";
import { useProviders } from "../../stores/providers";
import type { ModelTierConfig } from "../../types";

const SUGGESTIONS: Record<string, { strong: string[]; cheap: string[]; vision: string[] }> = {
  anthropic: {
    strong: ["claude-sonnet-4-6", "claude-opus-4"],
    cheap: ["claude-haiku-4-5"],
    vision: ["claude-sonnet-4-6"],
  },
  deepseek: {
    strong: ["deepseek-chat", "deepseek-reasoner"],
    cheap: ["deepseek-v4-flash", "deepseek-chat"],
    vision: ["deepseek-chat"],
  },
  xiaomi: {
    strong: ["mimo-v2.6-pro", "mimo-v2.6-pro-ultraspeed"],
    cheap: ["mimo-v2.6-flash"],
    vision: ["mimo-v2.6-pro"],
  },
  gemini: {
    strong: ["gemini-2.5-pro"],
    cheap: ["gemini-2.5-flash", "gemini-2.5-flash-lite"],
    vision: ["gemini-2.5-flash", "gemini-2.5-pro"],
  },
  openai: {
    strong: ["gpt-5", "o3"],
    cheap: ["gpt-4.1-mini", "o3-mini"],
    vision: ["gpt-5", "gpt-4o"],
  },
  openrouter: {
    strong: ["anthropic/claude-sonnet-4", "deepseek/deepseek-r1"],
    cheap: ["anthropic/claude-3.5-haiku", "openai/gpt-4o-mini"],
    vision: ["google/gemini-2.5-flash", "openai/gpt-4o"],
  },
};

export function ModelTiersCard() {
  const providers = useProviders((s) => s.providers);
  const modelTiers = useProviders((s) => s.modelTiers);
  const saveModelTiers = useProviders((s) => s.saveModelTiers);

  const configuredProviders = providers.filter((p) => p.configured);
  const activeProvider = providers.find((p) => p.active) ?? configuredProviders[0];

  const [strongPid, setStrongPid] = useState("");
  const [strongModel, setStrongModel] = useState("");
  const [cheapPid, setCheapPid] = useState("");
  const [cheapModel, setCheapModel] = useState("");
  const [visionPid, setVisionPid] = useState("");
  const [visionModel, setVisionModel] = useState("");
  const [savedNotice, setSavedNotice] = useState(false);

  useEffect(() => {
    if (modelTiers) {
      setStrongPid(modelTiers.strongProviderId || activeProvider?.id || "");
      setStrongModel(modelTiers.strongModel || "");
      setCheapPid(modelTiers.cheapProviderId || activeProvider?.id || "");
      setCheapModel(modelTiers.cheapModel || "");
      setVisionPid(modelTiers.visionProviderId || activeProvider?.id || "");
      setVisionModel(modelTiers.visionModel || "");
    } else if (activeProvider) {
      setStrongPid(activeProvider.id);
      setCheapPid(activeProvider.id);
      setVisionPid(activeProvider.id);
    }
  }, [modelTiers, activeProvider]);

  const handleSave = async () => {
    const config: ModelTierConfig = {
      strongProviderId: strongPid || null,
      strongModel: strongModel.trim() || null,
      cheapProviderId: cheapPid || null,
      cheapModel: cheapModel.trim() || null,
      visionProviderId: visionPid || null,
      visionModel: visionModel.trim() || null,
    };
    await saveModelTiers(config);
    setSavedNotice(true);
    setTimeout(() => setSavedNotice(false), 3000);
  };

  if (configuredProviders.length === 0) return null;

  return (
    <div className="card model-tiers-card">
      <div className="card-head">
        <h3>Asignación de modelos por rol</h3>
        <span className="badge">3 roles activos</span>
      </div>

      <p className="hint">
        Desacopla tareas pesadas (temarios y clases), tareas rápidas (calificador de compuertas, asistente léxico) y multimodales (inspección de diagramas).
      </p>

      <div className="model-tiers-grid">
        {/* Tier Strong */}
        <div className="model-tier-row">
          <div className="model-tier-header">
            <div className="model-tier-title-group">
              <span className="model-tier-title">Modelo Fuerte</span>
              <span className="model-tier-tag strong">Strong</span>
            </div>
            <span className="model-tier-desc">Generación de bloques y temarios</span>
          </div>

          <div className="model-tier-fields">
            <div className="model-tier-field">
              <label htmlFor="tier-strong-provider">Proveedor:</label>
              <select
                id="tier-strong-provider"
                value={strongPid}
                onChange={(e) => {
                  setStrongPid(e.target.value);
                  const p = providers.find((x) => x.id === e.target.value);
                  if (p && SUGGESTIONS[p.provider]?.strong[0]) {
                    setStrongModel(SUGGESTIONS[p.provider].strong[0]);
                  }
                }}
              >
                {configuredProviders.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name} {p.active ? "★" : ""}
                  </option>
                ))}
              </select>
            </div>

            <div className="model-tier-field">
              <label htmlFor="tier-strong-model">Modelo:</label>
              <input
                id="tier-strong-model"
                value={strongModel}
                placeholder="Por defecto del proveedor"
                onChange={(e) => setStrongModel(e.target.value)}
              />
            </div>
          </div>

          {strongPid && (
            <div className="model-tier-chips">
              {(SUGGESTIONS[providers.find((p) => p.id === strongPid)?.provider || ""]?.strong || []).map((m) => (
                <button
                  key={m}
                  type="button"
                  className={`model-tier-chip ${strongModel === m ? "is-active" : ""}`}
                  onClick={() => setStrongModel(m)}
                >
                  {m}
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Tier Cheap */}
        <div className="model-tier-row">
          <div className="model-tier-header">
            <div className="model-tier-title-group">
              <span className="model-tier-title">Modelo Económico</span>
              <span className="model-tier-tag cheap">Rápido</span>
            </div>
            <span className="model-tier-desc">Calificadores, compuertas, pistas y asistente</span>
          </div>

          <div className="model-tier-fields">
            <div className="model-tier-field">
              <label htmlFor="tier-cheap-provider">Proveedor:</label>
              <select
                id="tier-cheap-provider"
                value={cheapPid}
                onChange={(e) => {
                  setCheapPid(e.target.value);
                  const p = providers.find((x) => x.id === e.target.value);
                  if (p && SUGGESTIONS[p.provider]?.cheap[0]) {
                    setCheapModel(SUGGESTIONS[p.provider].cheap[0]);
                  }
                }}
              >
                {configuredProviders.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name} {p.active ? "★" : ""}
                  </option>
                ))}
              </select>
            </div>

            <div className="model-tier-field">
              <label htmlFor="tier-cheap-model">Modelo:</label>
              <input
                id="tier-cheap-model"
                value={cheapModel}
                placeholder="Por defecto rápido del proveedor"
                onChange={(e) => setCheapModel(e.target.value)}
              />
            </div>
          </div>

          {cheapPid && (
            <div className="model-tier-chips">
              {(SUGGESTIONS[providers.find((p) => p.id === cheapPid)?.provider || ""]?.cheap || []).map((m) => (
                <button
                  key={m}
                  type="button"
                  className={`model-tier-chip ${cheapModel === m ? "is-active" : ""}`}
                  onClick={() => setCheapModel(m)}
                >
                  {m}
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Tier Vision */}
        <div className="model-tier-row">
          <div className="model-tier-header">
            <div className="model-tier-title-group">
              <span className="model-tier-title">Modelo Multimodal</span>
              <span className="model-tier-tag vision">Visión</span>
            </div>
            <span className="model-tier-desc">Inspección de diagramas y capturas</span>
          </div>

          <div className="model-tier-fields">
            <div className="model-tier-field">
              <label htmlFor="tier-vision-provider">Proveedor:</label>
              <select
                id="tier-vision-provider"
                value={visionPid}
                onChange={(e) => {
                  setVisionPid(e.target.value);
                  const p = providers.find((x) => x.id === e.target.value);
                  if (p && SUGGESTIONS[p.provider]?.vision[0]) {
                    setVisionModel(SUGGESTIONS[p.provider].vision[0]);
                  }
                }}
              >
                {configuredProviders.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name} {p.active ? "★" : ""}
                  </option>
                ))}
              </select>
            </div>

            <div className="model-tier-field">
              <label htmlFor="tier-vision-model">Modelo:</label>
              <input
                id="tier-vision-model"
                value={visionModel}
                placeholder="Por defecto multimodal del proveedor"
                onChange={(e) => setVisionModel(e.target.value)}
              />
            </div>
          </div>

          {visionPid && (
            <div className="model-tier-chips">
              {(SUGGESTIONS[providers.find((p) => p.id === visionPid)?.provider || ""]?.vision || []).map((m) => (
                <button
                  key={m}
                  type="button"
                  className={`model-tier-chip ${visionModel === m ? "is-active" : ""}`}
                  onClick={() => setVisionModel(m)}
                >
                  {m}
                </button>
              ))}
            </div>
          )}
        </div>
      </div>

      <div className="model-tiers-actions">
        <button className="btn-primary" onClick={handleSave}>
          Guardar asignación de modelos
        </button>
        {savedNotice && (
          <span className="model-tiers-saved-notice">✓ Cambios guardados</span>
        )}
      </div>
    </div>
  );
}
