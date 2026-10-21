import { useState } from "react";
import { useDev } from "../../stores/dev";

// Opt-in switch for the generation inspector. While on, the backend keeps
// the FULL prompts, tool arguments and model output of every agent run in
// memory (never on disk) so they can be inspected in the dev panel.
export function DevModeCard() {
  const enabled = useDev((s) => s.enabled);
  const setEnabled = useDev((s) => s.setEnabled);
  const [error, setError] = useState<string | null>(null);

  const toggle = async (on: boolean) => {
    setError(null);
    try {
      await setEnabled(on);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="card">
      <div className="card-head">
        <h3>Modo desarrollador</h3>
        <span className={"badge" + (enabled ? " ok" : "")}>{enabled ? "activo" : "apagado"}</span>
      </div>
      <p className="hint">
        Muestra cómo se generan los bloques: prompts completos, agentes y modelo usado, llamadas a tools (con sus
        argumentos), veredictos del crítico, reintentos y errores del modelo. Los prompts pueden contener datos de tu
        estudio; las trazas viven solo en memoria y las claves de API siempre se ocultan.
      </p>
      <label className="hint">
        <input type="checkbox" checked={enabled} onChange={(e) => void toggle(e.target.checked)} /> Activar modo
        desarrollador
      </label>
      {error && <div className="alert error">{error}</div>}
    </div>
  );
}
