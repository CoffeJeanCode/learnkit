import { useEffect, useId, useRef, useState } from "react";
import { type Edge, SIZE, nearestEdge, peelDepth, peelGeometry, settleDepth } from "../../../lib/peel";
import { type PieceShape, TONE_COLOR, type Tone } from "../../../lib/rewards";
import { sfx } from "../../../lib/sound";
import { STICKER_PATHS } from "./drawPiece";

const SCALE = SIZE / 24;
const PAPER = "#ece7da";
const BORDER = "#f3efe4";

const reduced = () => typeof window !== "undefined" && typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * A die-cut sticker that peels, after Sticker.js: bring the pointer to an edge and that
 * edge lifts, showing the paper back (the art faintly through it), a shadow in the fold and
 * the shadow the flap casts; leaving lets it settle back with an ease-out. Drawn as SVG so
 * the peel follows the sticker's own silhouette instead of a square.
 */
export function PeelSticker({ shape, tone, size = 64, label }: { shape: PieceShape; tone: Tone; size?: number; label?: string }) {
  const uid = useId().replace(/[^a-zA-Z0-9]/g, "");
  const svg = useRef<SVGSVGElement>(null);
  const [peel, setPeel] = useState<{ edge: Edge; depth: number }>({ edge: "left", depth: 0 });
  const live = useRef({ edge: "left" as Edge, depth: 0, hovering: false, raf: 0, voiced: false });

  useEffect(() => () => cancelAnimationFrame(live.current.raf), []);

  const local = (e: React.PointerEvent) => {
    const r = svg.current!.getBoundingClientRect();
    return { x: ((e.clientX - r.left) / r.width) * SIZE, y: ((e.clientY - r.top) / r.height) * SIZE };
  };
  const set = (edge: Edge, depth: number) => {
    live.current.edge = edge;
    live.current.depth = depth;
    setPeel({ edge, depth });
  };

  const onEnter = (e: React.PointerEvent) => {
    cancelAnimationFrame(live.current.raf);
    const { x, y } = local(e);
    live.current.hovering = true;
    live.current.voiced = false;
    // The edge the pointer arrives at is the one that lifts, and it stays that one until it leaves.
    set(nearestEdge(x, y), 0);
  };
  const onMove = (e: React.PointerEvent) => {
    if (!live.current.hovering) return;
    const { x, y } = local(e);
    const depth = peelDepth(live.current.edge, x, y);
    // The paper sound plays once per peel, as it first lifts.
    if (!live.current.voiced && depth > 8) {
      live.current.voiced = true;
      sfx.peel();
    }
    set(live.current.edge, depth);
  };
  const onLeave = () => {
    live.current.hovering = false;
    const from = live.current.depth;
    if (from <= 0) return;
    if (reduced()) {
      set(live.current.edge, 0);
      return;
    }
    const t0 = performance.now();
    const step = (now: number) => {
      const depth = settleDepth(from, now - t0);
      set(live.current.edge, depth < 0.2 ? 0 : depth);
      if (depth >= 0.2 && !live.current.hovering) live.current.raf = requestAnimationFrame(step);
    };
    live.current.raf = requestAnimationFrame(step);
  };

  const g = peelGeometry(peel.edge, peel.depth);
  const path = STICKER_PATHS[shape] ?? STICKER_PATHS.star!;
  const fill = TONE_COLOR[tone];
  const art = (
    <g transform={`scale(${SCALE})`}>
      <path d={path} fill="none" stroke={BORDER} strokeWidth={3.4} strokeLinejoin="round" />
      <path d={path} fill={fill} />
      <g clipPath={`url(#${uid}-sil24)`}>
        <rect x={0} y={0} width={24} height={9} fill="rgba(255,255,255,0.28)" />
      </g>
    </g>
  );
  const gradient = (id: string, k: { x1: number; y1: number; x2: number; y2: number }, from: number) => (
    <linearGradient id={`${uid}-${id}`} gradientUnits="userSpaceOnUse" x1={k.x1} y1={k.y1} x2={k.x2} y2={k.y2}>
      <stop offset="0" stopColor="#000" stopOpacity={from} />
      <stop offset="0.04" stopColor="#000" stopOpacity={from * 0.9} />
      <stop offset="1" stopColor="#000" stopOpacity={0} />
    </linearGradient>
  );
  const lifted = peel.depth > 0.1;

  return (
    <svg
      ref={svg}
      className="peel-sticker"
      viewBox={`0 0 ${SIZE} ${SIZE}`}
      width={size}
      height={size}
      role="img"
      aria-label={label ?? "Sticker"}
      onPointerEnter={onEnter}
      onPointerMove={onMove}
      onPointerLeave={onLeave}
    >
      <defs>
        <clipPath id={`${uid}-sil24`}>
          <path d={path} />
        </clipPath>
        <clipPath id={`${uid}-sil`}>
          <path d={path} transform={`scale(${SCALE})`} />
        </clipPath>
        <clipPath id={`${uid}-flat`}>
          <rect x={g.flat.x} y={g.flat.y} width={g.flat.w} height={g.flat.h} />
        </clipPath>
        <clipPath id={`${uid}-lift`}>
          <rect x={g.lifted.x} y={g.lifted.y} width={g.lifted.w} height={g.lifted.h} />
        </clipPath>
        {gradient("shade", g.shade, 0.62)}
        {gradient("cast", g.cast, 0.4)}
      </defs>

      {/* The part that stays stuck, with the shadow the flap casts beside the fold. */}
      <g clipPath={`url(#${uid}-flat)`}>
        {art}
        {lifted && (
          <g clipPath={`url(#${uid}-sil)`}>
            <rect x={g.cast.rect.x} y={g.cast.rect.y} width={g.cast.rect.w} height={g.cast.rect.h} fill={`url(#${uid}-cast)`} />
          </g>
        )}
      </g>

      {/* The lifted flap, folded over: the paper back with the art showing faintly through it. */}
      {lifted && (
        <g transform={g.mirror}>
          <g clipPath={`url(#${uid}-lift)`}>
            <g transform={`scale(${SCALE})`}>
              <path d={path} fill={PAPER} stroke={PAPER} strokeWidth={3.4} strokeLinejoin="round" />
              <path d={path} fill={fill} opacity={0.38} />
            </g>
            <g clipPath={`url(#${uid}-sil)`}>
              <rect x={g.lifted.x} y={g.lifted.y} width={g.lifted.w} height={g.lifted.h} fill={`url(#${uid}-shade)`} />
            </g>
          </g>
        </g>
      )}
    </svg>
  );
}
