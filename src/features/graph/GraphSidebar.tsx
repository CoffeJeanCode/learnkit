import { useState } from "react";
import type { ClassRecord, Concept, ConceptLink } from "../../lib/schemas";
import { RELATION_CONNECTIVES, RELATION_HINTS, classLabel, formatRelationLabel, truncate } from "./graph";
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
          <IconSidebar className="text-chalk-soft shrink-0" />
          <h3 className="text-sm font-semibold text-chalk truncate">
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
          <div className="flex flex-col gap-md">
            <div className="graph-sidebar-card">
              <span className="badge is-amber">Concepto propio</span>
              {editingConceptLabel !== null ? (
                <div className="mt-2 flex flex-col gap-xs">
                  <input
                    type="text"
                    className="w-full text-xs p-2 rounded bg-input text-chalk border border-soft"
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
                  <h4 className="text-base font-semibold text-chalk leading-snug">
                    {selectedConcept.label}
                  </h4>
                  <button
                    type="button"
                    className="btn-quiet p-1 text-chalk-soft hover:text-chalk"
                    onClick={() => setEditingConceptLabel(selectedConcept.label)}
                    title="Editar nombre del concepto"
                  >
                    <IconPencil />
                  </button>
                </div>
              )}

              {/* Class origin */}
              <div className="mt-3 pt-3 border-t border-soft text-xs text-chalk-soft">
                <span className="text-chalk-soft">Creado en: </span>
                <button
                  type="button"
                  className="text-blue hover:underline font-medium inline-flex items-center gap-1"
                  onClick={() => onSelect({ kind: "node", id: selectedConcept.classId })}
                >
                  {nodeNameMap.get(selectedConcept.classId) || "Clase"}
                </button>
              </div>
            </div>

            {/* Relations of this concept */}
            <div className="graph-sidebar-section">
              <div className="flex items-center justify-between mb-2">
                <span className="text-xs font-semibold text-chalk-soft">
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
                <p className="text-xs text-chalk-soft italic bg-paper-deep p-2.5 rounded-md border border-soft">
                  Este concepto aún no está conectado. Arrastra desde sus puntos azules como en Figma para relacionarlo.
                </p>
              ) : (
                <ul className="flex flex-col gap-2xs">
                  {nodeLinks.map((l) => {
                    const isOutgoing = l.fromId === selectedConcept.id;
                    const otherId = isOutgoing ? l.toId : l.fromId;
                    const otherName = nodeNameMap.get(otherId) || otherId;

                    return (
                      <li
                        key={l.id}
                        className="flex items-center justify-between gap-2 p-2 rounded-md bg-paper-deep border border-soft text-xs hover:border-soft transition-colors"
                      >
                        <button
                          type="button"
                          className="flex-1 text-left min-w-0"
                          onClick={() => onSelect({ kind: "link", id: l.id })}
                        >
                          <span className="text-2xs text-gold font-mono block">
                            {isOutgoing ? `→ ${formatRelationLabel(l.label)}` : `← ${formatRelationLabel(l.label)}`}
                          </span>
                          <span className="text-chalk font-medium truncate block">
                            {otherName}
                          </span>
                        </button>
                        <button
                          type="button"
                          className="text-chalk-soft hover:text-red p-1 rounded"
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
            <div className="pt-3 border-t border-soft flex flex-col gap-xs">
              <button
                type="button"
                className="btn-primary w-full text-xs py-2 flex items-center justify-center gap-1.5"
                onClick={() => onStartConnect(selectedConcept.id)}
              >
                <IconLink /> Conectar con otra idea
              </button>

              {confirmDeleteId === selectedConcept.id ? (
                <div className="p-2.5 rounded-md p-xs rounded-sm flex flex-col gap-xs text-xs">
                  <p className="text-red font-medium">
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
                  className="btn-quiet w-full text-xs text-red py-1.5 flex items-center justify-center gap-1.5"
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
          <div className="flex flex-col gap-md">
            <div className="graph-sidebar-card">
              <span className="badge is-blue">Clase del curso</span>
              <h4 className="mt-2 text-base font-semibold text-chalk leading-snug">
                {classLabel(selectedClass.title)}
              </h4>
              {selectedClass.objective && (
                <p className="mt-2 text-xs text-chalk-soft leading-relaxed">
                  {selectedClass.objective}
                </p>
              )}
            </div>

            {/* Concepts under this class */}
            <div className="graph-sidebar-section">
              <div className="flex items-center justify-between mb-2">
                <span className="text-xs font-semibold text-chalk-soft">
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
                <p className="text-xs text-chalk-soft italic bg-paper-deep p-2.5 rounded-md border border-soft">
                  Aún no has registrado conceptos en esta clase.
                </p>
              ) : (
                <ul className="flex flex-col gap-2xs">
                  {concepts
                    .filter((c) => c.classId === selectedClass.id)
                    .map((c) => (
                      <li
                        key={c.id}
                        className="flex items-center justify-between gap-2 p-2 rounded-md bg-paper-deep border border-soft text-xs hover:border-soft cursor-pointer transition-colors"
                        onClick={() => onSelect({ kind: "node", id: c.id })}
                      >
                        <span className="text-chalk font-medium truncate">
                          {c.label}
                        </span>
                        <span className="text-2xs text-gold font-mono">
                          {links.filter((l) => l.fromId === c.id || l.toId === c.id).length} rel
                        </span>
                      </li>
                    ))}
                </ul>
              )}
            </div>

            {/* Actions */}
            <div className="pt-3 border-t border-soft flex flex-col gap-xs">
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
          <div className="flex flex-col gap-md">
            <div className="graph-sidebar-card">
              <span className="badge is-amber">Relación entre ideas</span>

              {/* Source & Target Nodes */}
              <div className="mt-3 flex flex-col gap-xs">
                <div className="p-2 rounded bg-paper-deep border border-soft text-xs">
                  <span className="text-2xs text-chalk-soft block uppercase tracking-wider">
                    Desde (Origen)
                  </span>
                  <button
                    type="button"
                    className="font-medium text-chalk hover:text-blue truncate block text-left w-full"
                    onClick={() => onSelect({ kind: "node", id: selectedLink.fromId })}
                  >
                    {nodeNameMap.get(selectedLink.fromId) || selectedLink.fromId}
                  </button>
                </div>

                <div className="p-2 rounded bg-paper-deep border border-soft text-xs">
                  <span className="text-2xs text-chalk-soft block uppercase tracking-wider">
                    Hacia (Destino)
                  </span>
                  <button
                    type="button"
                    className="font-medium text-chalk hover:text-blue truncate block text-left w-full"
                    onClick={() => onSelect({ kind: "node", id: selectedLink.toId })}
                  >
                    {nodeNameMap.get(selectedLink.toId) || selectedLink.toId}
                  </button>
                </div>
              </div>

              {/* Relation Label Editor */}
              <div className="mt-3 pt-3 border-t border-soft">
                <label className="block text-2xs text-chalk-soft uppercase tracking-wider mb-1.5">
                  Significado de la unión
                </label>
                <input
                  type="text"
                  className="w-full text-xs p-2 rounded bg-input text-chalk border border-soft font-mono"
                  value={editingLinkLabel ?? selectedLink.label}
                  placeholder="Ej. causa, deriva de, contrasta con…"
                  onChange={(e) => setEditingLinkLabel(e.target.value)}
                />

                {/* Cognitive Connectives (LearnHarness-style) */}
                <div className="grid grid-cols-2 gap-1.5 mt-2">
                  {RELATION_CONNECTIVES.map((c) => (
                    <button
                      key={c.value}
                      type="button"
                      className={`chip text-2xs justify-start gap-1 py-1 px-1.5 ${
                        (editingLinkLabel ?? selectedLink.label) === c.value ? "is-active" : ""
                      }`}
                      onClick={() => setEditingLinkLabel(c.value)}
                      title={c.description}
                    >
                      <span className="font-mono text-gold font-bold">{c.glyph}</span>
                      <span className="truncate">{c.label}</span>
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
            <div className="pt-2 border-t border-soft">
              {confirmDeleteId === selectedLink.id ? (
                <div className="p-2.5 rounded-md p-xs rounded-sm flex flex-col gap-xs text-xs">
                  <p className="text-red font-medium">¿Borrar esta relación?</p>
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
                  className="btn-quiet w-full text-xs text-red py-1.5 flex items-center justify-center gap-1.5"
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
          <div className="flex flex-col gap-md">
            {/* Search Input */}
            <div className="relative">
              <IconSearch className="absolute left-2.5 top-2.5 text-chalk-soft pointer-events-none" />
              <input
                type="text"
                className="w-full text-xs pl-8 pr-3 py-2 rounded-md bg-input text-chalk border border-soft placeholder:text-chalk-faint"
                placeholder="Buscar conceptos o clases…"
                value={searchQuery}
                onChange={(e) => onSearchChange(e.target.value)}
              />
              {searchQuery && (
                <button
                  type="button"
                  className="absolute right-2 top-2 text-chalk-soft hover:text-chalk"
                  onClick={() => onSearchChange("")}
                >
                  <IconClose />
                </button>
              )}
            </div>

            {/* Metrics Tiles */}
            <div className="grid grid-cols-3 gap-2">
              <div className="p-2.5 rounded-md bg-paper-deep border border-soft text-center">
                <span className="block text-lg font-bold text-blue">{classes.length}</span>
                <span className="text-2xs text-chalk-soft">Clases</span>
              </div>
              <div className="p-2.5 rounded-md bg-paper-deep border border-soft text-center">
                <span className="block text-lg font-bold text-gold">{concepts.length}</span>
                <span className="text-2xs text-chalk-soft">Conceptos</span>
              </div>
              <div className="p-2.5 rounded-md bg-paper-deep border border-soft text-center">
                <span className="block text-lg font-bold text-green">{links.length}</span>
                <span className="text-2xs text-chalk-soft">Relaciones</span>
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
              <span className="text-2xs font-semibold text-chalk-soft uppercase tracking-wider mb-2 block">
                Ideas organizadas por clase
              </span>

              <div className="flex flex-col gap-xs max-h-[340px] overflow-y-auto pr-1">
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
                      className="rounded-md bg-paper-deep border border-soft p-2 flex flex-col gap-2xs"
                    >
                      <button
                        type="button"
                        className="text-xs font-semibold text-blue hover:text-blue text-left w-full truncate flex items-center justify-between"
                        onClick={() => onSelect({ kind: "node", id: cls.id })}
                      >
                        <span className="truncate">{classLabel(cls.title)}</span>
                        <span className="text-2xs font-normal text-chalk-soft shrink-0">
                          {classConcepts.length} ideas
                        </span>
                      </button>

                      {classConcepts.length > 0 && (
                        <div className="pl-2 flex flex-col gap-3xs border-l border-soft">
                          {classConcepts.map((c) => (
                            <button
                              key={c.id}
                              type="button"
                              className="text-xs text-chalk-soft hover:text-gold text-left w-full truncate block py-0.5"
                              onClick={() => onSelect({ kind: "node", id: c.id })}
                            >
                              • {truncate(c.label, 42)}
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
            <div className="p-3 rounded-md bg-paper-deep border border-soft text-2xs text-chalk-soft flex flex-col gap-2xs">
              <span className="font-semibold text-chalk-soft block">Atajos del lienzo:</span>
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
