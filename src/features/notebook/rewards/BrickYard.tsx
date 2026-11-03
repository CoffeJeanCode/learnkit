import Matter from "matter-js";
import { useEffect, useRef, useState } from "react";
import { BRICK_UNIT, type Piece, type Placement, pieceSize, spawnX } from "../../../lib/rewards";
import { sfx } from "../../../lib/sound";
import { drawPiece, drawSnapGhost } from "./drawPiece";
import {
  type AssembledBrick,
  BASEPLATE_HEIGHT,
  detectAssembledGridSlot,
  findSnapTarget,
  findSupportedBricks,
  getGridGeometry,
  isValidPlacement,
  buildOccupancyGrid,
  type SnapTarget,
} from "./legoSnap";

const { Bodies, Body, Composite, Engine, Events, Mouse, MouseConstraint, Query, Sleeping } = Matter;

/** How long everything must stay still before the pieces count as settled. */
const CALM_MS = 500;
/** Physics runs in fixed steps of this many ms, as many as real time requires (capped per frame). */
const STEP_MS = 1000 / 60;
const MAX_STEPS_PER_FRAME = 15;
/** A piece moving slower than this (px/frame, rad/frame) is at rest. */
const REST_SPEED = 0.2;
const REST_SPIN = 0.01;
/** Safety net: remember the layout this long after a drop even if something keeps jittering. */
const SETTLE_LIMIT_MS = 12000;

interface World {
  engine: Matter.Engine;
  canvas: HTMLCanvasElement;
  bodies: Map<string, Matter.Body>;
  walls: Matter.Body[];
  width: number;
  height: number;
  dpr: number;
  hover: string | null;
  dragged: Matter.Body | null;
  snapTarget: SnapTarget | null;
}

const reduced = () =>
  typeof window !== "undefined" &&
  typeof window.matchMedia === "function" &&
  window.matchMedia("(prefers-reduced-motion: reduce)").matches;

function makeWalls(w: number, h: number): Matter.Body[] {
  const t = 80;
  const floorTop = h - BASEPLATE_HEIGHT;
  const opts = { isStatic: true, friction: 0.8, label: "wall" };
  return [
    Bodies.rectangle(w / 2, floorTop + t / 2, w * 3, t, opts),
    Bodies.rectangle(-t / 2, h / 2, t, h * 4, opts),
    Bodies.rectangle(w + t / 2, h / 2, t, h * 4, opts),
  ];
}

function makeBody(piece: Piece, index: number, width: number, height: number): Matter.Body {
  const { w, h } = pieceSize(piece);
  const placed = piece.x !== undefined && piece.y !== undefined;
  const x = placed ? piece.x! * width : spawnX(index, width, w);
  const y = placed ? height - piece.y! : -h * 2;
  const opts = {
    restitution: 0.28,
    friction: 0.7,
    frictionAir: 0.012,
    density: 0.002,
    angle: placed ? (piece.angle ?? 0) : (Math.random() - 0.5) * 0.9,
    chamfer: { radius: 3 },
    label: piece.id,
  };
  const body = Bodies.rectangle(x, y, w, h, opts);
  body.plugin = { piece };
  if (!placed) Body.setVelocity(body, { x: (Math.random() - 0.5) * 3, y: 2 });
  return body;
}

/** Collects the list of assembled bricks and their grid positions. */
function getAssembledList(bodies: Map<string, Matter.Body>): AssembledBrick[] {
  const list: AssembledBrick[] = [];
  for (const b of bodies.values()) {
    if (b.plugin?.assembled && b.plugin?.cStart !== undefined && b.plugin?.layerBottom !== undefined) {
      const p = b.plugin.piece as Piece;
      list.push({
        id: p.id,
        cols: p.cols,
        rows: p.rows,
        cStart: b.plugin.cStart,
        layerBottom: b.plugin.layerBottom,
      });
    }
  }
  return list;
}

