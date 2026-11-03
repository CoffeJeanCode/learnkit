import { useEffect, useRef, useState } from "react";
import * as d3 from "d3";
import type { ClassRecord, Concept, ConceptLink } from "../../lib/schemas";
import { RELATION_HINTS, classLabel, truncate } from "./graph";
import type { Selection } from "./GraphParts";

export interface D3Node extends d3.SimulationNodeDatum {
  id: string;
  kind: "class" | "concept";
  label: string;
  classId: string;
  milestoneId?: string;
  w: number;
  h: number;
}

export interface D3Link extends d3.SimulationLinkDatum<D3Node> {
  id: string;
  fromId: string;
  toId: string;
  label: string;
}

export function GraphCanvasD3({
  classes,
  concepts,
  links,
  selection,
  onSelect,
  onCreateLink,
}: {
  classes: ClassRecord[];
  concepts: Concept[];
  links: ConceptLink[];
  selection: Selection;
  onSelect: (s: Selection) => void;
  onCreateLink: (fromId: string, toId: string, label: string) => Promise<boolean>;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);

  // Connecting rubberband state
  const [connectingFrom, setConnectingFrom] = useState<string | null>(null);
  const [dragLine, setDragLine] = useState<{ x1: number; y1: number; x2: number; y2: number } | null>(null);

  // Quick relation modal when connection is dropped onto target
  const [pendingRelation, setPendingRelation] = useState<{ fromId: string; toId: string } | null>(null);
  const [relationLabel, setRelationLabel] = useState("");
  const [creating, setCreating] = useState(false);

  // D3 zoom transform reference for coord mapping
  const zoomBehaviorRef = useRef<d3.ZoomBehavior<SVGSVGElement, unknown> | null>(null);
  const currentTransformRef = useRef<d3.ZoomTransform>(d3.zoomIdentity);

  // Simulation ref
  const simRef = useRef<d3.Simulation<D3Node, D3Link> | null>(null);
  const nodesRef = useRef<D3Node[]>([]);
  const linksRef = useRef<D3Link[]>([]);

  // Build node and link datasets
  useEffect(() => {
    const width = containerRef.current?.clientWidth || 900;
    const height = 550;

    // 1. Build nodes
    const nodeMap = new Map<string, D3Node>();
    const d3Nodes: D3Node[] = [];

    classes.forEach((c, idx) => {
      const node: D3Node = {
        id: c.id,
        kind: "class",
        label: classLabel(c.title),
        classId: c.id,
        milestoneId: c.milestone_id,
        w: 200,
        h: 44,
        x: width * 0.2 + (idx % 3) * 220,
        y: 100 + Math.floor(idx / 3) * 140,
      };
      nodeMap.set(c.id, node);
      d3Nodes.push(node);
    });

    concepts.forEach((c) => {
      const parent = nodeMap.get(c.classId);
      const node: D3Node = {
        id: c.id,
        kind: "concept",
        label: c.label,
        classId: c.classId,
        w: 170,
        h: 36,
        x: parent ? (parent.x ?? width / 2) + (Math.random() - 0.5) * 80 : width / 2,
        y: parent ? (parent.y ?? height / 2) + 70 + (Math.random() - 0.5) * 40 : height / 2,
      };
      nodeMap.set(c.id, node);
      d3Nodes.push(node);
    });

    // 2. Build links
    const d3Links: D3Link[] = [];
    links.forEach((l) => {
      if (nodeMap.has(l.fromId) && nodeMap.has(l.toId)) {
        d3Links.push({
          id: l.id,
          source: l.fromId,
          target: l.toId,
          fromId: l.fromId,
          toId: l.toId,
          label: l.label,
        });
      }
    });

    // Implicit hierarchical links between class and its concepts
    concepts.forEach((c) => {
      if (nodeMap.has(c.classId)) {
        d3Links.push({
          id: `hier-${c.classId}-${c.id}`,
          source: c.classId,
          target: c.id,
          fromId: c.classId,
          toId: c.id,
          label: "",
        });
      }
    });

    nodesRef.current = d3Nodes;
    linksRef.current = d3Links;

    // 3. Initialize Force Simulation
    const sim = d3
      .forceSimulation<D3Node>(d3Nodes)
      .force(
        "link",
        d3
          .forceLink<D3Node, D3Link>(d3Links)
          .id((d) => d.id)
          .distance((d) => (d.label ? 160 : 70))
          .strength((d) => (d.label ? 0.6 : 0.8)),
      )
      .force("charge", d3.forceManyBody().strength(-350))
      .force("collide", d3.forceCollide().radius(60).strength(0.7))
      .force("center", d3.forceCenter(width / 2, height / 2).strength(0.05))
      .alphaDecay(0.028);

    simRef.current = sim;

    const svgEl = svgRef.current;
    if (!svgEl) return;
    const svg = d3.select(svgEl);
    const g = svg.select<SVGGElement>(".d3-viewport");

    // 4. Zoom & Pan
    const zoom = d3
      .zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.3, 3])
      .on("zoom", (event) => {
        currentTransformRef.current = event.transform;
        g.attr("transform", event.transform);
      });

    zoomBehaviorRef.current = zoom;
    (svg as unknown as d3.Selection<SVGSVGElement, unknown, null, undefined>).call(zoom);

    // 5. Simulation Tick
    sim.on("tick", () => {
      // Update link lines
      g.selectAll<SVGLineElement, D3Link>(".d3-link-line")
        .data(d3Links, (d) => d.id)
        .join("line")
        .attr("x1", (d) => (d.source as D3Node).x || 0)
        .attr("y1", (d) => (d.source as D3Node).y || 0)
        .attr("x2", (d) => (d.target as D3Node).x || 0)
        .attr("y2", (d) => (d.target as D3Node).y || 0);

      // Update link labels
      g.selectAll<SVGTextElement, D3Link>(".d3-link-label")
        .data(d3Links.filter((d) => !!d.label), (d) => d.id)
        .join("text")
        .attr("x", (d) => (((d.source as D3Node).x || 0) + ((d.target as D3Node).x || 0)) / 2)
        .attr("y", (d) => (((d.source as D3Node).y || 0) + ((d.target as D3Node).y || 0)) / 2 - 4);

      // Update node groups
      g.selectAll<SVGGElement, D3Node>(".d3-node-group")
        .data(d3Nodes, (d) => d.id)
        .join("g")
        .attr("transform", (d) => `translate(${d.x || 0}, ${d.y || 0})`);
    });

    return () => {
      sim.stop();
    };
  }, [classes, concepts, links]);

  // Connected nodes map for spotlighting
  const connectedNodeIds = new Set<string>();
  if (selection?.kind === "node") {
    connectedNodeIds.add(selection.id);
    links.forEach((l) => {
      if (l.fromId === selection.id) connectedNodeIds.add(l.toId);
      if (l.toId === selection.id) connectedNodeIds.add(l.fromId);
    });
  } else if (selection?.kind === "link") {
    const l = links.find((x) => x.id === selection.id);
    if (l) {
      connectedNodeIds.add(l.fromId);
      connectedNodeIds.add(l.toId);
    }
  }

  // Zoom control handlers
  const handleZoom = (factor: number) => {
    if (!svgRef.current || !zoomBehaviorRef.current) return;
    const svg = d3.select(svgRef.current);
    svg.transition().duration(250).call(zoomBehaviorRef.current.scaleBy as any, factor);
  };

  const handleResetZoom = () => {
    if (!svgRef.current || !zoomBehaviorRef.current) return;
    const svg = d3.select(svgRef.current);
    svg.transition().duration(300).call(zoomBehaviorRef.current.transform as any, d3.zoomIdentity);
  };

  // Node Drag handlers (D3 force drag)
  const handleNodePointerDown = (e: React.PointerEvent, node: D3Node) => {
    if (e.button !== 0 || connectingFrom) return;
    e.stopPropagation();
    const sim = simRef.current;
    if (!sim) return;

    sim.alphaTarget(0.3).restart();
    node.fx = node.x;
    node.fy = node.y;

    const startX = e.clientX;
    const startY = e.clientY;
    const origFx = node.x ?? 0;
    const origFy = node.y ?? 0;
    const transform = currentTransformRef.current;

    const onPointerMove = (ev: PointerEvent) => {
      const dx = (ev.clientX - startX) / transform.k;
      const dy = (ev.clientY - startY) / transform.k;
      node.fx = origFx + dx;
      node.fy = origFy + dy;
    };

    const onPointerUp = () => {
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
      sim.alphaTarget(0);
      node.fx = null;
      node.fy = null;
    };

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
  };

  // Connector handle drag (to connect knowledge)
  const handleConnectorStart = (e: React.PointerEvent, sourceId: string) => {
    e.stopPropagation();
    setConnectingFrom(sourceId);

    const sourceNode = nodesRef.current.find((n) => n.id === sourceId);
    if (!sourceNode) return;

    const transform = currentTransformRef.current;
    const p1 = { x: sourceNode.x || 0, y: sourceNode.y || 0 };

    const onPointerMove = (ev: PointerEvent) => {
      const rect = svgRef.current?.getBoundingClientRect();
      if (!rect) return;
      const mouseX = (ev.clientX - rect.left - transform.x) / transform.k;
      const mouseY = (ev.clientY - rect.top - transform.y) / transform.k;
      setDragLine({ x1: p1.x, y1: p1.y, x2: mouseX, y2: mouseY });
    };

    const onPointerUp = (ev: PointerEvent) => {
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
      setDragLine(null);
      setConnectingFrom(null);

      // Check if dropped over another node
      const rect = svgRef.current?.getBoundingClientRect();
      if (!rect) return;
      const mouseX = (ev.clientX - rect.left - transform.x) / transform.k;
      const mouseY = (ev.clientY - rect.top - transform.y) / transform.k;

      const target = nodesRef.current.find((n) => {
        if (n.id === sourceId) return false;
        const dx = (n.x || 0) - mouseX;
        const dy = (n.y || 0) - mouseY;
        return Math.sqrt(dx * dx + dy * dy) < 45;
      });

      if (target) {
        setPendingRelation({ fromId: sourceId, toId: target.id });
        setRelationLabel("causa");
      }
    };

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
  };

  const handleConfirmPendingRelation = async () => {
    if (!pendingRelation || !relationLabel.trim()) return;
    setCreating(true);
    try {
      await onCreateLink(pendingRelation.fromId, pendingRelation.toId, relationLabel.trim());
      setPendingRelation(null);
    } finally {
      setCreating(false);
    }
  };

  return (
    <div ref={containerRef} className="d3-graph-container relative w-full h-[550px] overflow-hidden rounded-xl bg-paper-deep border border-slate-800">
      {/* Zoom controls floating toolbar */}
      <div className="absolute top-3 right-3 z-10 flex items-center gap-1.5 p-1 rounded-lg bg-surface-deep/80 backdrop-blur border border-slate-800 text-xs">
        <button
          className="p-1.5 hover:bg-slate-700/60 rounded text-slate-300 font-bold"
          onClick={() => handleZoom(1.25)}
          title="Acercar (+)"
        >
          ＋
        </button>
        <button
          className="p-1.5 hover:bg-slate-700/60 rounded text-slate-300 font-bold"
          onClick={() => handleZoom(0.8)}
          title="Alejar (-)"
        >
          －
        </button>
        <button
          className="px-2 py-1 hover:bg-slate-700/60 rounded text-slate-300 font-medium text-2xs"
          onClick={handleResetZoom}
          title="Centrar vista (1:1)"
        >
          Centrar
        </button>
      </div>

      {/* Guide pill */}
      <div className="absolute bottom-3 left-3 z-10 px-2.5 py-1 rounded-md bg-paper/90 border border-slate-800 text-2xs text-slate-400">
        💡 <strong>Tip:</strong> Arrastra el círculo azul del nodo hacia otro para conectar ideas al vuelo.
      </div>

      <svg ref={svgRef} className="w-full h-full cursor-grab active:cursor-grabbing select-none" role="group" aria-label="Grafo de conocimiento D3">
        <defs>
          <marker id="d3-arrow" viewBox="0 0 10 10" refX="28" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
            <path d="M 0 0 L 10 5 L 0 10 z" fill="var(--blue-chalk, #7fb0d8)" opacity="0.8" />
          </marker>
        </defs>

        <g className="d3-viewport">
          {/* Link lines */}
          {linksRef.current.map((link) => {
            const isExplicit = !!link.label;
            const isSelected = selection?.kind === "link" && selection.id === link.id;
            const dimmed = connectedNodeIds.size > 0 && !connectedNodeIds.has(link.fromId) && !connectedNodeIds.has(link.toId);

            return (
              <line
                key={link.id}
                className={`d3-link-line transition-opacity duration-200 ${
                  isExplicit ? "stroke-[1.75px] stroke-sky-400/70" : "stroke-[1px] stroke-slate-700/40 stroke-dasharray-[3,3]"
                } ${isSelected ? "stroke-amber-400 stroke-[2.5px]" : ""} ${dimmed ? "opacity-15" : "opacity-85"}`}
                markerEnd={isExplicit ? "url(#d3-arrow)" : undefined}
                onClick={(e) => {
                  e.stopPropagation();
                  if (isExplicit) onSelect({ kind: "link", id: link.id });
                }}
              />
            );
          })}

          {/* Interactive rubberband drag line */}
          {dragLine && (
            <line
              x1={dragLine.x1}
              y1={dragLine.y1}
              x2={dragLine.x2}
              y2={dragLine.y2}
              stroke="var(--gold-chalk, #d9b168)"
              strokeWidth="2.5"
              strokeDasharray="4 4"
              className="pointer-events-none"
            />
          )}

          {/* Link labels */}
          {linksRef.current
            .filter((l) => !!l.label)
            .map((link) => {
              const isSelected = selection?.kind === "link" && selection.id === link.id;
              const dimmed = connectedNodeIds.size > 0 && !connectedNodeIds.has(link.fromId) && !connectedNodeIds.has(link.toId);

              return (
                <text
                  key={`lbl-${link.id}`}
                  className={`d3-link-label text-[11px] font-mono cursor-pointer transition-opacity duration-200 ${
                    isSelected ? "fill-amber-300 font-bold" : "fill-slate-400 hover:fill-slate-200"
                  } ${dimmed ? "opacity-15" : "opacity-90"}`}
                  textAnchor="middle"
                  onClick={(e) => {
                    e.stopPropagation();
                    onSelect({ kind: "link", id: link.id });
                  }}
                >
                  {truncate(link.label, 20)}
                </text>
              );
            })}

          {/* Nodes */}
          {nodesRef.current.map((node) => {
            const isSelected = selection?.kind === "node" && selection.id === node.id;
            const isConnected = connectedNodeIds.has(node.id);
            const dimmed = connectedNodeIds.size > 0 && !isConnected;
            const isClass = node.kind === "class";

            return (
              <g
                key={node.id}
                className={`d3-node-group cursor-pointer transition-opacity duration-200 ${dimmed ? "opacity-25" : "opacity-100"}`}
                onPointerDown={(e) => handleNodePointerDown(e, node)}
                onClick={(e) => {
                  e.stopPropagation();
                  onSelect({ kind: "node", id: node.id });
                }}
              >
                {/* Node Box */}
                <rect
                  x={-node.w / 2}
                  y={-node.h / 2}
                  width={node.w}
                  height={node.h}
                  rx={12}
                  className={`transition-colors duration-150 ${
                    isClass
                      ? isSelected
                        ? "fill-sky-950 stroke-sky-400 stroke-2"
                        : "fill-slate-900/90 stroke-slate-700/80 stroke-1 hover:stroke-slate-500"
                      : isSelected
                        ? "fill-amber-950 stroke-amber-400 stroke-2"
                        : "fill-card stroke-slate-800 stroke-1 hover:stroke-slate-600"
                  }`}
                />

                {/* Node Label */}
                <text
                  x={-node.w / 2 + 14}
                  y={4}
                  className={`text-xs select-none pointer-events-none font-medium ${
                    isClass ? "fill-sky-200 font-semibold" : "fill-slate-200"
                  }`}
                >
                  {truncate(node.label, isClass ? 22 : 18)}
                </text>

                {/* Connection Handle (Anchor pill on right side) */}
                <circle
                  cx={node.w / 2 - 2}
                  cy={0}
                  r={7}
                  className="fill-sky-500/80 hover:fill-amber-400 transition-colors cursor-crosshair stroke-paper stroke-2"
                  onPointerDown={(e) => handleConnectorStart(e, node.id)}
                >
                  <title>Arrastra para conectar con otro nodo</title>
                </circle>
              </g>
            );
          })}
        </g>
      </svg>

      {/* Floating relation modal when connection is dropped */}
      {pendingRelation && (
        <div className="absolute inset-0 z-20 flex items-center justify-center bg-black/50 backdrop-blur-xs p-4">
          <div className="w-full max-w-sm p-4 rounded-xl bg-surface-deep border border-slate-700 shadow-xl space-y-3">
            <h4 className="text-sm font-semibold text-slate-100">
              Conectar conocimiento
            </h4>
            <p className="text-xs text-slate-400">
              ¿Cómo se relacionan estas dos ideas?
            </p>
            <input
              autoFocus
              className="w-full p-2 text-xs rounded bg-input text-slate-200 border border-slate-700"
              value={relationLabel}
              placeholder="Ej. causa, es parte de, contrasta con…"
              onChange={(e) => setRelationLabel(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleConfirmPendingRelation()}
            />
            <div className="flex flex-wrap gap-1">
              {RELATION_HINTS.map((h) => (
                <button
                  key={h}
                  type="button"
                  className={`chip text-2xs ${relationLabel === h ? "is-active" : ""}`}
                  onClick={() => setRelationLabel(h)}
                >
                  {h}
                </button>
              ))}
            </div>
            <div className="flex items-center justify-end gap-2 pt-2 border-t border-slate-800">
              <button
                type="button"
                className="btn-quiet text-xs"
                onClick={() => setPendingRelation(null)}
                disabled={creating}
              >
                Cancelar
              </button>
              <button
                type="button"
                className="btn-primary text-xs"
                onClick={handleConfirmPendingRelation}
                disabled={creating || !relationLabel.trim()}
              >
                {creating ? "Creando…" : "Enlazar"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
