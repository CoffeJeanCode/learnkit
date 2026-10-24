import { useCallback, useEffect, useState } from "react";
import type { SearchKeyStatus } from "../../lib/schemas";
import { deleteSearchKey, getSearchKeyStatus, saveSearchKey, tauriError } from "../../lib/tauri";

/**
 * The search key LearnKit uses to find real tutorials and guides for each class
 * (optional: without it classes work exactly as before). Kept encrypted in the
 * same vault as the model keys and never shown again after saving.
 */
export function SearchKeyCard() {
  const [status, setStatus] = useState<SearchKeyStatus | null>(null);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ kind: "ok" | "error"; text: string } | null>(null);

  const refresh = useCallback(() => {
    getSearchKeyStatus()
      .then(setStatus)
      .catch((e) => setMessage({ kind: "error", text: tauriError(e).message }));
  }, []);
  useEffect(refresh, [refresh]);

  const save = async () => {
    if (!key.trim()) return;
    setBusy(true);
    setMessage(null);
    try {
      await saveSearchKey(key);
      setKey("");
      setMessage({ kind: "ok", text: "Clave guardada. Tus clases nuevas buscarán recursos reales." });
      refresh();
    } catch (e) {
      setMessage({ kind: "error", text: tauriError(e).message });
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setBusy(true);
    setMessage(null);
    try {
      await deleteSearchKey();
      setMessage({ kind: "ok", text: "Clave eliminada." });
      refresh();
    } catch (e) {
      setMessage({ kind: "error", text: tauriError(e).message });
    } finally {
      setBusy(false);
    }
  };

  const configured = status?.configured ?? false;
  return (
    <div className="card provider">
      <div className="provider-head">
        <span className="provider-name">Buscador de recursos (opcional)</span>
        <span className="provider-head-right">
          <span className={configured ? "badge ok" : "badge"}>
            {configured ? (status?.source === "env" ? "Lista ✓ (variable de entorno)" : "Lista ✓") : "Sin llave"}
          </span>
        </span>
      </div>
      <p className="hint">
        Con una clave de Tavily, LearnKit busca tutoriales, documentación y ejercicios <strong>reales</strong> para cada clase
        (Linux, CAD, fotografía…) en vez de dejarlas solo en teoría. Solo se envían los temas de tus clases; tu clave se guarda
        cifrada en este equipo y no se vuelve a mostrar.
      </p>
      <div className="row">
        <input
          type="password"
          value={key}
          onChange={(e) => setKey(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && void save()}
          placeholder="tvly-…"
          autoComplete="off"
          disabled={busy}
          aria-label="Clave del buscador"
        />
        <button className="btn-primary" onClick={() => void save()} disabled={busy || !key.trim()}>
          {busy ? "Guardando…" : "Guardar"}
        </button>
        {status?.source === "vault" && (
          <button className="btn-quiet" onClick={() => void remove()} disabled={busy}>
            Quitar
          </button>
        )}
      </div>
      {message && <div className={`alert ${message.kind}`}>{message.text}</div>}
    </div>
  );
}
