// The "peel" of a sticker, after Sticker.js: bring the pointer to an edge and that edge lifts like a
// real sticker, showing the paper back, a shadow in the fold, and settling back with an ease-out on
// leave. This module is only the geometry (pure, in a 0..SIZE box) so it can be checked on its own;
// `PeelSticker` draws it as SVG, which lets the peel follow each sticker's die-cut silhouette.

export const SIZE = 100;
/** The most of the sticker that can lift (a fraction of its size). */
export const MAX_PEEL = 0.8;
/** The pointer has to travel this far in from the edge before it starts to lift (SIZE units). */
export const DEAD_ZONE = 6;
/** Leaving: the flap settles back over this long, with an ease-out (Sticker.js uses .6 s). */
export const SETTLE_MS = 600;

export type Edge = "left" | "right" | "top" | "bottom";

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Gradient {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

/** How wide the cast shadow beside the fold is (SIZE units). */
export const CAST = 14;

export interface Peel {
  edge: Edge;
  /** Where the sticker folds, on the axis of the edge (SIZE units). */
  fold: number;
  /** The part that stays flat. */
  flat: Rect;
  /** The part that lifts, in the sticker's own (unflipped) coordinates. */
  lifted: Rect;
  /** SVG transform that mirrors the lifted part over the fold, to draw the back. */
  mirror: string;
  /** Gradient across the lifted back (in the lifted part's own coordinates): darkest at the fold, clear at the tip. */
  shade: Gradient;
  /** The shadow the flap casts on the flat part: a strip beside the fold, darkest at the fold. */
  cast: { rect: Rect } & Gradient;
  /** Depth of the lift on its axis (SIZE units); 0 means flat. */
  depth: number;
}

/** The edge nearest to a point (SIZE units): the one that peels when the pointer arrives there. */
export function nearestEdge(x: number, y: number): Edge {
  const d: [Edge, number][] = [
    ["left", x],
    ["right", SIZE - x],
    ["top", y],
    ["bottom", SIZE - y],
  ];
  return d.reduce((best, cur) => (cur[1] < best[1] ? cur : best))[0];
}

/** How far the pointer is in from `edge`, minus the dead zone, clamped to what can lift. */
export function peelDepth(edge: Edge, x: number, y: number): number {
  const inward = edge === "left" ? x : edge === "right" ? SIZE - x : edge === "top" ? y : SIZE - y;
  return Math.min(SIZE * MAX_PEEL, Math.max(0, inward - DEAD_ZONE));
}

/** The geometry of an `edge` lifted by `depth`. The fold sits halfway between the edge and the
 *  pointer, and what lifts reflects over it, so the flap ends where the pointer is. */
export function peelGeometry(edge: Edge, depth: number): Peel {
  const d = Math.min(SIZE * MAX_PEEL, Math.max(0, depth));
  const c = d / 2;
  switch (edge) {
    case "left":
      return {
        edge,
        fold: c,
        flat: { x: c, y: 0, w: SIZE - c, h: SIZE },
        lifted: { x: 0, y: 0, w: c, h: SIZE },
        mirror: `translate(${2 * c} 0) scale(-1 1)`,
        shade: { x1: c, y1: 0, x2: 0, y2: 0 },
        cast: { rect: { x: c, y: 0, w: CAST, h: SIZE }, x1: c, y1: 0, x2: c + CAST, y2: 0 },
        depth: d,
      };
    case "right": {
      const fold = SIZE - c;
      return {
        edge,
        fold,
        flat: { x: 0, y: 0, w: fold, h: SIZE },
        lifted: { x: fold, y: 0, w: c, h: SIZE },
        mirror: `translate(${2 * fold} 0) scale(-1 1)`,
        shade: { x1: fold, y1: 0, x2: SIZE, y2: 0 },
        cast: { rect: { x: fold - CAST, y: 0, w: CAST, h: SIZE }, x1: fold, y1: 0, x2: fold - CAST, y2: 0 },
        depth: d,
      };
    }
    case "top":
      return {
        edge,
        fold: c,
        flat: { x: 0, y: c, w: SIZE, h: SIZE - c },
        lifted: { x: 0, y: 0, w: SIZE, h: c },
        mirror: `translate(0 ${2 * c}) scale(1 -1)`,
        shade: { x1: 0, y1: c, x2: 0, y2: 0 },
        cast: { rect: { x: 0, y: c, w: SIZE, h: CAST }, x1: 0, y1: c, x2: 0, y2: c + CAST },
        depth: d,
      };
    case "bottom": {
      const fold = SIZE - c;
      return {
        edge,
        fold,
        flat: { x: 0, y: 0, w: SIZE, h: fold },
        lifted: { x: 0, y: fold, w: SIZE, h: c },
        mirror: `translate(0 ${2 * fold}) scale(1 -1)`,
        shade: { x1: 0, y1: fold, x2: 0, y2: SIZE },
        cast: { rect: { x: 0, y: fold - CAST, w: SIZE, h: CAST }, x1: 0, y1: fold, x2: 0, y2: fold - CAST },
        depth: d,
      };
    }
  }
}

/** Ease-out used for the settle (a steep quint, close to Sticker.js's cubic-bezier(.23, 1, .32, 1)). */
export function easeOut(t: number): number {
  const u = 1 - Math.min(1, Math.max(0, t));
  return 1 - u ** 5;
}

/** The depth part-way through settling back from `from`, `elapsed` ms after the pointer left. */
export function settleDepth(from: number, elapsed: number): number {
  return from * (1 - easeOut(elapsed / SETTLE_MS));
}
