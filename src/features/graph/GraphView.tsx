import { useEffect, useMemo, useState } from "react";
import { useNotebookNav } from "../../stores/notebook";
import { useGraph } from "../../stores/graph";
import { useRoadmap } from "../../stores/roadmap";
import { useUi } from "../../stores/ui";
import { GraphCanvasD3 } from "./GraphCanvasD3";
import { GraphContextMenu, type ContextMenuState } from "./GraphContextMenu";
import { GraphSidebar } from "./GraphSidebar";
import type { Selection } from "./GraphParts";
import { classLabel } from "./graph";
import { IconClose, IconPlus, IconSidebar } from "../../components/Icons";

export function GraphView() {
  const sessionCourse = useRoadmap((s) => s.session?.imported_course_id ?? null);
  const navCourse = useNotebookNav((s) => s.courseId);
  const courseId = sessionCourse ?? navCourse;
  const setView = useUi((s) => s.setView);
  const setActiveClass = useNotebookNav((s) => s.setActiveClass);

  const {
    classes,
    graph,
    loading,
    error,
    load,
    clearError,
    addConcept,
    renameConcept,
    removeConcept,
    addLink,
    editLink,
    removeLink,
  } = useGraph();

  // Selection & Sidebar
  const [selection, setSelection] = useState<Selection>(null);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [searchQuery, setSearchQuery] = useState("");

  // Context Menu state
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);

  // Connection mode (from ID)
  const [connectingFromId, setConnectingFromId] = useState<string | null>(null);

  // New Concept dialog
  const [conceptModal, setConceptModal] = useState<{
    open: boolean;
    classId: string;
    label: string;
    sourceNodeId?: string;
  } | null>(null);

  useEffect(() => {
    if (courseId) void load(courseId);
  }, [courseId, load]);

  const firstClassId = classes[0]?.id ?? "";

  const handleSelect = (s: Selection) => {
    setSelection(s);
    if (s && !sidebarOpen) setSidebarOpen(true);
    clearError();
  };

  const handleNavigateToClass = (classId: string) => {
    setActiveClass(classId);
    setView("notebook");
  };

  const handleOpenNewConceptModal = (
    classId?: string,
    sourceNodeId?: string,
  ) => {
    setConceptModal({
      open: true,
      classId: classId || firstClassId,
      label: "",
      sourceNodeId,
    });
  };

  const handleCreateConcept = async (classId: string, label: string) => {
    if (!classId || !label.trim()) return;
    const created = await addConcept(classId, label.trim());
    if (created) {
      if (conceptModal?.sourceNodeId) {
        // Automatically link the source node to this new concept (Figma-style drag-to-create)
        await addLink(conceptModal.sourceNodeId, created.id, "relacionado");
      }
      setSelection({ kind: "node", id: created.id });
      setConceptModal(null);
    }
  };

  if (!courseId) {
    return (
      <div className="view graph-view p-8">
        <h2>Tu grafo de ideas</h2>
        <p className="muted">
          Todavía no hay clases cargadas. Cuando tu sesión tenga un curso, aquí podrás conectar lo que aprendes con tus propias palabras.
        </p>
      </div>
    );
  }

  return (
    <div className="view graph-view graph-fullscreen">
      {/* Top right Toggle Sidebar trigger if closed */}
      {!sidebarOpen && (
        <button
          type="button"
          className="graph-sidebar-toggle-btn"
          onClick={() => setSidebarOpen(true)}
          title="Abrir panel lateral de detalles"
        >
          <IconSidebar />
          <span>Detalles</span>
        </button>
      )}

      {/* Main Fullscreen D3 Canvas */}
      <div className="graph-canvas-area">
        {error && (
          <div className="graph-floating-alert" onClick={clearError}>
            <span>{error}</span>
            <button type="button" onClick={clearError}>✕</button>
          </div>
        )}

        <GraphCanvasD3
          classes={classes}
          concepts={graph.concepts}
          links={graph.links}
          selection={selection}
          onSelect={handleSelect}
          onCreateLink={addLink}
          searchQuery={searchQuery}
          connectingFromId={connectingFromId}
          onCancelConnect={() => setConnectingFromId(null)}
          onNewConceptAt={(canvasX, canvasY, sourceNodeId) => {
            handleOpenNewConceptModal(undefined, sourceNodeId);
          }}
          onOpenContextMenu={(e, type, targetId, worldPos, nodeInfo, linkInfo) => {
            setContextMenu({
              x: e.clientX,
              y: e.clientY,
              canvasX: worldPos?.x,
              canvasY: worldPos?.y,
              targetType: type,
              targetId,
              nodeKind: nodeInfo?.kind,
              nodeLabel: nodeInfo?.label,
              linkLabel: linkInfo?.label,
            });
          }}
        />
      </div>

      {/* Inspector Sidebar (Details & Overview) */}
      {sidebarOpen && (
        <GraphSidebar
          classes={classes}
          concepts={graph.concepts}
          links={graph.links}
          selection={selection}
          onSelect={handleSelect}
          onClose={() => setSidebarOpen(false)}
          onNewConcept={(classId) => handleOpenNewConceptModal(classId)}
          onStartConnect={(fromId) => setConnectingFromId(fromId)}
          onRenameConcept={renameConcept}
          onRemoveConcept={removeConcept}
          onEditLink={editLink}
          onRemoveLink={removeLink}
          onNavigateToClass={handleNavigateToClass}
          searchQuery={searchQuery}
          onSearchChange={setSearchQuery}
        />
      )}

      {/* Context Menu Overlay */}
      {contextMenu && (
        <GraphContextMenu
          menu={contextMenu}
          onClose={() => setContextMenu(null)}
          onNewConcept={(canvasX, canvasY, classId) =>
            handleOpenNewConceptModal(classId)
          }
          onConnectFrom={(nodeId) => setConnectingFromId(nodeId)}
          onEditNode={(nodeId) => {
            setSelection({ kind: "node", id: nodeId });
            setSidebarOpen(true);
          }}
          onViewDetails={(nodeId) => {
            setSelection({ kind: "node", id: nodeId });
            setSidebarOpen(true);
          }}
          onDeleteNode={async (nodeId) => {
            const ok = await removeConcept(nodeId);
            if (ok && selection?.kind === "node" && selection.id === nodeId) {
              setSelection(null);
            }
          }}
          onNavigateToClass={handleNavigateToClass}
          onEditLink={(linkId) => {
            setSelection({ kind: "link", id: linkId });
            setSidebarOpen(true);
          }}
          onDeleteLink={async (linkId) => {
            const ok = await removeLink(linkId);
            if (ok && selection?.kind === "link" && selection.id === linkId) {
              setSelection(null);
            }
          }}
          onResetZoom={() => {
            // Triggered inside D3 canvas via context menu
          }}
          onReorganize={() => {
            // Re-warm simulation
          }}
        />
      )}

      {/* Modal: New Concept */}
      {conceptModal?.open && (
        <div className="graph-modal-backdrop">
          <div className="graph-modal-box">
            <div className="flex items-center justify-between pb-2 border-b border-slate-800">
              <h4 className="text-sm font-semibold text-slate-100 flex items-center gap-1.5">
                <IconPlus /> Nuevo concepto
              </h4>
              <button
                type="button"
                className="text-slate-400 hover:text-slate-200"
                onClick={() => setConceptModal(null)}
              >
                <IconClose />
              </button>
            </div>

            <div className="space-y-3 pt-2">
              <div>
                <label className="block text-2xs text-slate-400 uppercase tracking-wider mb-1">
                  Clase de origen
                </label>
                <select
                  className="w-full text-xs p-2 rounded bg-input text-slate-100 border border-slate-700"
                  value={conceptModal.classId}
                  onChange={(e) =>
                    setConceptModal({ ...conceptModal, classId: e.target.value })
                  }
                >
                  {classes.map((c) => (
                    <option key={c.id} value={c.id}>
                      {classLabel(c.title)}
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label className="block text-2xs text-slate-400 uppercase tracking-wider mb-1">
                  Nombre o síntesis de la idea
                </label>
                <input
                  type="text"
                  autoFocus
                  className="w-full text-xs p-2 rounded bg-input text-slate-100 border border-slate-700 placeholder:text-slate-500"
                  placeholder="Ej. La densidad multiplica el base sin sustituirlo…"
                  value={conceptModal.label}
                  onChange={(e) =>
                    setConceptModal({ ...conceptModal, label: e.target.value })
                  }
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && conceptModal.label.trim()) {
                      void handleCreateConcept(conceptModal.classId, conceptModal.label);
                    }
                  }}
                />
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 pt-3 border-t border-slate-800">
              <button
                type="button"
                className="btn-quiet text-xs py-1 px-3"
                onClick={() => setConceptModal(null)}
              >
                Cancelar
              </button>
              <button
                type="button"
                className="btn-primary text-xs py-1 px-3"
                disabled={!conceptModal.label.trim()}
                onClick={() =>
                  void handleCreateConcept(conceptModal.classId, conceptModal.label)
                }
              >
                Crear concepto
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
