import { useState } from "react";
import type { ClassRecord, Concept, ConceptLink } from "../../lib/schemas";
import { RELATION_HINTS, classLabel, truncate } from "./graph";
import type { Selection } from "./GraphParts";
import {
  IconClose,
  IconLink,
  IconPencil,
  IconPlus,
  IconSearch,
  IconSidebar,
  IconTrash,
} from "../../components/Icons";

export function GraphSidebar({
  classes,
  concepts,
  links,
  selection,
  onSelect,
  onClose,
  onNewConcept,
  onStartConnect,
  onRenameConcept,
  onRemoveConcept,
  onEditLink,
  onRemoveLink,
  onNavigateToClass,
  searchQuery,
  onSearchChange,
}: {
  classes: ClassRecord[];
  concepts: Concept[];
  links: ConceptLink[];
  selection: Selection;
  onSelect: (s: Selection) => void;
  onClose: () => void;
  onNewConcept: (classId?: string) => void;
  onStartConnect: (fromId: string) => void;
  onRenameConcept: (id: string, classId: string, label: string) => Promise<boolean>;
  onRemoveConcept: (id: string) => Promise<boolean>;
  onEditLink: (id: string, fromId: string, toId: string, label: string) => Promise<boolean>;
  onRemoveLink: (id: string) => Promise<boolean>;
  onNavigateToClass: (classId: string) => void;
  searchQuery: string;
  onSearchChange: (q: string) => void;
}) {
  // Editing states
  const [editingConceptLabel, setEditingConceptLabel] = useState<string | null>(null);
  const [editingLinkLabel, setEditingLinkLabel] = useState<string | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Selected entities
  const selectedNodeId = selection?.kind === "node" ? selection.id : null;
  const selectedConcept = selectedNodeId ? concepts.find((c) => c.id === selectedNodeId) : null;
  const selectedClass = selectedNodeId ? classes.find((c) => c.id === selectedNodeId) : null;
  const selectedLink = selection?.kind === "link" ? links.find((l) => l.id === selection.id) : null;

  // Node lookup maps
  const nodeNameMap = new Map<string, string>();
  classes.forEach((c) => nodeNameMap.set(c.id, classLabel(c.title)));
  concepts.forEach((c) => nodeNameMap.set(c.id, c.label));

  // Connected links for selected node
  const nodeLinks = selectedNodeId
    ? links.filter((l) => l.fromId === selectedNodeId || l.toId === selectedNodeId)
    : [];

  const handleSaveConceptRename = async () => {
    if (!selectedConcept || !editingConceptLabel?.trim()) return;
    setBusy(true);
    try {
      await onRenameConcept(selectedConcept.id, selectedConcept.classId, editingConceptLabel.trim());
      setEditingConceptLabel(null);
    } finally {
      setBusy(false);
    }
  };

  const handleSaveLinkEdit = async () => {
    if (!selectedLink || !editingLinkLabel?.trim()) return;
    setBusy(true);
    try {
      await onEditLink(selectedLink.id, selectedLink.fromId, selectedLink.toId, editingLinkLabel.trim());
      setEditingLinkLabel(null);
    } finally {
      setBusy(false);
    }
  };

  const handleDeleteConcept = async () => {
    if (!selectedConcept) return;
    setBusy(true);
    try {
      const ok = await onRemoveConcept(selectedConcept.id);
      if (ok) {
        setConfirmDeleteId(null);
        onSelect(null);
      }
    } finally {
      setBusy(false);
    }
  };

  const handleDeleteLink = async () => {
    if (!selectedLink) return;
    setBusy(true);
    try {
      const ok = await onRemoveLink(selectedLink.id);
      if (ok) {
        setConfirmDeleteId(null);
        onSelect(null);
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <aside className="graph-sidebar" aria-label="Detalles del grafo de conocimiento">
      {/* Top Header */}
      <div className="graph-sidebar-header">
        <div className="flex items-center gap-2 min-w-0">
          <IconSidebar className="text-slate-400 shrink-0" />
          <h3 className="text-sm font-semibold text-slate-100 truncate">
            {selectedConcept
              ? "Concepto"
              : selectedClass
                ? "Clase"
                : selectedLink
                  ? "Relación"
                  : "Explorador de ideas"}
          </h3>
        </div>
        <button
          type="button"
          className="graph-sidebar-close"
          onClick={onClose}
          title="Cerrar panel lateral"
          aria-label="Cerrar panel lateral"
        >
          <IconClose />
        </button>
      </div>

      <div className="graph-sidebar-body">
        {/* ===================== CONCEPT SELECTED ===================== */}
        {selectedConcept && (
          <div className="space-y-4">
            <div className="graph-sidebar-card">
              <span className="badge is-amber">Concepto propio</span>
              {editingConceptLabel !== null ? (
                <div className="mt-2 space-y-2">
                  <input
                    type="text"
                    className="w-full text-xs p-2 rounded bg-input text-slate-100 border border-slate-700"
                    value={editingConceptLabel}
                    onChange={(e) => setEditingConceptLabel(e.target.value)}
                    autoFocus
                  />
                  <div className="flex items-center gap-2">
                    <button
                      type="button"
                      className="btn-primary text-xs py-1 px-2.5"
                      onClick={handleSaveConceptRename}
                      disabled={busy || !editingConceptLabel.trim()}
                    >
                      {busy ? "Guardando…" : "Guardar"}
                    </button>
                    <button
                      type="button"
                      className="btn-quiet text-xs py-1 px-2"
                      onClick={() => setEditingConceptLabel(null)}
                      disabled={busy}
                    >
                      Cancelar
                    </button>
                  </div>
                </div>
              ) : (
                <div className="mt-2 flex items-start justify-between gap-2">
                  <h4 className="text-base font-semibold text-slate-100 leading-snug">
                    {selectedConcept.label}
                  </h4>
                  <button
                    type="button"
                    className="btn-quiet p-1 text-slate-400 hover:text-slate-200"
                    onClick={() => setEditingConceptLabel(selectedConcept.label)}
                    title="Editar nombre del concepto"
                  >
                    <IconPencil />
                  </button>
                </div>
              )}

              {/* Class origin */}
              <div className="mt-3 pt-3 border-t border-slate-800/80 text-xs text-slate-400">
                <span className="text-slate-400">Creado en: </span>
                <button
                  type="button"
                  className="text-sky-400 hover:underline font-medium inline-flex items-center gap-1"
                  onClick={() => onSelect({ kind: "node", id: selectedConcept.classId })}
                >
                  {nodeNameMap.get(selectedConcept.classId) || "Clase"}
                </button>
              </div>
            </div>

            {/* Relations of this concept */}
            <div className="graph-sidebar-section">
              <div className="flex items-center justify-between mb-2">
                <span className="text-xs font-semibold text-slate-300">
                  Relaciones ({nodeLinks.length})
                </span>
                <button
                  type="button"
                  className="btn-quiet text-2xs py-0.5 px-1.5 flex items-center gap-1"
                  onClick={() => onStartConnect(selectedConcept.id)}
                  title="Conectar con otra idea"
                >
                  <IconPlus /> Conectar
                </button>
              </div>

              {nodeLinks.length === 0 ? (
                <p className="text-xs text-slate-400 italic bg-paper-deep/60 p-2.5 rounded-lg border border-slate-800">
                  Este concepto aún no está conectado. Arrastra desde sus puntos azules como en Figma para relacionarlo.
                </p>
              ) : (
                <ul className="space-y-1.5">
                  {nodeLinks.map((l) => {
                    const isOutgoing = l.fromId === selectedConcept.id;
                    const otherId = isOutgoing ? l.toId : l.fromId;
                    const otherName = nodeNameMap.get(otherId) || otherId;

                    return (
                      <li
                        key={l.id}
                        className="flex items-center justify-between gap-2 p-2 rounded-lg bg-paper-deep/80 border border-slate-800/90 text-xs hover:border-slate-700 transition-colors"
                      >
                        <button
                          type="button"
                          className="flex-1 text-left min-w-0"
                          onClick={() => onSelect({ kind: "link", id: l.id })}
                        >
                          <span className="text-2xs text-amber-300 font-mono block">
                            {isOutgoing ? `→ ${l.label}` : `← ${l.label}`}
                          </span>
                          <span className="text-slate-200 font-medium truncate block">
                            {otherName}
                          </span>
                        </button>
                        <button
                          type="button"
                          className="text-slate-400 hover:text-red-400 p-1 rounded"
                          onClick={() => onRemoveLink(l.id)}
                          title="Eliminar esta relación"
                        >
                          <IconTrash />
                        </button>
                      </li>
                    );
                  })}
                </ul>
              )}
            </div>

            {/* Actions / Danger Zone */}
            <div className="pt-3 border-t border-slate-800 space-y-2">
              <button
                type="button"
                className="btn-primary w-full text-xs py-2 flex items-center justify-center gap-1.5"
                onClick={() => onStartConnect(selectedConcept.id)}
              >
                <IconLink /> Conectar con otra idea
              </button>

              {confirmDeleteId === selectedConcept.id ? (
                <div className="p-2.5 rounded-lg bg-red-950/40 border border-red-900/60 space-y-2 text-xs">
                  <p className="text-red-200">
                    ¿Eliminar este concepto y todas sus relaciones?
                  </p>
                  <div className="flex gap-2">
                    <button
                      type="button"
                      className="btn-danger flex-1 text-xs py-1"
                      onClick={handleDeleteConcept}
                      disabled={busy}
                    >
                      {busy ? "Eliminando…" : "Sí, eliminar"}
                    </button>
                    <button
                      type="button"
                      className="btn-quiet flex-1 text-xs py-1"
                      onClick={() => setConfirmDeleteId(null)}
                      disabled={busy}
                    >
                      Cancelar
                    </button>
                  </div>
                </div>
              ) : (
                <button
                  type="button"
                  className="btn-quiet w-full text-xs text-red-400 hover:text-red-300 py-1.5 flex items-center justify-center gap-1.5"
                  onClick={() => setConfirmDeleteId(selectedConcept.id)}
                >
                  <IconTrash /> Eliminar concepto
                </button>
              )}
            </div>
          </div>
        )}

        {/* ===================== CLASS SELECTED ===================== */}
        {selectedClass && (
          <div className="space-y-4">
            <div className="graph-sidebar-card">
              <span className="badge is-blue">Clase del curso</span>
              <h4 className="mt-2 text-base font-semibold text-slate-100 leading-snug">
                {classLabel(selectedClass.title)}
              </h4>
              {selectedClass.objective && (
                <p className="mt-2 text-xs text-slate-400 leading-relaxed">
                  {selectedClass.objective}
                </p>
              )}
            </div>

            {/* Concepts under this class */}
            <div className="graph-sidebar-section">
              <div className="flex items-center justify-between mb-2">
                <span className="text-xs font-semibold text-slate-300">
                  Conceptos de esta clase
                </span>
                <button
                  type="button"
                  className="btn-quiet text-2xs py-0.5 px-1.5 flex items-center gap-1"
                  onClick={() => onNewConcept(selectedClass.id)}
                  title="Añadir concepto a esta clase"
                >
                  <IconPlus /> Añadir
                </button>
              </div>

              {concepts.filter((c) => c.classId === selectedClass.id).length === 0 ? (
                <p className="text-xs text-slate-400 italic bg-paper-deep/60 p-2.5 rounded-lg border border-slate-800">
                  Aún no has registrado conceptos en esta clase.
                </p>
              ) : (
                <ul className="space-y-1.5">
                  {concepts
                    .filter((c) => c.classId === selectedClass.id)
                    .map((c) => (
                      <li
                        key={c.id}
                        className="flex items-center justify-between gap-2 p-2 rounded-lg bg-paper-deep/80 border border-slate-800/90 text-xs hover:border-slate-700 cursor-pointer transition-colors"
                        onClick={() => onSelect({ kind: "node", id: c.id })}
                      >
                        <span className="text-slate-200 font-medium truncate">
                          {c.label}
                        </span>
                        <span className="text-2xs text-amber-400 font-mono">
                          {links.filter((l) => l.fromId === c.id || l.toId === c.id).length} rel
                        </span>
                      </li>
                    ))}
                </ul>
              )}
            </div>

            {/* Actions */}
            <div className="pt-3 border-t border-slate-800 space-y-2">
              <button
                type="button"
                className="btn-primary w-full text-xs py-2 flex items-center justify-center gap-1.5"
                onClick={() => onNewConcept(selectedClass.id)}
              >
                <IconPlus /> Añadir concepto aquí
              </button>
              <button
                type="button"
                className="btn-quiet w-full text-xs py-2 flex items-center justify-center gap-1.5"
                onClick={() => onNavigateToClass(selectedClass.id)}
              >
                <IconSidebar /> Ir a la clase en el cuaderno
              </button>
            </div>
          </div>
        )}

        {/* ===================== LINK SELECTED ===================== */}
        {selectedLink && (
          <div className="space-y-4">
            <div className="graph-sidebar-card">
              <span className="badge is-amber">Relación entre ideas</span>

              {/* Source & Target Nodes */}
              <div className="mt-3 space-y-2">
                <div className="p-2 rounded bg-paper-deep border border-slate-800 text-xs">
                  <span className="text-2xs text-slate-400 block uppercase tracking-wider">
                    Desde (Origen)
                  </span>
                  <button
                    type="button"
                    className="font-medium text-slate-100 hover:text-sky-300 truncate block text-left w-full"
                    onClick={() => onSelect({ kind: "node", id: selectedLink.fromId })}
                  >
                    {nodeNameMap.get(selectedLink.fromId) || selectedLink.fromId}
                  </button>
                </div>

                <div className="p-2 rounded bg-paper-deep border border-slate-800 text-xs">
                  <span className="text-2xs text-slate-400 block uppercase tracking-wider">
                    Hacia (Destino)
                  </span>
                  <button
                    type="button"
                    className="font-medium text-slate-100 hover:text-sky-300 truncate block text-left w-full"
                    onClick={() => onSelect({ kind: "node", id: selectedLink.toId })}
                  >
                    {nodeNameMap.get(selectedLink.toId) || selectedLink.toId}
                  </button>
                </div>
              </div>

              {/* Relation Label Editor */}
              <div className="mt-3 pt-3 border-t border-slate-800">
                <label className="block text-2xs text-slate-400 uppercase tracking-wider mb-1.5">
                  Significado de la unión
                </label>
                <input
                  type="text"
                  className="w-full text-xs p-2 rounded bg-input text-slate-100 border border-slate-700 font-mono"
                  value={editingLinkLabel ?? selectedLink.label}
                  placeholder="Ej. causa, deriva de, contrasta con…"
                  onChange={(e) => setEditingLinkLabel(e.target.value)}
                />

                {/* Suggested Chips */}
                <div className="flex flex-wrap gap-1 mt-2">
                  {RELATION_HINTS.map((h) => (
                    <button
                      key={h}
                      type="button"
                      className={`chip text-2xs ${
                        (editingLinkLabel ?? selectedLink.label) === h ? "is-active" : ""
                      }`}
                      onClick={() => setEditingLinkLabel(h)}
                    >
                      {h}
                    </button>
                  ))}
                </div>

                {editingLinkLabel !== null && editingLinkLabel !== selectedLink.label && (
                  <div className="flex items-center gap-2 mt-3">
                    <button
                      type="button"
                      className="btn-primary text-xs py-1 px-3"
                      onClick={handleSaveLinkEdit}
                      disabled={busy || !editingLinkLabel.trim()}
                    >
                      {busy ? "Guardando…" : "Guardar cambios"}
                    </button>
                    <button
                      type="button"
                      className="btn-quiet text-xs py-1 px-2"
                      onClick={() => setEditingLinkLabel(null)}
                      disabled={busy}
                    >
                      Cancelar
                    </button>
                  </div>
                )}
              </div>
            </div>

            {/* Delete Relation */}
            <div className="pt-2 border-t border-slate-800">
              {confirmDeleteId === selectedLink.id ? (
                <div className="p-2.5 rounded-lg bg-red-950/40 border border-red-900/60 space-y-2 text-xs">
                  <p className="text-red-200">¿Borrar esta relación?</p>
                  <div className="flex gap-2">
                    <button
                      type="button"
                      className="btn-danger flex-1 text-xs py-1"
                      onClick={handleDeleteLink}
                      disabled={busy}
                    >
                      {busy ? "Borrando…" : "Sí, borrar"}
                    </button>
                    <button
                      type="button"
                      className="btn-quiet flex-1 text-xs py-1"
                      onClick={() => setConfirmDeleteId(null)}
                      disabled={busy}
                    >
                      Cancelar
                    </button>
                  </div>
                </div>
              ) : (
                <button
                  type="button"
                  className="btn-quiet w-full text-xs text-red-400 hover:text-red-300 py-1.5 flex items-center justify-center gap-1.5"
                  onClick={() => setConfirmDeleteId(selectedLink.id)}
                >
                  <IconTrash /> Borrar relación
                </button>
              )}
            </div>
          </div>
        )}

        {/* ===================== OVERVIEW & SEARCH (NOTHING SELECTED) ===================== */}
        {!selectedConcept && !selectedClass && !selectedLink && (
          <div className="space-y-4">
            {/* Search Input */}
            <div className="relative">
              <IconSearch className="absolute left-2.5 top-2.5 text-slate-400 pointer-events-none" />
              <input
                type="text"
                className="w-full text-xs pl-8 pr-3 py-2 rounded-lg bg-input text-slate-100 border border-slate-700 placeholder:text-slate-500"
                placeholder="Buscar conceptos o clases…"
                value={searchQuery}
                onChange={(e) => onSearchChange(e.target.value)}
              />
              {searchQuery && (
                <button
                  type="button"
                  className="absolute right-2 top-2 text-slate-400 hover:text-slate-200"
                  onClick={() => onSearchChange("")}
                >
                  <IconClose />
                </button>
              )}
            </div>

            {/* Metrics Tiles */}
            <div className="grid grid-cols-3 gap-2">
              <div className="p-2.5 rounded-lg bg-paper-deep border border-slate-800 text-center">
                <span className="block text-lg font-bold text-sky-400">{classes.length}</span>
                <span className="text-2xs text-slate-400">Clases</span>
              </div>
              <div className="p-2.5 rounded-lg bg-paper-deep border border-slate-800 text-center">
                <span className="block text-lg font-bold text-amber-400">{concepts.length}</span>
                <span className="text-2xs text-slate-400">Conceptos</span>
              </div>
              <div className="p-2.5 rounded-lg bg-paper-deep border border-slate-800 text-center">
                <span className="block text-lg font-bold text-emerald-400">{links.length}</span>
                <span className="text-2xs text-slate-400">Relaciones</span>
              </div>
            </div>

            <button
              type="button"
              className="btn-primary w-full text-xs py-2 flex items-center justify-center gap-1.5"
              onClick={() => onNewConcept()}
            >
              <IconPlus /> Nuevo concepto
            </button>

            {/* Classes & Concepts Tree List */}
            <div className="graph-sidebar-section">
              <span className="text-2xs font-semibold text-slate-400 uppercase tracking-wider mb-2 block">
                Ideas organizadas por clase
              </span>

              <div className="space-y-2 max-h-[340px] overflow-y-auto pr-1">
                {classes.map((cls) => {
                  const classConcepts = concepts.filter((c) => c.classId === cls.id);
                  const matchesSearch =
                    !searchQuery ||
                    classLabel(cls.title).toLowerCase().includes(searchQuery.toLowerCase()) ||
                    classConcepts.some((c) => c.label.toLowerCase().includes(searchQuery.toLowerCase()));

                  if (!matchesSearch) return null;

                  return (
                    <div
                      key={cls.id}
                      className="rounded-lg bg-paper-deep/60 border border-slate-800/80 p-2 space-y-1.5"
                    >
                      <button
                        type="button"
                        className="text-xs font-semibold text-sky-300 hover:text-sky-200 text-left w-full truncate flex items-center justify-between"
                        onClick={() => onSelect({ kind: "node", id: cls.id })}
                      >
                        <span className="truncate">{classLabel(cls.title)}</span>
                        <span className="text-2xs font-normal text-slate-400 shrink-0">
                          {classConcepts.length} ideas
                        </span>
                      </button>

                      {classConcepts.length > 0 && (
                        <div className="pl-2 space-y-1 border-l border-slate-800">
                          {classConcepts.map((c) => (
                            <button
                              key={c.id}
                              type="button"
                              className="text-xs text-slate-300 hover:text-amber-300 text-left w-full truncate block py-0.5"
                              onClick={() => onSelect({ kind: "node", id: c.id })}
                            >
                              • {truncate(c.label, 26)}
                            </button>
                          ))}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>

            {/* Quick Tips / Shortcuts */}
            <div className="p-3 rounded-lg bg-paper-deep/40 border border-slate-800/60 text-2xs text-slate-400 space-y-1.5">
              <span className="font-semibold text-slate-300 block">Atajos del lienzo:</span>
              <p>• <strong>Conectar como Figma:</strong> Arrastra desde cualquiera de los 4 puntos de un nodo hacia otro.</p>
              <p>• <strong>Menú contextual:</strong> Clic derecho en el lienzo, un concepto o una línea.</p>
              <p>• <strong>Mover:</strong> Arrastra el fondo para desplazarte, rueda para zoom.</p>
            </div>
          </div>
        )}
      </div>
    </aside>
  );
}
