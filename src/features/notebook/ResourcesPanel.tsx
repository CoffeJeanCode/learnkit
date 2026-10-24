import { useCallback, useEffect, useState } from "react";
import type { ClassResource, CourseResearch } from "../../lib/schemas";
import {
  getClassResources,
  getCourseResearch,
  getSearchKeyStatus,
  onResearchStatus,
  openClassResource,
  startCourseResearch,
  tauriError,
} from "../../lib/tauri";
import { useUi } from "../../stores/ui";
import { KIND_LABEL, panelView, shouldAutoStart } from "./resourcesPanelModel";

/**
 * Real tutorials, docs and exercises found for THIS class, so it can be
 * practiced and not only read. They come from a background research that runs
 * once when the course is sealed (see the backend `research_service`); every
 * link was returned by an actual web search — the model only chose among them.
 *
 * Opening a link goes through the backend, which only opens URLs on this
 * class's list.
 */
export function ResourcesPanel({ classId, courseId }: { classId: string; courseId: string }) {
  const setView = useUi((s) => s.setView);
  const [research, setResearch] = useState<CourseResearch | null>(null);
  const [resources, setResources] = useState<ClassResource[]>([]);
  const [keyConfigured, setKeyConfigured] = useState(false);
  const [openError, setOpenError] = useState<string | null>(null);
  const [retrying, setRetrying] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [r, list, key] = await Promise.all([getCourseResearch(courseId), getClassResources(classId), getSearchKeyStatus()]);
      setResearch(r);
      setResources(list);
      setKeyConfigured(key.configured);
    } catch (e) {
      // Resources are an extra: a failure to read them must never break the class.
      console.error("ResourcesPanel: refresh failed", e);
    }
  }, [classId, courseId]);

  // Reset on class change so the previous class's links never flash under the new one.
  useEffect(() => {
    setResearch(null);
    setResources([]);
    setOpenError(null);
    void refresh();
  }, [refresh]);

  // The backend tells us when this course's research changes state.
  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | null = null;
    onResearchStatus((e) => {
      if (alive && e.course_id === courseId) void refresh();
    }).then((u) => {
      if (alive) unlisten = u;
      else u();
    });
    return () => {
      alive = false;
      unlisten?.();
    };
  }, [courseId, refresh]);

  // Start the research when opening the panel makes sense (see `shouldAutoStart`).
  useEffect(() => {
    if (shouldAutoStart(research, keyConfigured)) {
      startCourseResearch(courseId).catch((e) => console.error("start_course_research failed", e));
    }
  }, [research, keyConfigured, courseId]);

  const retry = async () => {
    setRetrying(true);
    try {
      await startCourseResearch(courseId);
      await refresh();
    } catch (e) {
      setOpenError(tauriError(e).message);
    } finally {
      setRetrying(false);
    }
  };

  const open = async (url: string) => {
    setOpenError(null);
    try {
      await openClassResource(classId, url);
    } catch (e) {
      setOpenError(tauriError(e).message);
    }
  };

  const view = panelView(research, resources);
  if (view.kind === "none") return null; // nothing useful found: do not clutter the sidebar

  return (
    <div className="card resources-panel">
      <div className="card-head">
        <h3>Para practicar</h3>
      </div>

      {view.kind === "searching" && (
        <div role="status" aria-live="polite">
          <p className="hint">Buscando tutoriales y guías reales para tus clases…</p>
          <div className="gen-skeleton gen-skeleton-text" aria-hidden="true">
            <span className="sk-line" style={{ width: "92%" }} />
            <span className="sk-line" style={{ width: "70%" }} />
            <span className="sk-line" style={{ width: "84%" }} />
          </div>
        </div>
      )}

      {view.kind === "needs_key" && (
        <>
          <p className="hint">
            Conecta un buscador y LearnKit encontrará tutoriales, documentación y ejercicios reales para cada clase. Es opcional:
            tus clases funcionan igual sin él.
          </p>
          <div className="row">
            <button className="btn-quiet" onClick={() => setView("providers")}>
              Conectar buscador
            </button>
          </div>
        </>
      )}

      {view.kind === "failed" && (
        <>
          <p className="hint">No pude buscar recursos esta vez{view.detail ? ` (${view.detail})` : ""}. Tus clases funcionan igual.</p>
          <div className="row">
            <button className="btn-quiet" onClick={() => void retry()} disabled={retrying}>
              {retrying ? "Buscando…" : "Buscar de nuevo"}
            </button>
          </div>
        </>
      )}

      {view.kind === "list" && (
        <ul className="resource-list">
          {view.resources.map((r) => (
            <li key={r.url} className="resource-item">
              <span className="resource-kind">{KIND_LABEL[r.kind]}</span>
              <button type="button" className="resource-link" onClick={() => void open(r.url)} title="Abrir en el navegador">
                {r.title} ↗
              </button>
              <p className="hint">{r.why}</p>
              {r.whatToDo && (
                <p className="resource-todo">
                  <strong>Empieza por:</strong> {r.whatToDo}
                </p>
              )}
            </li>
          ))}
        </ul>
      )}

      {openError && (
        <p className="field-error" role="alert">
          {openError}
        </p>
      )}
    </div>
  );
}
