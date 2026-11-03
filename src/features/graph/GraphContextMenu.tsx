import { useEffect, useRef } from "react";
import {
  IconClose,
  IconLink,
  IconPencil,
  IconPlus,
  IconRefresh,
  IconSidebar,
  IconTrash,
} from "../../components/Icons";

export interface ContextMenuState {
  x: number;
  y: number;
  canvasX?: number;
  canvasY?: number;
  targetType: "canvas" | "node" | "link";
  targetId?: string;
  nodeKind?: "class" | "concept";
  nodeLabel?: string;
  linkLabel?: string;
}

export function GraphContextMenu({
  menu,
  onClose,
  onNewConcept,
  onConnectFrom,
  onEditNode,
  onViewDetails,
  onDeleteNode,
  onNavigateToClass,
  onEditLink,
  onDeleteLink,
  onResetZoom,
  onReorganize,
}: {
  menu: ContextMenuState;
  onClose: () => void;
  onNewConcept: (canvasX?: number, canvasY?: number, classId?: string) => void;
  onConnectFrom: (nodeId: string) => void;
  onEditNode: (nodeId: string) => void;
  onViewDetails: (nodeId: string) => void;
  onDeleteNode: (nodeId: string) => void;
  onNavigateToClass: (classId: string) => void;
  onEditLink: (linkId: string) => void;
  onDeleteLink: (linkId: string) => void;
  onResetZoom: () => void;
  onReorganize: () => void;
}) {
  const menuRef = useRef<HTMLDivElement>(null);

  // Close on Escape or click outside
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    const handlePointerDown = (e: PointerEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("pointerdown", handlePointerDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("pointerdown", handlePointerDown);
    };
  }, [onClose]);

  // Keep menu inside viewport boundaries
  const menuWidth = 220;
  const menuHeight = 240;
  const left = Math.max(12, Math.min(menu.x, window.innerWidth - menuWidth - 12));
  const top = Math.max(12, Math.min(menu.y, window.innerHeight - menuHeight - 12));

  return (
    <div
      ref={menuRef}
      className="graph-context-menu"
      style={{ left: `${left}px`, top: `${top}px` }}
      role="menu"
      aria-label="Menú contextual del grafo"
    >
      {menu.targetType === "canvas" && (
        <>
          <div className="graph-menu-header">Lienzo</div>
          <button
            type="button"
            className="graph-menu-item"
            role="menuitem"
            onClick={() => {
              onClose();
              onNewConcept(menu.canvasX, menu.canvasY);
            }}
          >
            <IconPlus />
            <span>Nuevo concepto aquí</span>
          </button>
          <button
            type="button"
            className="graph-menu-item"
            role="menuitem"
            onClick={() => {
              onClose();
              onResetZoom();
            }}
          >
            <IconRefresh />
            <span>Centrar lienzo</span>
          </button>
          <button
            type="button"
            className="graph-menu-item"
            role="menuitem"
            onClick={() => {
              onClose();
              onReorganize();
            }}
          >
            <IconRefresh />
            <span>Reorganizar ideas</span>
          </button>
        </>
      )}

      {menu.targetType === "node" && menu.targetId && (
        <>
          <div className="graph-menu-header">
            {menu.nodeKind === "class" ? "Clase" : "Concepto"}: {menu.nodeLabel || "Nodo"}
          </div>
          <button
            type="button"
            className="graph-menu-item"
            role="menuitem"
            onClick={() => {
              onClose();
              onConnectFrom(menu.targetId!);
            }}
          >
            <IconLink />
            <span>Conectar con otra idea…</span>
          </button>
          <button
            type="button"
            className="graph-menu-item"
            role="menuitem"
            onClick={() => {
              onClose();
              onViewDetails(menu.targetId!);
            }}
          >
            <IconSidebar />
            <span>Ver detalles en sidebar</span>
          </button>
          {menu.nodeKind === "concept" ? (
            <>
              <button
                type="button"
                className="graph-menu-item"
                role="menuitem"
                onClick={() => {
                  onClose();
                  onEditNode(menu.targetId!);
                }}
              >
                <IconPencil />
                <span>Editar concepto</span>
              </button>
              <div className="graph-menu-divider" />
              <button
                type="button"
                className="graph-menu-item is-danger"
                role="menuitem"
                onClick={() => {
                  onClose();
                  onDeleteNode(menu.targetId!);
                }}
              >
                <IconTrash />
                <span>Eliminar concepto</span>
              </button>
            </>
          ) : (
            <>
              <button
                type="button"
                className="graph-menu-item"
                role="menuitem"
                onClick={() => {
                  onClose();
                  onNewConcept(undefined, undefined, menu.targetId);
                }}
              >
                <IconPlus />
                <span>Añadir concepto a esta clase</span>
              </button>
              <button
                type="button"
                className="graph-menu-item"
                role="menuitem"
                onClick={() => {
                  onClose();
                  onNavigateToClass(menu.targetId!);
                }}
              >
                <IconSidebar />
                <span>Ir a esta clase</span>
              </button>
            </>
          )}
        </>
      )}

      {menu.targetType === "link" && menu.targetId && (
        <>
          <div className="graph-menu-header">
            Relación: {menu.linkLabel || "Enlace"}
          </div>
          <button
            type="button"
            className="graph-menu-item"
            role="menuitem"
            onClick={() => {
              onClose();
              onEditLink(menu.targetId!);
            }}
          >
            <IconPencil />
            <span>Editar relación</span>
          </button>
          <div className="graph-menu-divider" />
          <button
            type="button"
            className="graph-menu-item is-danger"
            role="menuitem"
            onClick={() => {
              onClose();
              onDeleteLink(menu.targetId!);
            }}
          >
            <IconTrash />
            <span>Eliminar relación</span>
          </button>
        </>
      )}
    </div>
  );
}
