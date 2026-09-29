import { useUpdater } from "../stores/updater";

/** Non-blocking "there is a new version" strip shown once the automatic
 *  startup check finds one. `Más tarde` only hides it for this session — the
 *  header's update button still installs it. */
export function UpdateBanner() {
  const status = useUpdater((s) => s.status);
  const update = useUpdater((s) => s.update);
  const dismissed = useUpdater((s) => s.dismissed);
  const installing = useUpdater((s) => s.status === "installing");
  const error = useUpdater((s) => s.error);
  const install = useUpdater((s) => s.install);
  const dismiss = useUpdater((s) => s.dismiss);

  if (status !== "available" || dismissed || !update) return null;

  return (
    <div className="update-banner" role="status">
      <span className="update-banner-text">
        <strong>Nueva versión {update.version}</strong>
        {update.body ? <span className="update-banner-notes">{update.body}</span> : null}
        {error ? <span className="update-banner-error">{error}</span> : null}
      </span>
      <span className="update-banner-actions">
        <button type="button" className="btn-primary" onClick={() => void install()} disabled={installing}>
          {installing ? "Instalando…" : "Actualizar ahora"}
        </button>
        <button type="button" className="btn-quiet" onClick={dismiss}>
          Más tarde
        </button>
      </span>
    </div>
  );
}
