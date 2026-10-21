import { useEffect, useMemo, useState } from "react";
import { useNotebookNav } from "../../stores/notebook";
import { useGraph } from "../../stores/graph";
import { useRoadmap } from "../../stores/roadmap";
import { useUi } from "../../stores/ui";
import { ConceptForm, ConfirmButton, GraphCanvas, LinkForm, RelationList } from "./GraphParts";
import type { Selection } from "./GraphParts";
import { classLabel, layoutGraph, nodeOptions } from "./graph";

type Panel =
  | { kind: "concept"; classId: string }
  | { kind: "link"; fromId?: string }
  | null;

/** The student's own map of ideas. They write the concepts and draw (and
 *  name) every relation themselves — putting into words WHY two ideas connect
 *  is the point, so nothing here is suggested or generated. */
export function GraphView() {
  const sessionCourse = useRoadmap((s) => s.session?.imported_course_id ?? null);
  const navCourse = useNotebookNav((s) => s.courseId);
  const courseId = sessionCourse ?? navCourse;
  const setView = useUi((s) => s.setView);
  const { classes, graph, loading, error, load, clearError, addConcept, renameConcept, removeConcept, addLink, editLink, removeLink } = useGraph();

  const [selection, setSelection] = useState<Selection>(null);
  const [panel, setPanel] = useState<Panel>(null);

  useEffect(() => {
    if (courseId) void load(courseId);
  }, [courseId, load]);

  const layout = useMemo(() => layoutGraph(classes, graph.concepts, graph.links), [classes, graph]);
  const options = useMemo(() => nodeOptions(classes, graph.concepts), [classes, graph.concepts]);

  const selectedLink = selection?.kind === "link" ? graph.links.find((l) => l.id === selection.id) : undefined;
  const selectedNode = selection?.kind === "node" ? layout.nodes.find((n) => n.id === selection.id) : undefined;
  const selectedConcept = selectedNode?.kind === "concept" ? graph.concepts.find((c) => c.id === selectedNode.id) : undefined;
  const firstClass = classes[0]?.id ?? "";

  const select = (s: Selection) => {
    setSelection(s);
    setPanel(null);
    clearError();
  };

  if (!courseId) {
    return (
      <div className="view graph-view">
        <h2>Tu grafo de ideas</h2>
        <p className="muted">Todavía no hay clases. Cuando tu sesión tenga un curso, aquí podrás conectar lo que aprendes con tus propias palabras.</p>
      </div>
    );
  }

  return (
    <div className="view graph-view">
      <h2>Tu grafo de ideas</h2>
      <p className="muted">
        Conecta lo que aprendes con tus propias palabras. Las relaciones las creas tú: explicar <em>por qué</em> dos ideas se unen es lo que convierte la
        información en conocimiento.
      </p>

      <div className="row">
        <button className="btn-primary" disabled={!firstClass} onClick={() => { setSelection(null); setPanel({ kind: "concept", classId: selectedNode?.classId ?? firstClass }); }}>
          ＋ Nuevo concepto
        </button>
        <button className="btn-primary" disabled={options.length < 2} onClick={() => { setSelection(null); setPanel({ kind: "link", fromId: selectedNode?.id }); }}>
          ＋ Nueva relación
        </button>
      </div>

      {error && (
        <div className="alert error" onClick={clearError}>
          {error}
        </div>
      )}
      {loading && <p className="muted">Cargando…</p>}

      {panel?.kind === "concept" && (
        <ConceptForm classes={classes} defaultClassId={panel.classId} submitLabel="Añadir concepto" onSubmit={async (classId, label) => (await addConcept(classId, label)) !== null} onCancel={() => setPanel(null)} />
      )}
      {panel?.kind === "link" && (
        <LinkForm options={options} initialFrom={panel.fromId} submitLabel="Crear relación" onSubmit={addLink} onCancel={() => setPanel(null)} />
      )}

      {!loading && layout.nodes.length === 0 ? (
        <p className="muted">Este curso aún no tiene clases para conectar.</p>
      ) : (
        <GraphCanvas layout={layout} selection={selection} onSelect={select} />
      )}
      {!loading && graph.links.length === 0 && layout.nodes.length > 0 && (
        <p className="hint">Aún no has creado relaciones. Empieza con «＋ Nueva relación», o al terminar una clase usa el paso «Conecta lo aprendido».</p>
      )}

      {selectedLink && (
        <section className="card graph-inspector" aria-label="Editar relación">
          <h3>Relación</h3>
          <LinkForm
            key={selectedLink.id}
            options={options}
            initialFrom={selectedLink.fromId}
            initialTo={selectedLink.toId}
            initialLabel={selectedLink.label}
            submitLabel="Guardar cambios"
            onSubmit={(from, to, label) => editLink(selectedLink.id, from, to, label)}
            onCancel={() => setSelection(null)}
          />
          <ConfirmButton
            label="Borrar relación"
            warning="¿Borrar esta relación?"
            onConfirm={async () => {
              if (await removeLink(selectedLink.id)) setSelection(null);
            }}
          />
        </section>
      )}

      {selectedNode?.kind === "class" && (
        <section className="card graph-inspector" aria-label="Clase seleccionada">
          <h3>{classLabel(classes.find((c) => c.id === selectedNode.id)?.title ?? selectedNode.label)}</h3>
          <div className="row">
            <button className="btn-quiet" onClick={() => setPanel({ kind: "concept", classId: selectedNode.id })}>
              ＋ Añadir un concepto aquí
            </button>
            <button className="btn-quiet" onClick={() => setPanel({ kind: "link", fromId: selectedNode.id })}>
              Relacionar desde aquí
            </button>
          </div>
        </section>
      )}

      {selectedConcept && (
        <section className="card graph-inspector" aria-label="Editar concepto">
          <h3>Concepto</h3>
          <ConceptForm
            key={selectedConcept.id}
            classes={classes}
            defaultClassId={selectedConcept.classId}
            initialLabel={selectedConcept.label}
            submitLabel="Guardar cambios"
            onSubmit={(classId, label) => renameConcept(selectedConcept.id, classId, label)}
            onCancel={() => setSelection(null)}
          />
          <div className="row">
            <button className="btn-quiet" onClick={() => setPanel({ kind: "link", fromId: selectedConcept.id })}>
              Relacionar desde aquí
            </button>
            <ConfirmButton
              label="Borrar concepto"
              warning="Se borrarán también sus relaciones."
              onConfirm={async () => {
                if (await removeConcept(selectedConcept.id)) setSelection(null);
              }}
            />
          </div>
        </section>
      )}

      <RelationList links={graph.links} options={options} selection={selection} onSelect={select} />

      <div className="row">
        <button className="btn-quiet" onClick={() => setView("notebook")}>
          Volver a las clases
        </button>
      </div>
    </div>
  );
}
