import { useEffect, useState } from "react";
import { exportStudyReport, type StudyVariantMode } from "../../lib/tauri";
import { useStudy } from "../../stores/study";

const MODE_LABEL: Record<StudyVariantMode, string> = {
  random: "Aleatoria (asignada una sola vez)",
  gamified: "Con mapa de capacidades y logros",
  plain: "Sin mapa de capacidades ni logros",
};

// For whoever runs the study. Both versions teach, practise, give feedback and
// assess identically (including the transfer challenge and the graded
// retrieval); only the PRESENTATION of progress differs.
export function StudySettingsCard() {
  const settings = useStudy((s) => s.settings);
  const load = useStudy((s) => s.load);
  const setMode = useStudy((s) => s.setMode);
  const [exportPath, setExportPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!settings) void load();
  }, [settings, load]);

  if (!settings) return null;

  const change = async (mode: StudyVariantMode) => {
    setError(null);
    try {
      await setMode(mode);
    } catch (e) {
      setError(String(e));
    }
  };

  const exportData = async () => {
    setError(null);
    try {
      setExportPath(await exportStudyReport());
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="card">
      <div className="card-head">
        <h3>Versión del estudio</h3>
        <span className="badge">{settings.variant === "gamified" ? "con logros" : "sin logros"}</span>
      </div>
      <p className="hint">
        Las dos versiones enseñan, practican, dan feedback y evalúan igual. Solo cambia cómo se muestra el progreso
        (mapa de capacidades y logros). Cambiarla a mitad del estudio queda registrado en los datos.
      </p>
      <label className="hint">
        Versión{" "}
        <select value={settings.mode} onChange={(e) => void change(e.target.value as StudyVariantMode)}>
          {(Object.keys(MODE_LABEL) as StudyVariantMode[]).map((m) => (
            <option key={m} value={m}>
              {MODE_LABEL[m]}
            </option>
          ))}
        </select>
      </label>
      <p className="hint">
        Participante: <code>{settings.participantId}</code>
        {settings.randomAssignment ? ` · asignación aleatoria: ${settings.randomAssignment}` : ""}
      </p>
      <div className="row">
        <button onClick={() => void exportData()}>Exportar datos del estudio</button>
      </div>
      {exportPath && <p className="hint">Guardado en: {exportPath}</p>}
      {error && <div className="alert error">{error}</div>}
    </div>
  );
}
