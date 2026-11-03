import React, {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
} from "react";
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

export interface GraphCanvasHandle {
  fitToCenter: () => void;
  reorganize: () => void;
}

export interface GraphCanvasD3Props {
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
}

export const GraphCanvasD3 = forwardRef<GraphCanvasHandle, GraphCanvasD3Props>(
  function GraphCanvasD3(
    {
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
    },
    ref,
  ) {
    const containerRef = useRef<HTMLDivElement>(null);
    const svgRef = useRef<SVGSVGElement>(null);
    const zoomValRef = useRef<HTMLSpanElement>(null);

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
    const [pendingRelation, setPendingRelation] = useState<{
      fromId: string;
      toId: string;
    } | null>(null);
    const [relationLabel, setRelationLabel] = useState("deriva en");
    const [creating, setCreating] = useState(false);

    // Reactive render state for React DOM elements
    const [renderNodes, setRenderNodes] = useState<D3Node[]>([]);
    const [renderLinks, setRenderLinks] = useState<D3Link[]>([]);
    const connectingFromRef = useRef<string | null>(null);

    // Zoom behavior reference
    const zoomBehaviorRef = useRef<d3.ZoomBehavior<SVGSVGElement, unknown> | null>(null);
    const currentTransformRef = useRef<d3.ZoomTransform>(d3.zoomIdentity);

    // Simulation references
    const simRef = useRef<d3.Simulation<D3Node, D3Link> | null>(null);
    const nodesRef = useRef<D3Node[]>([]);
    const linksRef = useRef<D3Link[]>([]);

    // Centers the camera smoothly around all current nodes
    const fitToCenter = () => {
      if (!svgRef.current || !zoomBehaviorRef.current || !containerRef.current) return;
      const svg = d3.select(svgRef.current);
      const width = containerRef.current.clientWidth || svgRef.current.clientWidth || 1000;
      const height = containerRef.current.clientHeight || svgRef.current.clientHeight || 700;

      const nodes = nodesRef.current;
      if (nodes.length === 0) {
        svg
          .interrupt()
          .transition()
          .duration(350)
          .call(zoomBehaviorRef.current.transform as any, d3.zoomIdentity);
        return;
      }

      let minX = Infinity;
      let maxX = -Infinity;
      let minY = Infinity;
      let maxY = -Infinity;

      nodes.forEach((n) => {
        const nx = typeof n.x === "number" ? n.x : width / 2;
        const ny = typeof n.y === "number" ? n.y : height / 2;
        minX = Math.min(minX, nx - n.w / 2);
        maxX = Math.max(maxX, nx + n.w / 2);
        minY = Math.min(minY, ny - n.h / 2);
        maxY = Math.max(maxY, ny + n.h / 2);
      });

      const padding = 120;
      const boxW = Math.max(maxX - minX, 160) + padding * 2;
      const boxH = Math.max(maxY - minY, 160) + padding * 2;
      const midX = (minX + maxX) / 2;
      const midY = (minY + maxY) / 2;

      const scale = Math.min(Math.max(Math.min(width / boxW, height / boxH), 0.4), 1.25);
      const translateX = width / 2 - midX * scale;
      const translateY = height / 2 - midY * scale;

      const transform = d3.zoomIdentity.translate(translateX, translateY).scale(scale);
      svg
        .interrupt()
        .transition()
        .duration(450)
        .call(zoomBehaviorRef.current.transform as any, transform);
    };

    // Re-warms simulation with slight jitter to pop apart stacked nodes
    const reorganize = () => {
      if (!simRef.current || !containerRef.current) return;
      const width = containerRef.current.clientWidth || 1000;
      const height = containerRef.current.clientHeight || 700;

      nodesRef.current.forEach((n, i) => {
        const angle = (i / Math.max(1, nodesRef.current.length)) * Math.PI * 2;
        n.x = (n.x ?? width / 2) + Math.cos(angle) * 45;
        n.y = (n.y ?? height / 2) + Math.sin(angle) * 45;
        n.vx = (Math.random() - 0.5) * 8;
        n.vy = (Math.random() - 0.5) * 8;
      });
      simRef.current.alpha(0.85).restart();
    };

    useImperativeHandle(ref, () => ({
      fitToCenter,
      reorganize,
    }));

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

      const cols = Math.max(1, Math.min(classes.length, 3));
      classes.forEach((c, idx) => {
        const prev = prevPosMap.get(c.id);
        const col = idx % cols;
        const row = Math.floor(idx / cols);
        const startX = width * 0.25;
        const stepX = Math.min(300, (width * 0.5) / Math.max(1, cols - 1));
        const node: D3Node = {
          id: c.id,
          kind: "class",
          label: classLabel(c.title),
          classId: c.id,
          milestoneId: c.milestone_id,
          w: 220,
          h: 46,
          x: prev
            ? prev.x
            : startX + col * (classes.length === 1 ? 0 : stepX) + (Math.random() - 0.5) * 20,
          y: prev ? prev.y : 140 + row * 190 + (Math.random() - 0.5) * 20,
        };
        nodeMap.set(c.id, node);
        d3Nodes.push(node);
      });

      concepts.forEach((c, idx) => {
        const prev = prevPosMap.get(c.id);
        const parent = nodeMap.get(c.classId);
        const angle = (idx / Math.max(1, concepts.length)) * Math.PI * 2;
        const radius = 95 + (idx % 2) * 35;
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
              ? (parent.x ?? width / 2) + Math.cos(angle) * radius
              : width / 2 + Math.cos(angle) * radius + (Math.random() - 0.5) * 40,
          y: prev
            ? prev.y
            : parent
              ? (parent.y ?? height / 2) + Math.sin(angle) * radius + 40
              : height / 2 + Math.sin(angle) * radius + (Math.random() - 0.5) * 40,
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
      setRenderNodes(d3Nodes);
      setRenderLinks(d3Links);

      // 3. Initialize Force Simulation with calm fluid damping & zero-torque centering
      const sim = d3
        .forceSimulation<D3Node>(d3Nodes)
        .velocityDecay(0.65) // High damping prevents angular swing/orbiting
        .force(
          "link",
          d3
            .forceLink<D3Node, D3Link>(d3Links)
            .id((d) => d.id)
            .distance((d) => (d.label ? 180 : 110))
            .strength((d) => (d.label ? 0.45 : 0.35)),
        )
        .force("charge", d3.forceManyBody().strength(-350).distanceMax(550))
        .force("collide", d3.forceCollide().radius(78).strength(0.85))
        .force("x", d3.forceX(width / 2).strength(0.03)) // Zero-curl orthogonal X centering
        .force("y", d3.forceY(height / 2).strength(0.03)) // Zero-curl orthogonal Y centering
        .alphaDecay(0.04);

      simRef.current = sim;

      const svgEl = svgRef.current;
      if (!svgEl) return;
      const svg = d3.select(svgEl);
      const g = svg.select<SVGGElement>(".d3-viewport");

      // 4. Zoom & Pan
      const zoom = d3
        .zoom<SVGSVGElement, unknown>()
        .scaleExtent([0.25, 3.5])
        .filter((event) => {
          if (event.type === "wheel") return true;
          const target = event.target as Element | null;
          if (target) {
            if (
              target.closest(".d3-node-group") ||
              target.closest(".figma-connection-anchors") ||
              target.closest(".d3-link-path") ||
              target.closest(".d3-link-label") ||
              target.closest("button") ||
              target.closest(".graph-canvas-overlay-topleft") ||
              target.closest(".graph-canvas-overlay-bottomright") ||
              target.closest(".graph-canvas-overlay-bottomleft") ||
              target.closest(".graph-modal-backdrop")
            ) {
              return false;
            }
          }
          if (connectingFromRef.current) return false;
          if (event.button === 1) return true;
          return event.button === 0;
        })
        .on("zoom", (event) => {
          currentTransformRef.current = event.transform;
          if (zoomValRef.current) {
            zoomValRef.current.textContent = `${Math.round(event.transform.k * 100)}%`;
          }
          g.attr("transform", event.transform);
        });

      zoomBehaviorRef.current = zoom;
      (svg as unknown as d3.Selection<SVGSVGElement, unknown, null, undefined>).call(zoom);

      // 5. Simulation Tick: Direct SVG element updates without destroying React DOM
      let tickCount = 0;
      sim.on("tick", () => {
        tickCount++;

        // Update link lines
        d3Links.forEach((link) => {
          const s = link.source as D3Node;
          const t = link.target as D3Node;
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
          const pathD = `M ${sx} ${sy} C ${cx1} ${cy1}, ${cx2} ${cy2}, ${tx} ${ty}`;

          const el = document.getElementById(`d3-link-${link.id}`);
          if (el) el.setAttribute("d", pathD);
          const hitEl = document.getElementById(`d3-link-hit-${link.id}`);
          if (hitEl) hitEl.setAttribute("d", pathD);
        });

        // Update link labels
        d3Links.forEach((link) => {
          if (!link.label) return;
          const labelEl = document.getElementById(`d3-label-${link.id}`);
          if (labelEl) {
            const s = link.source as D3Node;
            const t = link.target as D3Node;
            const mx = ((s.x || 0) + (t.x || 0)) / 2;
            const my = ((s.y || 0) + (t.y || 0)) / 2 - 6;
            labelEl.setAttribute("x", String(mx));
            labelEl.setAttribute("y", String(my));
          }
        });

        // Update node groups
        d3Nodes.forEach((node) => {
          const nodeEl = document.getElementById(`d3-node-${node.id}`);
          if (nodeEl) {
            nodeEl.setAttribute("transform", `translate(${node.x || 0}, ${node.y || 0})`);
          }
        });

        // Update cluster hulls (LearnHarness-style visual grouping by class)
        const clustersMap = new Map<string, Array<[number, number]>>();
        d3Nodes.forEach((n) => {
          if (typeof n.x === "number" && typeof n.y === "number") {
            const cid = n.classId;
            if (!clustersMap.has(cid)) clustersMap.set(cid, []);
            const pts = clustersMap.get(cid)!;
            const pad = 28;
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

        // Auto-center view once initial layout settles
        if (tickCount === 28) {
          fitToCenter();
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

    useEffect(() => {
      connectingFromRef.current = connectingFrom;
    }, [connectingFrom]);

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

    // Node Drag handlers (D3 force drag with movement-threshold to avoid hold-spin)
    const handleNodePointerDown = (e: React.PointerEvent, node: D3Node) => {
      if (e.button !== 0 || connectingFrom) return;
      e.stopPropagation();

      const startX = e.clientX;
      const startY = e.clientY;
      let isDragging = false;
      const capturedTarget = e.currentTarget as HTMLElement;

      const handlePointerMove = (moveEvent: PointerEvent) => {
        const dist = Math.hypot(moveEvent.clientX - startX, moveEvent.clientY - startY);
        // Only start drag if movement exceeds threshold (prevent hold from activating forces)
        if (!isDragging && dist < 4) return;

        if (!isDragging) {
          isDragging = true;
          capturedTarget.setPointerCapture?.(e.pointerId);
          simRef.current?.alphaTarget(0.12).restart();
        }

        const world = screenToWorld(moveEvent.clientX, moveEvent.clientY);
        node.fx = world.x;
        node.fy = world.y;
      };

      const handlePointerUp = (upEvent: PointerEvent) => {
        window.removeEventListener("pointermove", handlePointerMove);
        window.removeEventListener("pointerup", handlePointerUp);
        if (isDragging) {
          capturedTarget.releasePointerCapture?.(upEvent.pointerId);
          simRef.current?.alphaTarget(0);
          node.fx = null;
          node.fy = null;
        }
      };

      window.addEventListener("pointermove", handlePointerMove);
      window.addEventListener("pointerup", handlePointerUp);
    };

    // Figma-style Connector Handle Drag (from any of 4 anchors)
    const handleConnectorStart = (
      e: React.PointerEvent,
      sourceId: string,
      anchorOffset: { dx: number; dy: number },
    ) => {
      e.stopPropagation();
      e.preventDefault();
      connectingFromRef.current = sourceId;
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
          return Math.hypot(dx, dy) < 70;
        });
        setSnapTargetId(target ? target.id : null);
      };

      const onPointerUp = (ev: PointerEvent) => {
        window.removeEventListener("pointermove", onPointerMove);
        window.removeEventListener("pointerup", onPointerUp);
        connectingFromRef.current = null;
        setDragLine(null);
        setConnectingFrom(null);
        setSnapTargetId(null);

        const mouse = screenToWorld(ev.clientX, ev.clientY);

        // Check if dropped onto a target node
        const target = nodesRef.current.find((n) => {
          if (n.id === sourceId) return false;
          const dx = (n.x || 0) - mouse.x;
          const dy = (n.y || 0) - mouse.y;
          return Math.hypot(dx, dy) < 70;
        });

        if (target) {
          setPendingRelation({ fromId: sourceId, toId: target.id });
          setRelationLabel("deriva en");
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
          <div className="flex items-center gap-1.5">
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
              onClick={fitToCenter}
              title="Centrar vista (encuadrar todas las ideas)"
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
            <span ref={zoomValRef} className="graph-zoom-val" title="Escala actual">
              100%
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
              onClick={fitToCenter}
              title="Centrar vista en los nodos"
            >
              Centrar
            </button>
            <button
              type="button"
              className="graph-zoom-btn text-2xs px-2"
              onClick={reorganize}
              title="Reorganizar nodos automáticamente"
            >
              Ordenar
            </button>
          </div>
        </div>

        {/* Floating Hint Overlay (Bottom Left) */}
        <div className="graph-canvas-overlay-bottomleft">
          <span className="text-2xs text-chalk-soft">
            💡 Arrastra los 4 puntos azules de cualquier nodo para conectar como en Figma · Clic derecho para opciones
          </span>
        </div>

        {/* Main D3 Viewport SVG */}
        <svg ref={svgRef} className="graph-canvas-svg" aria-label="Lienzo del grafo de conocimiento">
          <defs>
            <pattern id="d3-dot-grid" width="28" height="28" patternUnits="userSpaceOnUse">
              <circle cx="2" cy="2" r="1.2" fill="var(--chalk-faint, rgba(255,255,255,0.08))" />
            </pattern>

            <marker
              id="d3-arrow"
              viewBox="0 -5 10 10"
              refX="18"
              refY="0"
              markerWidth="7"
              markerHeight="7"
              orient="auto"
            >
              <path d="M 0 -4 L 8 0 L 0 4 z" fill="var(--chalk-soft)" />
            </marker>

            <marker
              id="d3-arrow-active"
              viewBox="0 -5 10 10"
              refX="18"
              refY="0"
              markerWidth="8"
              markerHeight="8"
              orient="auto"
            >
              <path d="M 0 -4 L 8 0 L 0 4 z" fill="var(--gold-chalk)" />
            </marker>
          </defs>

          {/* Grid Background */}
          <rect width="100%" height="100%" fill="url(#d3-dot-grid)" />

          <g className="d3-viewport">
            {/* Cluster hulls layer: visual territories of knowledge (LearnHarness-style) */}
            <g className="d3-hulls-layer" />

            {/* Link paths */}
            {renderLinks.map((link) => {
              const isExplicit = !!link.label;
              const isSelected = selection?.kind === "link" && selection.id === link.id;
              const isConnected =
                connectedNodeIds.size > 0 &&
                (connectedNodeIds.has(link.fromId) || connectedNodeIds.has(link.toId));
              const dimmed = connectedNodeIds.size > 0 && !isConnected;

              return (
                <g key={link.id}>
                  {/* Invisible fat hit line for easy selection */}
                  <path
                    id={`d3-link-hit-${link.id}`}
                    className="cursor-pointer"
                    fill="none"
                    stroke="transparent"
                    strokeWidth="16"
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
                  />

                  {/* Visible link curve */}
                  <path
                    id={`d3-link-${link.id}`}
                    className={`d3-link-path cursor-pointer ${
                      dimmed ? "opacity-15" : "opacity-100"
                    }`}
                    fill="none"
                    stroke={
                      isSelected
                        ? "var(--gold-chalk)"
                        : isConnected
                          ? "var(--gold-chalk)"
                          : isExplicit
                            ? "var(--chalk-soft)"
                            : "var(--border-soft)"
                    }
                    strokeWidth={isSelected ? 2.5 : isExplicit ? 1.8 : 1.2}
                    strokeDasharray={isExplicit ? undefined : "4 4"}
                    markerEnd={isExplicit ? (isSelected ? "url(#d3-arrow-active)" : "url(#d3-arrow)") : undefined}
                    style={{ transition: "stroke 0.15s ease, opacity 0.2s ease" }}
                    onClick={(e) => {
                      e.stopPropagation();
                      onSelect({ kind: "link", id: link.id });
                    }}
                  />
                </g>
              );
            })}

            {/* Interactive rubberband drag line (Figma style) */}
            {dragLine && (
              <path
                d={`M ${dragLine.x1} ${dragLine.y1} C ${(dragLine.x1 + dragLine.x2) / 2} ${dragLine.y1}, ${(dragLine.x1 + dragLine.x2) / 2} ${dragLine.y2}, ${dragLine.x2} ${dragLine.y2}`}
                fill="none"
                stroke="#fbbf24"
                strokeWidth="2.5"
                strokeDasharray="5 4"
                markerEnd="url(#d3-arrow-active)"
                className="pointer-events-none"
              />
            )}

            {/* Link labels */}
            {renderLinks
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
                    id={`d3-label-${link.id}`}
                    className={`d3-link-label cursor-pointer ${
                      dimmed ? "opacity-20" : "opacity-100"
                    }`}
                    textAnchor="middle"
                    fill={isSelected ? "var(--gold-chalk)" : "var(--chalk)"}
                    fontSize={11}
                    fontFamily="var(--font-mono)"
                    fontWeight={isSelected ? 700 : 500}
                    paintOrder="stroke"
                    stroke="var(--paper-deep)"
                    strokeWidth={3}
                    strokeLinejoin="round"
                    style={{ userSelect: "none", transition: "opacity 0.2s ease" }}
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
            {renderNodes.map((node) => {
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
                  id={`d3-node-${node.id}`}
                  className={`d3-node-group cursor-grab active:cursor-grabbing transition-opacity duration-200 ${
                    dimmed ? "opacity-25" : "opacity-100"
                  }`}
                  transform={`translate(${node.x || 0}, ${node.y || 0})`}
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
                      rx={16}
                      fill="none"
                      stroke="#fbbf24"
                      strokeWidth={2.5}
                      strokeDasharray="4 3"
                      className="pointer-events-none"
                    />
                  )}

                  {/* Node Box */}
                  <rect
                    x={-node.w / 2}
                    y={-node.h / 2}
                    width={node.w}
                    height={node.h}
                    rx={12}
                    fill={
                      isClass
                        ? isSelected
                          ? "var(--sky-dark)"
                          : "var(--surface-card-hi)"
                        : isSelected
                          ? "var(--amber-dark)"
                          : "var(--surface-card)"
                    }
                    stroke={
                      isSelected
                        ? "var(--gold-chalk)"
                        : isClass
                          ? "var(--blue-chalk)"
                          : isConnected
                            ? "var(--gold-chalk)"
                            : "var(--border-soft)"
                    }
                    strokeWidth={isSelected ? 2.5 : isClass ? 1.8 : 1.2}
                    style={{
                      filter: isSelected
                        ? "drop-shadow(0 0 10px rgba(217, 177, 104, 0.45))"
                        : isHovered
                          ? "drop-shadow(0 4px 12px rgba(0, 0, 0, 0.35))"
                          : undefined,
                      transition: "stroke 0.15s ease, filter 0.15s ease",
                    }}
                  />

                  {/* Node Type Pill indicator on Class */}
                  {isClass && (
                    <g transform={`translate(${-node.w / 2 + 10}, ${-node.h / 2 + 6})`}>
                      <rect
                        width={44}
                        height={15}
                        rx={4}
                        style={{
                          fill: "var(--blue-wash, rgba(37, 99, 235, 0.2))",
                          stroke: "var(--blue-chalk, #7fb0d8)",
                          strokeWidth: 0.8,
                        }}
                      />
                      <text
                        x={22}
                        y={11}
                        textAnchor="middle"
                        fontSize={9}
                        fontFamily="var(--font-mono, monospace)"
                        fontWeight={700}
                        letterSpacing="0.05em"
                        style={{
                          fill: "var(--blue-chalk, #7fb0d8)",
                          userSelect: "none",
                          pointerEvents: "none",
                        }}
                      >
                        CLASE
                      </text>
                    </g>
                  )}

                  {/* Concept badge or glyph */}
                  {!isClass && (
                    <circle
                      cx={-node.w / 2 + 18}
                      cy={0}
                      r={5}
                      style={{
                        fill: "var(--gold-chalk, #d9b168)",
                        opacity: 0.9,
                      }}
                    />
                  )}

                  {/* Node Label Text */}
                  <text
                    x={isClass ? -node.w / 2 + 14 : -node.w / 2 + 30}
                    y={isClass ? 12 : 5}
                    className="d3-node-text"
                    fontSize={isClass ? 12 : 13}
                    fontFamily="var(--font-body, system-ui, sans-serif)"
                    fontWeight={isClass ? 600 : 500}
                    style={{ fill: "var(--chalk, #e8e2d4)", userSelect: "none", pointerEvents: "none" }}
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
                          r={7}
                          cursor="crosshair"
                          style={{
                            fill: "var(--blue-chalk, #7fb0d8)",
                            stroke: "var(--paper-deep, #121417)",
                            strokeWidth: 2,
                            transition: "all 0.15s ease",
                          }}
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
          <div className="graph-modal-backdrop">
            <div className="graph-modal-box">
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  paddingBottom: "8px",
                  marginBottom: "12px",
                  borderBottom: "1px solid var(--border-soft, #334155)",
                }}
              >
                <h4
                  style={{
                    fontSize: "14px",
                    fontWeight: 600,
                    color: "var(--chalk, #f8fafc)",
                    display: "flex",
                    alignItems: "center",
                    gap: "6px",
                    margin: 0,
                  }}
                >
                  <IconLink /> Relacionar conceptos
                </h4>
                <button
                  type="button"
                  className="btn-quiet"
                  style={{ padding: "4px", color: "var(--chalk-soft, #94a3b8)" }}
                  onClick={() => setPendingRelation(null)}
                >
                  <IconClose />
                </button>
              </div>

              {/* Connection visual preview: from -> to */}
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "8px",
                  padding: "8px 12px",
                  borderRadius: "8px",
                  background: "var(--card, #1e293b)",
                  border: "1px solid var(--border-soft, #334155)",
                  fontSize: "12px",
                  marginBottom: "12px",
                }}
              >
                <span
                  style={{
                    fontWeight: 600,
                    color: "var(--chalk, #f8fafc)",
                    flex: 1,
                    textAlign: "left",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {nodesRef.current.find((n) => n.id === pendingRelation.fromId)?.label ||
                    pendingRelation.fromId}
                </span>
                <span
                  style={{
                    color: "var(--gold-chalk, #f59e0b)",
                    fontFamily: "monospace",
                    fontWeight: 700,
                    padding: "0 4px",
                  }}
                >
                  ⟹
                </span>
                <span
                  style={{
                    fontWeight: 600,
                    color: "var(--chalk, #f8fafc)",
                    flex: 1,
                    textAlign: "right",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {nodesRef.current.find((n) => n.id === pendingRelation.toId)?.label ||
                    pendingRelation.toId}
                </span>
              </div>

              <div style={{ marginBottom: "12px" }}>
                <label
                  style={{
                    display: "block",
                    fontSize: "11px",
                    color: "var(--chalk-soft, #94a3b8)",
                    textTransform: "uppercase",
                    letterSpacing: "0.05em",
                    marginBottom: "6px",
                  }}
                >
                  Tipo de asociación lógica (LearnHarness)
                </label>
                <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "6px" }}>
                  {RELATION_CONNECTIVES.map((c) => (
                    <button
                      key={c.value}
                      type="button"
                      className={`chip ${relationLabel === c.value ? "is-active" : ""}`}
                      style={{
                        justifyContent: "flex-start",
                        gap: "6px",
                        padding: "6px 8px",
                        fontSize: "11px",
                      }}
                      onClick={() => setRelationLabel(c.value)}
                      title={c.description}
                    >
                      <span
                        style={{
                          fontFamily: "monospace",
                          color: "var(--gold-chalk, #fbbf24)",
                          fontWeight: 700,
                        }}
                      >
                        {c.glyph}
                      </span>
                      <span
                        style={{
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                        }}
                      >
                        {c.label}
                      </span>
                    </button>
                  ))}
                </div>
              </div>

              <div style={{ marginBottom: "14px" }}>
                <label
                  style={{
                    display: "block",
                    fontSize: "11px",
                    color: "var(--chalk-soft, #94a3b8)",
                    textTransform: "uppercase",
                    letterSpacing: "0.05em",
                    marginBottom: "4px",
                  }}
                >
                  O escribe con tus propias palabras
                </label>
                <input
                  autoFocus
                  style={{
                    width: "100%",
                    padding: "8px",
                    fontSize: "12px",
                    borderRadius: "6px",
                    background: "var(--input-bg, #0f172a)",
                    color: "var(--chalk, #f8fafc)",
                    border: "1px solid var(--border-soft, #334155)",
                    fontFamily: "monospace",
                    boxSizing: "border-box",
                  }}
                  value={relationLabel}
                  placeholder="Ej. deriva en, fundamenta a, se compone con…"
                  onChange={(e) => setRelationLabel(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && handleConfirmPendingRelation()}
                />
              </div>

              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "flex-end",
                  gap: "8px",
                  paddingTop: "8px",
                  borderTop: "1px solid var(--border-soft, #334155)",
                }}
              >
                <button
                  type="button"
                  className="btn-quiet"
                  style={{ fontSize: "12px", padding: "4px 12px" }}
                  onClick={() => setPendingRelation(null)}
                >
                  Cancelar
                </button>
                <button
                  type="button"
                  className="btn-primary"
                  style={{ fontSize: "12px", padding: "4px 14px" }}
                  disabled={!relationLabel.trim() || creating}
                  onClick={handleConfirmPendingRelation}
                >
                  {creating ? "Guardando…" : "Crear relación"}
                </button>
              </div>
            </div>
          </div>
        )}
      </div>
    );
  },
);
