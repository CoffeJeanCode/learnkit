import { useEffect, useRef, useState } from "react";
import * as d3 from "d3";
import type { ClassRecord, Concept, ConceptLink } from "../../lib/schemas";
import {
  RELATION_CONNECTIVES,
  RELATION_HINTS,
  classLabel,
  formatRelationLabel,
  truncate,
} from "./graph";
import type { Selection } from "./GraphParts";
import {
  IconClose,
  IconLink,
  IconPlus,
  IconRefresh,
} from "../../components/Icons";

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
  onOpenContextMenu,
  onNewConceptAt,
  searchQuery,
  connectingFromId,
  onCancelConnect,
}: {
  classes: ClassRecord[];
  concepts: Concept[];
  links: ConceptLink[];
  selection: Selection;
  onSelect: (s: Selection) => void;
  onCreateLink: (fromId: string, toId: string, label: string) => Promise<boolean>;
  onOpenContextMenu: (
    e: React.MouseEvent,
    type: "canvas" | "node" | "link",
    targetId?: string,
    worldPos?: { x: number; y: number },
    nodeInfo?: { kind: "class" | "concept"; label: string },
    linkInfo?: { label: string },
  ) => void;
  onNewConceptAt?: (canvasX: number, canvasY: number, sourceNodeId?: string) => void;
  searchQuery?: string;
  connectingFromId?: string | null;
  onCancelConnect?: () => void;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);

  // Figma-style interactive connection rubberband
  const [connectingFrom, setConnectingFrom] = useState<string | null>(null);
  const [snapTargetId, setSnapTargetId] = useState<string | null>(null);
  const [dragLine, setDragLine] = useState<{
    x1: number;
    y1: number;
    x2: number;
    y2: number;
  } | null>(null);

  // Hovered node for revealing Figma 4-handle anchors
  const [hoveredNodeId, setHoveredNodeId] = useState<string | null>(null);

  // Quick relation modal when connection is dropped onto target
  const [pendingRelation, setPendingRelation] = useState<{ fromId: string; toId: string } | null>(null);
  const [relationLabel, setRelationLabel] = useState("");
  const [creating, setCreating] = useState(false);

  // Zoom behavior reference
  const zoomBehaviorRef = useRef<d3.ZoomBehavior<SVGSVGElement, unknown> | null>(null);
  const currentTransformRef = useRef<d3.ZoomTransform>(d3.zoomIdentity);
  const [zoomScale, setZoomScale] = useState(1);

  // Simulation references
  const simRef = useRef<d3.Simulation<D3Node, D3Link> | null>(null);
  const nodesRef = useRef<D3Node[]>([]);
  const linksRef = useRef<D3Link[]>([]);

  // Build node and link datasets & initialize D3 simulation
  useEffect(() => {
    const width = containerRef.current?.clientWidth || 1000;
    const height = containerRef.current?.clientHeight || 700;

    // 1. Build nodes map
    const nodeMap = new Map<string, D3Node>();
    const d3Nodes: D3Node[] = [];

    // Prior layout coordinates cache if re-simulating
    const prevPosMap = new Map<string, { x: number; y: number }>();
    nodesRef.current.forEach((n) => {
      if (typeof n.x === "number" && typeof n.y === "number") {
        prevPosMap.set(n.id, { x: n.x, y: n.y });
      }
    });

    classes.forEach((c, idx) => {
      const prev = prevPosMap.get(c.id);
      const node: D3Node = {
        id: c.id,
        kind: "class",
        label: classLabel(c.title),
        classId: c.id,
        milestoneId: c.milestone_id,
        w: 220,
        h: 46,
        x: prev ? prev.x : width * 0.15 + (idx % 3) * 260,
        y: prev ? prev.y : 120 + Math.floor(idx / 3) * 180,
      };
      nodeMap.set(c.id, node);
      d3Nodes.push(node);
    });

    concepts.forEach((c) => {
      const prev = prevPosMap.get(c.id);
      const parent = nodeMap.get(c.classId);
      const node: D3Node = {
        id: c.id,
        kind: "concept",
        label: c.label,
        classId: c.classId,
        w: 190,
        h: 40,
        x: prev
          ? prev.x
          : parent
            ? (parent.x ?? width / 2) + (Math.random() - 0.5) * 120
            : width / 2,
        y: prev
          ? prev.y
          : parent
            ? (parent.y ?? height / 2) + 85 + (Math.random() - 0.5) * 60
            : height / 2,
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

    // 3. Initialize Force Simulation with gentle decay
    const sim = d3
      .forceSimulation<D3Node>(d3Nodes)
      .force(
        "link",
        d3
          .forceLink<D3Node, D3Link>(d3Links)
          .id((d) => d.id)
          .distance((d) => (d.label ? 180 : 90))
          .strength((d) => (d.label ? 0.6 : 0.8)),
      )
      .force("charge", d3.forceManyBody().strength(-400))
      .force("collide", d3.forceCollide().radius(75).strength(0.75))
      .force("center", d3.forceCenter(width / 2, height / 2).strength(0.04))
      .alphaDecay(0.03);

    simRef.current = sim;

    const svgEl = svgRef.current;
    if (!svgEl) return;
    const svg = d3.select(svgEl);
    const g = svg.select<SVGGElement>(".d3-viewport");

    // 4. Zoom & Pan
    const zoom = d3
      .zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.25, 3.5])
      .on("zoom", (event) => {
        currentTransformRef.current = event.transform;
        setZoomScale(Math.round(event.transform.k * 100));
        g.attr("transform", event.transform);
      });

    zoomBehaviorRef.current = zoom;
    (svg as unknown as d3.Selection<SVGSVGElement, unknown, null, undefined>).call(zoom);

    // 5. Simulation Tick
    sim.on("tick", () => {
      // Update link lines
      g.selectAll<SVGPathElement, D3Link>(".d3-link-path")
        .data(d3Links, (d) => d.id)
        .join("path")
        .attr("d", (d) => {
          const s = d.source as D3Node;
          const t = d.target as D3Node;
          const sx = s.x || 0;
          const sy = s.y || 0;
          const tx = t.x || 0;
          const ty = t.y || 0;
          const dx = tx - sx;
          const dy = ty - sy;
          const cx1 = sx + dx * 0.45;
          const cy1 = sy;
          const cx2 = sx + dx * 0.55;
          const cy2 = ty;
          return `M ${sx} ${sy} C ${cx1} ${cy1}, ${cx2} ${cy2}, ${tx} ${ty}`;
        });

      // Update link labels
      g.selectAll<SVGTextElement, D3Link>(".d3-link-label")
        .data(d3Links.filter((d) => !!d.label), (d) => d.id)
        .join("text")
        .attr("x", (d) => (((d.source as D3Node).x || 0) + ((d.target as D3Node).x || 0)) / 2)
        .attr("y", (d) => (((d.source as D3Node).y || 0) + ((d.target as D3Node).y || 0)) / 2 - 6);

      // Update node groups
      g.selectAll<SVGGElement, D3Node>(".d3-node-group")
        .data(d3Nodes, (d) => d.id)
        .join("g")
        .attr("transform", (d) => `translate(${d.x || 0}, ${d.y || 0})`);

      // Update cluster hulls (LearnHarness-style visual grouping by class)
      const clustersMap = new Map<string, Array<[number, number]>>();
      d3Nodes.forEach((n) => {
        if (typeof n.x === "number" && typeof n.y === "number") {
          const cid = n.classId;
          if (!clustersMap.has(cid)) clustersMap.set(cid, []);
          const pts = clustersMap.get(cid)!;
          const pad = 24;
          const hw = n.w / 2 + pad;
          const hh = n.h / 2 + pad;
          pts.push([n.x - hw, n.y - hh]);
          pts.push([n.x + hw, n.y - hh]);
          pts.push([n.x + hw, n.y + hh]);
          pts.push([n.x - hw, n.y + hh]);
        }
      });

      const clusterData: Array<{ classId: string; pathD: string }> = [];
      const lineGen = d3.line().curve(d3.curveCatmullRomClosed);
      clustersMap.forEach((points, cid) => {
        const hasConcepts = d3Nodes.some((n) => n.classId === cid && n.kind === "concept");
        if (hasConcepts && points.length >= 6) {
          const hull = d3.polygonHull(points);
          if (hull) {
            const d = lineGen(hull);
            if (d) clusterData.push({ classId: cid, pathD: d });
          }
        }
      });

      const hullsLayer = g.select<SVGGElement>(".d3-hulls-layer");
      if (!hullsLayer.empty()) {
        hullsLayer
          .selectAll<SVGPathElement, { classId: string; pathD: string }>(".d3-cluster-hull")
          .data(clusterData, (d) => (d as { classId: string }).classId)
          .join("path")
          .attr("class", "d3-cluster-hull")
          .attr("d", (d) => (d as { pathD: string }).pathD);
      }
    });

    return () => {
      sim.stop();
    };
  }, [classes, concepts, links]);

  // Handle external connectingFrom trigger (e.g. from sidebar or context menu)
  useEffect(() => {
    if (connectingFromId) {
      setConnectingFrom(connectingFromId);
    }
  }, [connectingFromId]);

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

  // Zoom handlers
  const handleZoom = (factor: number) => {
    if (!svgRef.current || !zoomBehaviorRef.current) return;
    const svg = d3.select(svgRef.current);
    svg.transition().duration(200).call(zoomBehaviorRef.current.scaleBy as any, factor);
  };

  const handleResetZoom = () => {
    if (!svgRef.current || !zoomBehaviorRef.current) return;
    const svg = d3.select(svgRef.current);
    svg.transition().duration(250).call(zoomBehaviorRef.current.transform as any, d3.zoomIdentity);
  };

  // Convert client coordinates to canvas world coordinates
  const screenToWorld = (clientX: number, clientY: number) => {
    const rect = svgRef.current?.getBoundingClientRect();
    const transform = currentTransformRef.current;
    if (!rect) return { x: 0, y: 0 };
    return {
      x: (clientX - rect.left - transform.x) / transform.k,
      y: (clientY - rect.top - transform.y) / transform.k,
    };
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

  // Figma-style Connector Handle Drag (from any of 4 anchors)
  const handleConnectorStart = (
    e: React.PointerEvent,
    sourceId: string,
    anchorOffset: { dx: number; dy: number },
  ) => {
    e.stopPropagation();
    setConnectingFrom(sourceId);

    const sourceNode = nodesRef.current.find((n) => n.id === sourceId);
    if (!sourceNode) return;

    const p1 = {
      x: (sourceNode.x || 0) + anchorOffset.dx,
      y: (sourceNode.y || 0) + anchorOffset.dy,
    };

    const onPointerMove = (ev: PointerEvent) => {
      const mouse = screenToWorld(ev.clientX, ev.clientY);
      setDragLine({ x1: p1.x, y1: p1.y, x2: mouse.x, y2: mouse.y });

      // Magnetic snap detection towards candidate nodes
      const target = nodesRef.current.find((n) => {
        if (n.id === sourceId) return false;
        const dx = (n.x || 0) - mouse.x;
        const dy = (n.y || 0) - mouse.y;
        return Math.hypot(dx, dy) < 65;
      });
      setSnapTargetId(target ? target.id : null);
    };

    const onPointerUp = (ev: PointerEvent) => {
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
      setDragLine(null);
      setConnectingFrom(null);
      setSnapTargetId(null);

      const mouse = screenToWorld(ev.clientX, ev.clientY);

      // Check if dropped onto a target node
      const target = nodesRef.current.find((n) => {
        if (n.id === sourceId) return false;
        const dx = (n.x || 0) - mouse.x;
        const dy = (n.y || 0) - mouse.y;
        return Math.hypot(dx, dy) < 65;
      });

      if (target) {
        setPendingRelation({ fromId: sourceId, toId: target.id });
        setRelationLabel("causa");
      } else if (onNewConceptAt && Math.hypot(mouse.x - p1.x, mouse.y - p1.y) > 80) {
        // Dropped onto empty canvas far from start: offer creating and connecting a concept!
        onNewConceptAt(mouse.x, mouse.y, sourceId);
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

  // Re-warm simulation
  const handleReorganize = () => {
    if (simRef.current) {
      simRef.current.alpha(0.7).restart();
    }
  };

  return (
    <div
      ref={containerRef}
      className="graph-canvas-wrapper"
      onContextMenu={(e) => {
        e.preventDefault();
        const world = screenToWorld(e.clientX, e.clientY);
        onOpenContextMenu(e, "canvas", undefined, world);
      }}
      onClick={() => {
        if (connectingFrom) {
          setConnectingFrom(null);
          onCancelConnect?.();
        }
      }}
    >
      {/* Floating Canvas Header Overlay (Top Left) */}
      <div className="graph-canvas-overlay-topleft">
        <div className="graph-badge-pill">
          <span className="graph-badge-title">Grafo de ideas</span>
          <span className="graph-badge-count">
            {concepts.length} conceptos · {links.length} relaciones
          </span>
        </div>
        <div className="flex items-center gap-1">
          <button
            type="button"
            className="graph-overlay-btn"
            onClick={() => onNewConceptAt?.(0, 0)}
            title="Crear un nuevo concepto"
          >
            <IconPlus />
            <span>Concepto</span>
          </button>
          <button
            type="button"
            className="graph-overlay-btn"
            onClick={handleResetZoom}
            title="Centrar vista (100%)"
          >
            <IconRefresh />
            <span>Centrar</span>
          </button>
        </div>
      </div>

      {/* Floating Zoom Toolbar Overlay (Bottom Right) */}
      <div className="graph-canvas-overlay-bottomright">
        <div className="graph-zoom-toolbar">
          <button
            type="button"
            className="graph-zoom-btn"
            onClick={() => handleZoom(1.25)}
            title="Acercar (+)"
          >
            ＋
          </button>
          <span className="graph-zoom-val" title="Escala actual">
            {zoomScale}%
          </span>
          <button
            type="button"
            className="graph-zoom-btn"
            onClick={() => handleZoom(0.8)}
            title="Alejar (-)"
          >
            －
          </button>
          <button
            type="button"
            className="graph-zoom-btn text-2xs px-2"
            onClick={handleResetZoom}
            title="Restablecer vista (1:1)"
          >
            Centrar
          </button>
          <button
            type="button"
            className="graph-zoom-btn text-2xs px-2"
            onClick={handleReorganize}
            title="Reorganizar nodos automáticamente"
          >
            Ordenar
          </button>
        </div>
      </div>

      {/* Floating Hint Overlay (Bottom Left) */}
      <div className="graph-canvas-overlay-bottomleft">
        <span className="text-2xs text-slate-400">
          💡 Arrastra los 4 puntos azules de cualquier nodo para conectar como en Figma · Clic derecho para opciones
        </span>
      </div>

      {/* SVG Canvas with D3 Viewport */}
      <svg
        ref={svgRef}
        className="graph-canvas-svg"
        role="group"
        aria-label="Grafo de conocimiento interactivo"
      >
        <defs>
          {/* Dot matrix grid pattern for infinite canvas feel */}
          <pattern id="d3-dot-grid" width="28" height="28" patternUnits="userSpaceOnUse">
            <circle cx="2" cy="2" r="1.2" fill="var(--chalk-soft, #475569)" opacity="0.25" />
          </pattern>

          {/* Arrow markers */}
          <marker
            id="d3-arrow"
            viewBox="0 0 10 10"
            refX="30"
            refY="5"
            markerWidth="6"
            markerHeight="6"
            orient="auto-start-reverse"
          >
            <path d="M 0 0 L 10 5 L 0 10 z" fill="var(--blue-chalk, #7fb0d8)" opacity="0.85" />
          </marker>
          <marker
            id="d3-arrow-active"
            viewBox="0 0 10 10"
            refX="10"
            refY="5"
            markerWidth="7"
            markerHeight="7"
            orient="auto-start-reverse"
          >
            <path d="M 0 0 L 10 5 L 0 10 z" fill="var(--gold-chalk, #d9b168)" />
          </marker>
        </defs>

        {/* Canvas background dot grid */}
        <rect width="100%" height="100%" fill="url(#d3-dot-grid)" />

        <g className="d3-viewport">
          {/* Cluster hulls layer: visual territories of knowledge (LearnHarness-style) */}
          <g className="d3-hulls-layer" />

          {/* Link paths */}
          {linksRef.current.map((link) => {
            const isExplicit = !!link.label;
            const isSelected = selection?.kind === "link" && selection.id === link.id;
            const dimmed =
              connectedNodeIds.size > 0 &&
              !connectedNodeIds.has(link.fromId) &&
              !connectedNodeIds.has(link.toId);

            return (
              <path
                key={link.id}
                className={`d3-link-path transition-all duration-200 cursor-pointer ${
                  isExplicit
                    ? "stroke-[2px] stroke-sky-400/75 hover:stroke-sky-300 fill-none"
                    : "stroke-[1.2px] stroke-slate-700/50 stroke-dasharray-[4,4] fill-none"
                } ${isSelected ? "stroke-amber-400 stroke-[3px]" : ""} ${dimmed ? "opacity-15" : "opacity-90"}`}
                markerEnd={isExplicit ? "url(#d3-arrow)" : undefined}
                onClick={(e) => {
                  e.stopPropagation();
                  if (isExplicit) onSelect({ kind: "link", id: link.id });
                }}
                onContextMenu={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  if (isExplicit) {
                    onOpenContextMenu(e, "link", link.id, undefined, undefined, {
                      label: link.label,
                    });
                  }
                }}
              />
            );
          })}

          {/* Interactive rubberband drag line (Figma style) */}
          {dragLine && (
            <path
              d={`M ${dragLine.x1} ${dragLine.y1} C ${(dragLine.x1 + dragLine.x2) / 2} ${dragLine.y1}, ${(dragLine.x1 + dragLine.x2) / 2} ${dragLine.y2}, ${dragLine.x2} ${dragLine.y2}`}
              fill="none"
              stroke="var(--gold-chalk, #d9b168)"
              strokeWidth="2.5"
              strokeDasharray="5 4"
              markerEnd="url(#d3-arrow-active)"
              className="pointer-events-none"
            />
          )}

          {/* Link labels */}
          {linksRef.current
            .filter((l) => !!l.label)
            .map((link) => {
              const isSelected = selection?.kind === "link" && selection.id === link.id;
              const dimmed =
                connectedNodeIds.size > 0 &&
                !connectedNodeIds.has(link.fromId) &&
                !connectedNodeIds.has(link.toId);

              return (
                <text
                  key={`lbl-${link.id}`}
                  className={`d3-link-label text-[11px] font-mono cursor-pointer transition-opacity duration-200 ${
                    isSelected ? "fill-amber-300 font-bold" : "fill-slate-300 hover:fill-slate-100"
                  } ${dimmed ? "opacity-15" : "opacity-95"}`}
                  textAnchor="middle"
                  onClick={(e) => {
                    e.stopPropagation();
                    onSelect({ kind: "link", id: link.id });
                  }}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    onOpenContextMenu(e, "link", link.id, undefined, undefined, {
                      label: link.label,
                    });
                  }}
                >
                  {truncate(formatRelationLabel(link.label), 28)}
                </text>
              );
            })}

          {/* Nodes */}
          {nodesRef.current.map((node) => {
            const isSelected = selection?.kind === "node" && selection.id === node.id;
            const isConnected = connectedNodeIds.has(node.id);
            const isSnapTarget = snapTargetId === node.id;
            const isHovered = hoveredNodeId === node.id;
            const dimmed =
              (connectedNodeIds.size > 0 && !isConnected) ||
              (!!searchQuery &&
                !node.label.toLowerCase().includes(searchQuery.toLowerCase()));
            const isClass = node.kind === "class";

            // 4 Figma connector handle positions (top, right, bottom, left)
            const handles = [
              { id: "top", cx: 0, cy: -node.h / 2, dx: 0, dy: -node.h / 2 },
              { id: "right", cx: node.w / 2, cy: 0, dx: node.w / 2, dy: 0 },
              { id: "bottom", cx: 0, cy: node.h / 2, dx: 0, dy: node.h / 2 },
              { id: "left", cx: -node.w / 2, cy: 0, dx: -node.w / 2, dy: 0 },
            ];

            return (
              <g
                key={node.id}
                className={`d3-node-group cursor-grab active:cursor-grabbing transition-opacity duration-200 ${
                  dimmed ? "opacity-25" : "opacity-100"
                }`}
                onPointerDown={(e) => handleNodePointerDown(e, node)}
                onClick={(e) => {
                  e.stopPropagation();
                  onSelect({ kind: "node", id: node.id });
                }}
                onMouseEnter={() => setHoveredNodeId(node.id)}
                onMouseLeave={() => setHoveredNodeId(null)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  const world = screenToWorld(e.clientX, e.clientY);
                  onOpenContextMenu(e, "node", node.id, world, {
                    kind: node.kind,
                    label: node.label,
                  });
                }}
              >
                {/* Magnetic Snap Target Halo (when dragging connection) */}
                {isSnapTarget && (
                  <rect
                    x={-node.w / 2 - 6}
                    y={-node.h / 2 - 6}
                    width={node.w + 12}
                    height={node.h + 12}
                    rx={18}
                    className="fill-none stroke-amber-400 stroke-2 animate-pulse pointer-events-none"
                  />
                )}

                {/* Node Box */}
                <rect
                  x={-node.w / 2}
                  y={-node.h / 2}
                  width={node.w}
                  height={node.h}
                  rx={14}
                  className={`transition-colors duration-150 ${
                    isClass
                      ? isSelected
                        ? "fill-sky-950 stroke-sky-400 stroke-2"
                        : "fill-surface-deep stroke-slate-700 stroke-1 hover:stroke-slate-500"
                      : isSelected
                        ? "fill-amber-950 stroke-amber-400 stroke-2"
                        : "fill-card stroke-slate-800 stroke-1 hover:stroke-slate-600"
                  }`}
                />

                {/* Node Type Pill indicator on Class */}
                {isClass && (
                  <text
                    x={-node.w / 2 + 14}
                    y={-node.h / 2 + 14}
                    className="text-[9px] uppercase tracking-wider font-mono fill-sky-400 font-semibold pointer-events-none select-none"
                  >
                    Clase
                  </text>
                )}

                {/* Node Label Text */}
                <text
                  x={-node.w / 2 + 14}
                  y={isClass ? 8 : 4}
                  className={`text-xs select-none pointer-events-none font-medium ${
                    isClass ? "fill-sky-100 font-semibold" : "fill-slate-100"
                  }`}
                >
                  {truncate(node.label, isClass ? 24 : 20)}
                </text>

                {/* 4 Figma-Style Connection Anchors (Top, Right, Bottom, Left) */}
                {(isHovered || connectingFrom === node.id || isSelected) && (
                  <g className="figma-connection-anchors">
                    {handles.map((h) => (
                      <circle
                        key={h.id}
                        cx={h.cx}
                        cy={h.cy}
                        r={6}
                        className="fill-sky-400 hover:fill-amber-400 stroke-paper stroke-2 cursor-crosshair transition-transform hover:scale-125"
                        onPointerDown={(e) =>
                          handleConnectorStart(e, node.id, { dx: h.dx, dy: h.dy })
                        }
                      >
                        <title>Arrastra para conectar con otro nodo (estilo Figma)</title>
                      </circle>
                    ))}
                  </g>
                )}
              </g>
            );
          })}
        </g>
      </svg>

      {/* Floating Relation Creator Modal when connection is dropped */}
      {pendingRelation && (
        <div className="absolute inset-0 z-30 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4">
          <div className="w-full max-w-md p-4 rounded-xl bg-surface-deep border border-slate-700 shadow-2xl space-y-3 animate-in fade-in zoom-in-95">
            <div className="flex items-center justify-between pb-1 border-b border-slate-800">
              <h4 className="text-sm font-semibold text-slate-100 flex items-center gap-1.5">
                <IconLink /> Relacionar conceptos
              </h4>
              <button
                type="button"
                className="text-slate-400 hover:text-slate-200"
                onClick={() => setPendingRelation(null)}
              >
                <IconClose />
              </button>
            </div>

            {/* Connection visual preview: from -> to */}
            <div className="flex items-center gap-2 p-2 rounded-lg bg-paper-deep/90 border border-slate-800 text-xs">
              <span className="font-semibold text-slate-200 truncate flex-1 text-left">
                {nodesRef.current.find((n) => n.id === pendingRelation.fromId)?.label || pendingRelation.fromId}
              </span>
              <span className="text-amber-400 font-mono font-bold px-1">⟹</span>
              <span className="font-semibold text-slate-200 truncate flex-1 text-right">
                {nodesRef.current.find((n) => n.id === pendingRelation.toId)?.label || pendingRelation.toId}
              </span>
            </div>

            <div>
              <label className="block text-2xs text-slate-400 uppercase tracking-wider mb-1.5">
                Tipo de asociación lógica (LearnHarness)
              </label>
              <div className="grid grid-cols-2 gap-1.5">
                {RELATION_CONNECTIVES.map((c) => (
                  <button
                    key={c.value}
                    type="button"
                    className={`chip text-2xs justify-start gap-1.5 py-1.5 px-2 ${
                      relationLabel === c.value ? "is-active" : ""
                    }`}
                    onClick={() => setRelationLabel(c.value)}
                    title={c.description}
                  >
                    <span className="font-mono text-amber-300 font-bold text-xs">{c.glyph}</span>
                    <span className="truncate">{c.label}</span>
                  </button>
                ))}
              </div>
            </div>

            <div>
              <label className="block text-2xs text-slate-400 uppercase tracking-wider mb-1">
                O escribe con tus propias palabras
              </label>
              <input
                autoFocus
                className="w-full p-2 text-xs rounded bg-input text-slate-100 border border-slate-700 font-mono"
                value={relationLabel}
                placeholder="Ej. deriva en, fundamenta a, se compone con…"
                onChange={(e) => setRelationLabel(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleConfirmPendingRelation()}
              />
            </div>

            <div className="flex items-center justify-end gap-2 pt-2 border-t border-slate-800">
              <button
                type="button"
                className="btn-quiet text-xs py-1 px-3"
                onClick={() => setPendingRelation(null)}
                disabled={creating}
              >
                Cancelar
              </button>
              <button
                type="button"
                className="btn-primary text-xs py-1 px-3"
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
