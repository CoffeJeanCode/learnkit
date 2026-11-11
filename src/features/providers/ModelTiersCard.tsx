import { useEffect, useState } from "react";
import { useProviders } from "../../stores/providers";
import type { ModelTierConfig } from "../../types";

const SUGGESTIONS: Record<string, { strong: string[]; cheap: string[]; vision: string[]; escalation: string[] }> = {
  anthropic: {
    strong: ["claude-sonnet-5-5", "claude-opus-5-5"],
    cheap: ["claude-haiku-5-5"],
    vision: ["claude-sonnet-5-5"],
    escalation: ["claude-opus-5-5", "claude-sonnet-5-5"],
  },
  deepseek: {
    strong: ["deepseek-chat", "deepseek-reasoner"],
    cheap: ["deepseek-v4-flash", "deepseek-chat"],
    vision: ["deepseek-chat"],
    escalation: ["deepseek-reasoner", "deepseek-v4-pro"],
  },
  xiaomi: {
    strong: ["mimo-v2.6-pro", "mimo-v2.6-pro-ultraspeed"],
    cheap: ["mimo-v2.6-flash"],
    vision: ["mimo-v2.6-pro"],
    escalation: ["mimo-v2.6-pro"],
  },
  gemini: {
    strong: ["gemini-2.5-pro"],
    cheap: ["gemini-2.5-flash", "gemini-2.5-flash-lite"],
    vision: ["gemini-2.5-flash", "gemini-2.5-pro"],
    escalation: ["gemini-2.5-pro"],
  },
  openai: {
    strong: ["gpt-5", "o3"],
    cheap: ["gpt-4.1-mini", "o3-mini"],
    vision: ["gpt-5", "gpt-4o"],
    escalation: ["gpt-5"],
  },
  openrouter: {
    strong: ["anthropic/claude-sonnet-5-5", "deepseek/deepseek-r1"],
    cheap: ["anthropic/claude-haiku-5-5", "openai/gpt-4o-mini"],
    vision: ["google/gemini-2.5-flash", "openai/gpt-4o"],
    escalation: ["anthropic/claude-opus-5-5"],
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
  const [escalationPid, setEscalationPid] = useState("");
  const [escalationModel, setEscalationModel] = useState("");
  const [savedNotice, setSavedNotice] = useState(false);

  useEffect(() => {
    if (modelTiers) {
      setStrongPid(modelTiers.strongProviderId || activeProvider?.id || "");
      setStrongModel(modelTiers.strongModel || "");
      setCheapPid(modelTiers.cheapProviderId || activeProvider?.id || "");
      setCheapModel(modelTiers.cheapModel || "");
      setVisionPid(modelTiers.visionProviderId || activeProvider?.id || "");
      setVisionModel(modelTiers.visionModel || "");
      setEscalationPid(modelTiers.escalationProviderId || "");
      setEscalationModel(modelTiers.escalationModel || "");
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
      // Escalation is opt-in: no provider or no model = off.
      escalationProviderId: escalationPid && escalationModel.trim() ? escalationPid : null,
      escalationModel: escalationPid && escalationModel.trim() ? escalationModel.trim() : null,
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
        <span className="badge">{escalationPid && escalationModel.trim() ? "4 roles activos" : "3 roles activos"}</span>
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
        {/* Tier Escalation (optional) */}
        <div className="model-tier-row">
          <div className="model-tier-header">
            <div className="model-tier-title-group">
              <span className="model-tier-title">Escalado de figuras</span>
              <span className="model-tier-tag escalation">Opcional</span>
            </div>
            <span className="model-tier-desc">Toma un bloque cuya figura falló dos veces (temas difíciles): solo se paga cuando hace falta</span>
          </div>

          <div className="model-tier-fields">
            <div className="model-tier-field">
              <label htmlFor="tier-escalation-provider">Proveedor:</label>
              <select
                id="tier-escalation-provider"
                value={escalationPid}
                onChange={(e) => {
                  setEscalationPid(e.target.value);
                  const p = providers.find((x) => x.id === e.target.value);
                  setEscalationModel(p ? (SUGGESTIONS[p.provider]?.escalation[0] ?? "") : "");
                }}
              >
                <option value="">Desactivado</option>
                {configuredProviders.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name} {p.active ? "★" : ""}
                  </option>
                ))}
              </select>
            </div>

            <div className="model-tier-field">
              <label htmlFor="tier-escalation-model">Modelo:</label>
              <input
                id="tier-escalation-model"
                value={escalationModel}
                disabled={!escalationPid}
                placeholder="p. ej. claude-opus-5-5"
                onChange={(e) => setEscalationModel(e.target.value)}
              />
            </div>
          </div>

          {escalationPid && (
            <div className="model-tier-chips">
              {(SUGGESTIONS[providers.find((p) => p.id === escalationPid)?.provider || ""]?.escalation || []).map((m) => (
                <button
                  key={m}
                  type="button"
                  className={`model-tier-chip ${escalationModel === m ? "is-active" : ""}`}
                  onClick={() => setEscalationModel(m)}
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