/** Unfreezes any assembled brick that lost its structural foundation down to the baseplate. */
function pruneUnsupported(w: World) {
  const assembled = getAssembledList(w.bodies);
  const supported = findSupportedBricks(assembled);
  for (const b of w.bodies.values()) {
    if (b.plugin?.assembled && !supported.has((b.plugin.piece as Piece).id)) {
      b.plugin.assembled = false;
      b.plugin.cStart = undefined;
      b.plugin.layerBottom = undefined;
      Body.setStatic(b, false);
      Sleeping.set(b, false);
    }
  }
}

/**
 * The table: a Lego-style workbench where the student's earned pieces fall, collide,
 * snap into stud-aligned structures, and can be assembled or disassembled freely.
 */
export function BrickYard({
  pieces,
  height = 230,
  onSettle,
  onHover,
}: {
  pieces: readonly Piece[];
  height?: number;
  onSettle: (placements: Placement[]) => void;
  onHover: (piece: Piece | null) => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  const [tooltip, setTooltip] = useState<{ text: string; x: number; y: number } | null>(null);
  const world = useRef<World | null>(null);
  const settleTimer = useRef(0);
  const settling = useRef(false);
  const latest = useRef({ pieces, onSettle, onHover });
  latest.current = { pieces, onSettle, onHover };

  const report = () => {
    const w = world.current;
    if (!w) return;
    const placements: Placement[] = [...w.bodies.entries()].map(([id, b]) => ({
      id,
      x: Math.round((b.position.x / w.width) * 1000) / 1000,
      y: Math.round(w.height - b.position.y),
      angle: Math.round(b.angle * 1000) / 1000,
    }));
    latest.current.onSettle(placements);
  };

  /** Creates missing bodies and restores previously assembled LEGO positions. */
  const sync = () => {
    const w = world.current;
    if (!w) return;
    let fresh = 0;
    latest.current.pieces.forEach((piece, i) => {
      if (w.bodies.has(piece.id)) return;
      const body = makeBody(piece, i, w.width, w.height);
      w.bodies.set(piece.id, body);
      Composite.add(w.engine.world, body);

      // If loaded from saved placement, check if it was assembled on the stud grid
      if (piece.x !== undefined && piece.y !== undefined) {
        const slot = detectAssembledGridSlot(body.position.x, body.position.y, body.angle, piece, w.width, w.height);
        if (slot) {
          Body.setPosition(body, { x: slot.snapX, y: slot.snapY });
          Body.setAngle(body, 0);
          Body.setVelocity(body, { x: 0, y: 0 });
          Body.setAngularVelocity(body, 0);
          Body.setStatic(body, true);
          body.plugin.assembled = true;
          body.plugin.cStart = slot.cStart;
          body.plugin.layerBottom = slot.layerBottom;
        }
      } else {
        fresh++;
      }
    });

    pruneUnsupported(w);

    if (fresh === 0) return;
    if (reduced()) for (let i = 0; i < 360; i++) Engine.update(w.engine, 1000 / 60);
    settling.current = true;
    window.clearTimeout(settleTimer.current);
    settleTimer.current = window.setTimeout(() => {
      if (!settling.current) return;
      settling.current = false;
      report();
    }, SETTLE_LIMIT_MS);
  };

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const canvas = document.createElement("canvas");
    canvas.className = "yard-canvas";
    el.appendChild(canvas);
    const ctx = canvas.getContext("2d");
    if (!ctx) {
      canvas.remove();
      return;
    }

    const engine = Engine.create({ gravity: { x: 0, y: 1.1, scale: 0.0012 }, enableSleeping: true });
    const w: World = {
      engine,
      canvas,
      bodies: new Map(),
      walls: [],
      width: 0,
      height,
      dpr: 1,
      hover: null,
      dragged: null,
      snapTarget: null,
    };
    world.current = w;

    const mouse = Mouse.create(canvas);
    canvas.removeEventListener("mousewheel", (mouse as unknown as { mousewheel: EventListener }).mousewheel);
    canvas.removeEventListener("DOMMouseScroll", (mouse as unknown as { mousewheel: EventListener }).mousewheel);
    const drag = MouseConstraint.create(engine, { mouse, constraint: { stiffness: 0.25, damping: 0.1, render: { visible: false } } });
    Composite.add(engine.world, drag);

    const resize = () => {
      const width = Math.max(120, Math.floor(el.clientWidth));
      w.dpr = Math.min(window.devicePixelRatio || 1, 2);
      canvas.width = Math.round(width * w.dpr);
      canvas.height = Math.round(height * w.dpr);
      canvas.style.width = `${width}px`;
      canvas.style.height = `${height}px`;
      mouse.pixelRatio = w.dpr;
      const widthChanged = width !== w.width;
      w.width = width;
      w.height = height;
      if (widthChanged) {
        Composite.remove(engine.world, w.walls);
        w.walls = makeWalls(width, height);
        Composite.add(engine.world, w.walls);

        // Keep assembled bricks aligned to updated gridOriginX
        const { gridOriginX } = getGridGeometry(width, height);
        for (const b of w.bodies.values()) {
          if (b.plugin?.assembled && b.plugin?.cStart !== undefined) {
            const p = b.plugin.piece as Piece;
            const newX = gridOriginX + (b.plugin.cStart + p.cols / 2) * BRICK_UNIT;
            Body.setPosition(b, { x: newX, y: b.position.y });
          }
        }
      }
    };
    resize();
    const ro = typeof ResizeObserver !== "undefined" ? new ResizeObserver(resize) : null;
    ro?.observe(el);

    // Pointer down: detach and wake up assembled piece immediately so drag is crisp
    const onPointerDown = (evt: PointerEvent) => {
      const rect = canvas.getBoundingClientRect();
      const pos = {
        x: evt.clientX - rect.left,
        y: evt.clientY - rect.top,
      };
      const hit = Query.point([...w.bodies.values()], pos)[0];
      if (hit && (hit.isStatic || hit.plugin?.assembled)) {
        hit.plugin.assembled = false;
        hit.plugin.cStart = undefined;
        hit.plugin.layerBottom = undefined;
        Body.setStatic(hit, false);
        Sleeping.set(hit, false);
        pruneUnsupported(w);
      }
    };
    canvas.addEventListener("pointerdown", onPointerDown, { passive: true });

    // Collisions: plastic on plastic clacks
    Events.on(engine, "collisionStart", (event) => {
      for (const pair of event.pairs) {
        const [a, b] = [pair.bodyA, pair.bodyB];
        const speed = Math.hypot(a.velocity.x - b.velocity.x, a.velocity.y - b.velocity.y);
        const piece = (a.isStatic ? b : a).plugin?.piece as Piece | undefined;
        if (piece) sfx.clack(speed, pieceSize(piece).w);
      }
    });

    Events.on(drag, "startdrag", () => {
      setTooltip(null);
      sfx.pick();
      const b = drag.body;
      if (!b) return;
      w.dragged = b;
      canvas.style.cursor = "grabbing";

      if (b.isStatic || b.plugin?.assembled) {
        b.plugin.assembled = false;
        b.plugin.cStart = undefined;
        b.plugin.layerBottom = undefined;
        Body.setStatic(b, false);
        Sleeping.set(b, false);
        pruneUnsupported(w);
      }
    });

    Events.on(drag, "enddrag", () => {
      const b = w.dragged;
      if (b) {
        if (w.snapTarget) {
          // Snap locked onto studs
          Body.setPosition(b, { x: w.snapTarget.x, y: w.snapTarget.y });
          Body.setVelocity(b, { x: 0, y: 0 });
          Body.setAngularVelocity(b, 0);
          Body.setAngle(b, 0);
          Body.setStatic(b, true);
          b.plugin.assembled = true;
          b.plugin.cStart = w.snapTarget.cStart;
          b.plugin.layerBottom = w.snapTarget.layerBottom;
          const piece = b.plugin.piece as Piece;
          sfx.clack(14, pieceSize(piece).w);
        } else {
          // Free loose drop
          Body.setStatic(b, false);
        }
      }
      w.dragged = null;
      w.snapTarget = null;
      canvas.style.cursor = w.hover ? "grab" : "default";
      report();
    });

    Events.on(drag, "mousemove", () => {
      if (w.dragged) {
        setTooltip(null);
        return;
      }
      const hit = Query.point([...w.bodies.values()], mouse.position)[0];
      const id = hit ? (hit.plugin.piece as Piece).id : null;
      if (id !== w.hover) {
        w.hover = id;
        canvas.style.cursor = id ? "grab" : "default";
        latest.current.onHover(id ? (hit!.plugin.piece as Piece) : null);
      }
      if (hit) {
        const piece = hit.plugin.piece as Piece;
        setTooltip({
          text: piece.why,
          x: hit.position.x,
          y: Math.max(25, hit.position.y - 25),
        });
      } else {
        setTooltip(null);
      }
    });

    sync();

    let raf = 0;
    let calm = 0;
    let last = performance.now();
    let owed = 0;

    const frame = (now: number) => {
      raf = requestAnimationFrame(frame);
      const elapsed = Math.min(now - last, 250);
      last = now;
      if (document.hidden) return;

      owed += elapsed;
      for (let n = 0; owed >= STEP_MS && n < MAX_STEPS_PER_FRAME; n++) {
        Engine.update(engine, STEP_MS);
        owed -= STEP_MS;
      }
      owed = Math.min(owed, STEP_MS);

      // Check magnetic snap target while dragging
      if (w.dragged) {
        const b = w.dragged;
        const piece = b.plugin.piece as Piece;
        const assembled = getAssembledList(w.bodies);
        const target = findSnapTarget(
          b.position.x,
          b.position.y,
          piece,
          w.width,
          w.height,
          assembled,
          piece.id
        );
        w.snapTarget = target;
        if (target) {
          // Magnetic alignment: level the brick horizontally as it enters snap zone
          Body.setAngle(b, b.angle * 0.75);
          Body.setAngularVelocity(b, 0);
        }
      }

      if (settling.current && !w.dragged) {
        let still = true;
        for (const b of w.bodies.values()) {
          if (!b.isSleeping && (b.speed > REST_SPEED || Math.abs(b.angularSpeed) > REST_SPIN)) {
            still = false;
            break;
          }
        }
        calm = still ? calm + elapsed : 0;
        if (calm >= CALM_MS) {
          settling.current = false;
          calm = 0;
          window.clearTimeout(settleTimer.current);

          // Auto-lock bricks that settled flat on studs
          for (const b of w.bodies.values()) {
            if (!b.isStatic && !b.plugin?.assembled) {
              const piece = b.plugin.piece as Piece;
              const slot = detectAssembledGridSlot(b.position.x, b.position.y, b.angle, piece, w.width, w.height, 8);
              if (slot) {
                const assembled = getAssembledList(w.bodies);
                const { numCols: colsCount } = getGridGeometry(w.width, w.height);
                const grid = buildOccupancyGrid(colsCount, assembled, piece.id);
                if (isValidPlacement(piece.cols, piece.rows, slot.cStart, slot.layerBottom, colsCount, grid)) {
                  Body.setPosition(b, { x: slot.snapX, y: slot.snapY });
                  Body.setVelocity(b, { x: 0, y: 0 });
                  Body.setAngularVelocity(b, 0);
                  Body.setAngle(b, 0);
                  Body.setStatic(b, true);
                  b.plugin.assembled = true;
                  b.plugin.cStart = slot.cStart;
                  b.plugin.layerBottom = slot.layerBottom;
                }
              }
            }
          }

          report();
        }
      }

      // Safety: return escaped pieces
      for (const b of w.bodies.values()) {
        if (b.position.y > w.height + 160 || b.position.x < -160 || b.position.x > w.width + 160) {
          Body.setPosition(b, { x: w.width / 2, y: -40 });
          Body.setVelocity(b, { x: 0, y: 0 });
        }
      }

      ctx.setTransform(w.dpr, 0, 0, w.dpr, 0, 0);
      ctx.clearRect(0, 0, w.width, w.height);

      const { numCols, gridOriginX, floorTop } = getGridGeometry(w.width, w.height);

      // Baseplate foundation strip
      ctx.fillStyle = "#14171f";
      ctx.fillRect(0, floorTop, w.width, BASEPLATE_HEIGHT);
      ctx.fillStyle = "#262e3d";
      ctx.fillRect(0, floorTop, w.width, 1.5);

      // Baseplate studs aligned to 30px grid
      const studW = BRICK_UNIT * 0.5;
      const studH = 5;
      for (let col = 0; col < numCols; col++) {
        const cx = gridOriginX + (col + 0.5) * BRICK_UNIT;
        // Stud base
        ctx.fillStyle = "#222a38";
        ctx.beginPath();
        ctx.roundRect(cx - studW / 2, floorTop - studH, studW, studH + 2, 2);
        ctx.fill();
        // Lit top of stud
        ctx.fillStyle = "#333f52";
        ctx.fillRect(cx - studW / 2 + 1, floorTop - studH, studW - 2, 1.5);
      }

      // Snap Ghost Preview
      if (w.dragged && w.snapTarget) {
        ctx.save();
        ctx.translate(w.snapTarget.x, w.snapTarget.y);
        drawSnapGhost(ctx, w.dragged.plugin.piece as Piece);
        ctx.restore();
      }

      // Bricks rendering order: static/assembled first (bottom to top), dynamic, then dragged on top
      const bodiesToDraw = [...w.bodies.values()].filter((b) => b !== w.dragged);
      bodiesToDraw.sort((a, b) => {
        const aStatic = a.isStatic ? 1 : 0;
        const bStatic = b.isStatic ? 1 : 0;
        if (aStatic !== bStatic) return bStatic - aStatic;
        return b.position.y - a.position.y;
      });

      for (const b of bodiesToDraw) {
        ctx.save();
        ctx.translate(b.position.x, b.position.y);
        ctx.rotate(b.angle);
        drawPiece(ctx, b.plugin.piece as Piece, false);
        ctx.restore();
      }

      if (w.dragged) {
        ctx.save();
        ctx.translate(w.dragged.position.x, w.dragged.position.y);
        ctx.rotate(w.dragged.angle);
        drawPiece(ctx, w.dragged.plugin.piece as Piece, true);
        ctx.restore();
      }
    };

    raf = requestAnimationFrame(frame);

    return () => {
      cancelAnimationFrame(raf);
      window.clearTimeout(settleTimer.current);
      report();
      ro?.disconnect();
      canvas.removeEventListener("pointerdown", onPointerDown);
      Events.off(engine, "collisionStart");
      Events.off(drag, "startdrag");
      Events.off(drag, "enddrag");
      Events.off(drag, "mousemove");
      Composite.clear(engine.world, false);
      Engine.clear(engine);
      canvas.remove();
      world.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [height]);

  useEffect(() => {
    sync();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pieces]);

  return (
    <div
      ref={host}
      className="yard"
      role="img"
      aria-label={`Tu mesa: ${pieces.length} ${pieces.length === 1 ? "pieza" : "piezas"}`}
      onPointerLeave={() => setTooltip(null)}
    >
      {tooltip && (
        <div
          className="brick-tooltip"
          style={{
            position: "absolute",
            left: `${tooltip.x}px`,
            top: `${tooltip.y}px`,
            transform: "translate(-50%, -100%)",
            pointerEvents: "none",
            zIndex: 40,
          }}
        >
          <div className="brick-tooltip-bubble">
            {tooltip.text}
          </div>
        </div>
      )}
    </div>
  );
}
