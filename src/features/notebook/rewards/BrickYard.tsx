import Matter from "matter-js";
import { useEffect, useRef } from "react";
import { type Piece, type Placement, pieceSize, spawnX } from "../../../lib/rewards";
import { sfx } from "../../../lib/sound";
import { drawPiece } from "./drawPiece";

const { Bodies, Body, Composite, Engine, Events, Mouse, MouseConstraint, Query } = Matter;

/** Height of the baseplate the pieces rest on, in px. */
const PLATE = 12;
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
}

const reduced = () => typeof window !== "undefined" && typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;

function makeWalls(w: number, h: number): Matter.Body[] {
  const t = 80;
  const floorTop = h - PLATE;
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
  const body = piece.shape === "brick" ? Bodies.rectangle(x, y, w, h, opts) : Bodies.circle(x, y, w * 0.4, opts);
  body.plugin = { piece };
  if (!placed) Body.setVelocity(body, { x: (Math.random() - 0.5) * 3, y: 2 });
  return body;
}

/**
 * The table: a small physics world where the student's pieces fall, collide,
 * stack and can be picked up with the mouse or a finger. Where each piece was
 * left is reported through `onSettle` (after a drag, and after a new piece
 * lands) so the arrangement is the notebook's own, and `onHover` says which
 * piece is under the pointer so the table can tell how it was earned.
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
  const world = useRef<World | null>(null);
  const settleTimer = useRef(0);
  // Set when a new piece drops: the layout is remembered once everything has come to rest.
  const settling = useRef(false);
  // The callbacks and pieces change identity every render; the loop reads the latest through refs.
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

  /** Creates the missing bodies; pieces that were left somewhere come back there, new ones fall in. */
  const sync = () => {
    const w = world.current;
    if (!w) return;
    let fresh = 0;
    latest.current.pieces.forEach((piece, i) => {
      if (w.bodies.has(piece.id)) return;
      const body = makeBody(piece, i, w.width, w.height);
      w.bodies.set(piece.id, body);
      Composite.add(w.engine.world, body);
      if (piece.x === undefined) fresh++;
    });
    if (fresh === 0) return;
    // Reduced motion: no falling to watch, so let it land off-screen and show the result.
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

    // Sleeping: a piece that has stopped goes to sleep (no jitter, no wasted work) and wakes when touched.
    const engine = Engine.create({ gravity: { x: 0, y: 1.1, scale: 0.0012 }, enableSleeping: true });
    const w: World = { engine, canvas, bodies: new Map(), walls: [], width: 0, height, dpr: 1, hover: null, dragged: null };
    world.current = w;

    const mouse = Mouse.create(canvas);
    // Let the page scroll over the table: Matter swallows the wheel by default.
    canvas.removeEventListener("mousewheel", (mouse as unknown as { mousewheel: EventListener }).mousewheel);
    canvas.removeEventListener("DOMMouseScroll", (mouse as unknown as { mousewheel: EventListener }).mousewheel);
    const drag = MouseConstraint.create(engine, { mouse, constraint: { stiffness: 0.2, damping: 0.1, render: { visible: false } } });
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
      }
    };
    resize();
    const ro = typeof ResizeObserver !== "undefined" ? new ResizeObserver(resize) : null;
    ro?.observe(el);

    // Plastic on plastic: every impact clacks, as loud as it hits and as low as the piece is big.
    Events.on(engine, "collisionStart", (event) => {
      for (const pair of event.pairs) {
        const [a, b] = [pair.bodyA, pair.bodyB];
        const speed = Math.hypot(a.velocity.x - b.velocity.x, a.velocity.y - b.velocity.y);
        const piece = (a.isStatic ? b : a).plugin?.piece as Piece | undefined;
        if (piece) sfx.clack(speed, pieceSize(piece).w);
      }
    });
    Events.on(drag, "startdrag", () => {
      sfx.pick();
      w.dragged = drag.body;
      canvas.style.cursor = "grabbing";
    });
    Events.on(drag, "enddrag", () => {
      w.dragged = null;
      canvas.style.cursor = w.hover ? "grab" : "default";
      report();
    });
    Events.on(drag, "mousemove", () => {
      if (w.dragged) return;
      const hit = Query.point([...w.bodies.values()], mouse.position)[0];
      const id = hit ? (hit.plugin.piece as Piece).id : null;
      if (id === w.hover) return;
      w.hover = id;
      canvas.style.cursor = id ? "grab" : "default";
      latest.current.onHover(id ? (hit!.plugin.piece as Piece) : null);
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
      // Real-time physics: a slow or throttled frame owes extra fixed steps, so the pieces
      // fall at the same speed on any machine (capped, so a stall never spirals).
      owed += elapsed;
      for (let n = 0; owed >= STEP_MS && n < MAX_STEPS_PER_FRAME; n++) {
        Engine.update(engine, STEP_MS);
        owed -= STEP_MS;
      }
      owed = Math.min(owed, STEP_MS);
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
          report();
        }
      }
      // A piece that escaped (a hard throw) comes back from the top instead of being lost.
      for (const b of w.bodies.values()) {
        if (b.position.y > w.height + 160 || b.position.x < -160 || b.position.x > w.width + 160) {
          Body.setPosition(b, { x: w.width / 2, y: -40 });
          Body.setVelocity(b, { x: 0, y: 0 });
        }
      }
      ctx.setTransform(w.dpr, 0, 0, w.dpr, 0, 0);
      ctx.clearRect(0, 0, w.width, w.height);
      // The baseplate: a dark strip with a row of studs, like the plate bricks click onto.
      ctx.fillStyle = "#14171b";
      ctx.fillRect(0, w.height - PLATE, w.width, PLATE);
      ctx.fillStyle = "#242a33";
      for (let x = 10; x < w.width; x += 22) {
        ctx.beginPath();
        ctx.roundRect(x, w.height - PLATE - 2, 12, 4, 2);
        ctx.fill();
      }
      for (const b of w.bodies.values()) {
        ctx.save();
        ctx.translate(b.position.x, b.position.y);
        ctx.rotate(b.angle);
        drawPiece(ctx, b.plugin.piece as Piece, b === w.dragged);
        ctx.restore();
      }
    };
    raf = requestAnimationFrame(frame);

    return () => {
      cancelAnimationFrame(raf);
      window.clearTimeout(settleTimer.current);
      report();
      ro?.disconnect();
      Events.off(engine, "collisionStart");
      Events.off(drag, "startdrag");
      Events.off(drag, "enddrag");
      Events.off(drag, "mousemove");
      Composite.clear(engine.world, false);
      Engine.clear(engine);
      canvas.remove();
      world.current = null;
    };
    // The world lives as long as the panel; pieces are synced by the effect below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [height]);

  useEffect(() => {
    sync();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pieces]);

  return <div ref={host} className="yard" role="img" aria-label={`Tu mesa: ${pieces.length} ${pieces.length === 1 ? "pieza" : "piezas"}`} />;
}
