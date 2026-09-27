import { useEffect, useState } from "react";
import type { RoadmapPhase } from "../lib/schemas";
import { useRoadmap } from "../stores/roadmap";
import { useUi } from "../stores/ui";

// User-facing gate labels (mirrors RoadmapView's tracker).
const PHASE_LABEL: Record<RoadmapPhase, string> = {
  onboarding: "1. Tu meta",
  diagnostic: "2. Diagnóstico",
  roadmap: "3. Tu plan",
};

function formatDate(ms: number): string {
  try {
    return new Date(ms).toLocaleString(undefined, {
      day: "numeric",
      month: "short",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return "";
  }
}

// Slide-over panel with the saved roadmap sessions. Refreshes every time it
// opens so newly sealed sessions show up without a reload. Each row opens on
// click and offers inline rename (✎) plus two-step delete (🗑 → confirm).
export function SessionsDrawer({ open, onClose }: { open: boolean; onClose: () => void }) {
  const {
    saved,
    savedLoading,
    error,
    refreshSaved,
    openSession,
    renameSession,
    deleteSession,
    session,
  } = useRoadmap();
  const setView = useUi((s) => s.setView);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draftTitle, setDraftTitle] = useState("");
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      refreshSaved();
      setEditingId(null);
      setConfirmDeleteId(null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open ]);

  if (!open) return null;

  const choose = async (id: string) => {
    await openSession(id);
    onClose();
    // Opening a plan always lands on the plan — the drawer is reachable
    // from the notebook view too.
    setView("roadmap");
  };

  const commitRename = async (id: string) => {
    if (!draftTitle.trim()) return;
    await renameSession(id, draftTitle);
    setEditingId(null);
  };

  const confirmDelete = async (id: string) => {
    await deleteSession(id);
    setConfirmDeleteId(null);
  };

  return (
    <div className="drawer-overlay" onClick={onClose}>
      <aside className="drawer" onClick={(e) => e.stopPropagation()}>
        <div className="drawer-head">
          <h3>Sesiones guardadas ✎</h3>
          <button className="drawer-close" onClick={onClose} title="Cerrar">
            ✕
          </button>
        </div>
        {error && <div className="alert error">{error}</div>}
        {savedLoading && <p className="muted">Buscando en el diario…</p>}
        {!savedLoading && saved.length === 0 && (
          <p className="muted">Aún no hay sesiones guardadas. ¡Empieza una con Nueva sesión!</p>
        )}
        <div className="drawer-list">
          {saved.map((s) => (
            <div
              key={s.session_id}
              className={"drawer-item" + (session?.session_id === s.session_id ? " current" : "")}
            >
              {editingId === s.session_id ? (
                <div className="drawer-edit">
                  <input
                    value={draftTitle}
                    onChange={(e) => setDraftTitle(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") commitRename(s.session_id);
                      if (e.key === "Escape") setEditingId(null);
                    }}
                    maxLength={80}
                    autoFocus
                  />
                  <button title="Guardar" onClick={() => commitRename(s.session_id)}>
                    ✓
                  </button>
                  <button title="Cancelar" onClick={() => setEditingId(null)}>
                    ✕
                  </button>
                </div>
              ) : (
                <button className="drawer-item-open" onClick={() => choose(s.session_id)}>
                  <span className="drawer-item-title">{s.title}</span>
                  <span className="drawer-item-meta">
                    {PHASE_LABEL[s.phase]} · {s.status === "sealed" ? "listo ✓" : "en curso"} ·{" "}
                    {formatDate(s.updated_at_ms)}
                  </span>
                </button>
              )}
              <div className="drawer-item-actions">
                {confirmDeleteId === s.session_id ? (
                  <>
                    <button
                      className="drawer-danger"
                      onClick={() => confirmDelete(s.session_id)}
                      title="Confirmar borrado"
                    >
                      Sí, borrar
                    </button>
                    <button onClick={() => setConfirmDeleteId(null)} title="Cancelar">
                      No
                    </button>
                  </>
                ) : (
                  <>
                    <button
                      title="Renombrar"
                      onClick={() => {
                        setEditingId(s.session_id);
                        setDraftTitle(s.title);
                        setConfirmDeleteId(null);
                      }}
                    >
                      ✎
                    </button>
                    <button title="Borrar" onClick={() => setConfirmDeleteId(s.session_id)}>
                      🗑
                    </button>
                  </>
                )}
              </div>
            </div>
          ))}
        </div>
      </aside>
    </div>
  );
}
