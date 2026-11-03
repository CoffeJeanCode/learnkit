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
      <div className="card-header">
        <div>
          <h3>Asignación Dinámica de Modelos por Rol</h3>
          <p className="muted text-xs">
            Desacopla tareas pesadas (generación de temarios/clases), tareas rápidas (calificador de compuertas, asistente léxico) y multimodales.
          </p>
        </div>
      </div>

      <div className="space-y-4 pt-3">
        {/* Tier Strong */}
        <div className="tier-row p-3 rounded-lg bg-paper-deep border border-slate-800">
          <div className="flex items-center justify-between mb-2">
            <span className="font-semibold text-sm text-sky-300">
              ⚡ Modelo Fuerte (Strong)
            </span>
            <span className="text-2xs text-slate-400">Generación de bloques y temarios</span>
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
            <label className="text-xs text-slate-300">
              Proveedor:
              <select
                className="w-full mt-1 p-2 rounded bg-input text-xs text-slate-200 border border-slate-700"
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
            </label>
            <label className="text-xs text-slate-300">
              Modelo:
              <input
                className="w-full mt-1 p-2 rounded bg-input text-xs text-slate-200 border border-slate-700"
                value={strongModel}
                placeholder="Por defecto del proveedor"
                onChange={(e) => setStrongModel(e.target.value)}
              />
            </label>
          </div>
          {strongPid && (
            <div className="flex flex-wrap gap-1 mt-2">
              {(SUGGESTIONS[providers.find((p) => p.id === strongPid)?.provider || ""]?.strong || []).map((m) => (
                <button
                  key={m}
                  type="button"
                  className={`chip text-2xs ${strongModel === m ? "is-active" : ""}`}
                  onClick={() => setStrongModel(m)}
                >
                  {m}
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Tier Cheap */}
        <div className="tier-row p-3 rounded-lg bg-paper-deep border border-slate-800">
          <div className="flex items-center justify-between mb-2">
            <span className="font-semibold text-sm text-amber-300">
              🌱 Modelo Económico / Rápido (Cheap)
            </span>
            <span className="text-2xs text-slate-400">Calificadores, compuertas, pistas y asistente</span>
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
            <label className="text-xs text-slate-300">
              Proveedor:
              <select
                className="w-full mt-1 p-2 rounded bg-input text-xs text-slate-200 border border-slate-700"
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
            </label>
            <label className="text-xs text-slate-300">
              Modelo:
              <input
                className="w-full mt-1 p-2 rounded bg-input text-xs text-slate-200 border border-slate-700"
                value={cheapModel}
                placeholder="Por defecto rápido del proveedor"
                onChange={(e) => setCheapModel(e.target.value)}
              />
            </label>
          </div>
          {cheapPid && (
            <div className="flex flex-wrap gap-1 mt-2">
              {(SUGGESTIONS[providers.find((p) => p.id === cheapPid)?.provider || ""]?.cheap || []).map((m) => (
                <button
                  key={m}
                  type="button"
                  className={`chip text-2xs ${cheapModel === m ? "is-active" : ""}`}
                  onClick={() => setCheapModel(m)}
                >
                  {m}
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Tier Vision */}
        <div className="tier-row p-3 rounded-lg bg-paper-deep border border-slate-800">
          <div className="flex items-center justify-between mb-2">
            <span className="font-semibold text-sm text-emerald-300">
              👁️ Modelo Multimodal (Vision)
            </span>
            <span className="text-2xs text-slate-400">Inspección de diagramas y capturas</span>
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
            <label className="text-xs text-slate-300">
              Proveedor:
              <select
                className="w-full mt-1 p-2 rounded bg-input text-xs text-slate-200 border border-slate-700"
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
            </label>
            <label className="text-xs text-slate-300">
              Modelo:
              <input
                className="w-full mt-1 p-2 rounded bg-input text-xs text-slate-200 border border-slate-700"
                value={visionModel}
                placeholder="Por defecto multimodal del proveedor"
                onChange={(e) => setVisionModel(e.target.value)}
              />
            </label>
          </div>
          {visionPid && (
            <div className="flex flex-wrap gap-1 mt-2">
              {(SUGGESTIONS[providers.find((p) => p.id === visionPid)?.provider || ""]?.vision || []).map((m) => (
                <button
                  key={m}
                  type="button"
                  className={`chip text-2xs ${visionModel === m ? "is-active" : ""}`}
                  onClick={() => setVisionModel(m)}
                >
                  {m}
                </button>
              ))}
            </div>
          )}
        </div>
      </div>

      <div className="flex items-center justify-between pt-4 mt-2 border-t border-slate-800">
        <button className="btn-primary" onClick={handleSave}>
          Guardar asignación de modelos
        </button>
        {savedNotice && (
          <span className="text-xs text-emerald-400 font-medium">✓ Cambios guardados</span>
        )}
      </div>
    </div>
  );
}
