import { useEffect, useState } from "react";
import type { RoadmapPhase } from "../lib/schemas";
import { getRoadmapSession, listCourseClasses } from "../lib/tauri";
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
  // Class count per session being confirmed for deletion — `null` = still
  // loading or the session owns no course (nothing to cascade).
  const [deleteCounts, setDeleteCounts] = useState<Record<string, number | null>>({});

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
    // Land by state, not by habit: a sealed session has real content waiting
    // in Plan/Clases, so the drawer (which is where you go to pick a course)
    // drops you on the plan; a session still being built goes to its
    // conversation. Either way the header chip now names what you opened.
    const opened = useRoadmap.getState().session;
    setView(opened?.status === "sealed" ? "plan" : "roadmap");
  };

  const beginDelete = async (summary: { session_id: string; status: string; imported_course_id?: string | null }) => {
    setConfirmDeleteId(summary.session_id);
    setDeleteCounts((c) => ({ ...c, [summary.session_id]: null }));
    // The summary doesn't carry the course id — ask the full session, then
    // count its classes so the confirmation can say what's actually lost.
    try {
      const full = await getRoadmapSession(summary.session_id);
      if (!full.imported_course_id) {
        setDeleteCounts((c) => ({ ...c, [summary.session_id]: 0 }));
        return;
      }
      const classes = await listCourseClasses(full.imported_course_id);
      setDeleteCounts((c) => ({ ...c, [summary.session_id]: classes.length }));
    } catch {
      // Unknown count — the confirmation degrades to a plain session delete.
      setDeleteCounts((c) => ({ ...c, [summary.session_id]: null }));
    }
  };

  const commitRename = async (id: string) => {
    if (!draftTitle.trim()) return;
    await renameSession(id, draftTitle);
    setEditingId(null);
  };

  const confirmDelete = async (id: string) => {
    const wasActive = useRoadmap.getState().session?.session_id === id;
    await deleteSession(id);
    setConfirmDeleteId(null);
    // The deleted session WAS this one's container (its course and classes
    // just cascaded away with it) — a Plan/Clases section would have nothing
    // left to show, so fall back to the conversation prompt.
    if (wasActive) setView("roadmap");
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
                    <button title="Borrar" onClick={() => void beginDelete(s)}>
                      🗑
                    </button>
                  </>
                )}
              </div>
              {confirmDeleteId === s.session_id && (
                <p className="drawer-danger-text">
                  {deleteCounts[s.session_id] != null && deleteCounts[s.session_id]! > 0
                    ? `Se borrarán la sesión y sus ${deleteCounts[s.session_id]} clases.`
                    : "Se borrará la sesión."}
                </p>
              )}
            </div>
          ))}
        </div>
      </aside>
    </div>
  );
}
