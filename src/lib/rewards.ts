// Rewards for the notebook: physical pieces (Lego-style bricks, die-cut stickers)
// the student earns and keeps on a table beside the lesson. Pure model — no React,
// no physics — so the rules and the persistence can be checked on their own.
//
// A piece SAYS something: its tone and size record HOW it was earned (first try,
// second try, persistence), so the table is a record of the student's own path,
// not a random collection. Nothing here is graded or sent anywhere.

export type Tone = "gold" | "blue" | "green" | "red";
export type PieceShape = "brick" | "star" | "bolt" | "heart" | "flame";

export interface Piece {
  /** Stable per source (`gate:<blockId>`, `class:<classId>`), so passing twice never pays twice. */
  id: string;
  shape: PieceShape;
  tone: Tone;
  /** Studs across and rows high, for bricks (stickers are always 1x1). */
  cols: number;
  rows: number;
  /** One sentence: what the student did to earn it. Shown on hover. */
  why: string;
  earnedAt: number;
  /** Bricks: where it was left on the table — x as a fraction of the table width, y as px above the floor, angle in radians. */
  x?: number;
  y?: number;
  angle?: number;
  /** Stickers: where it was stuck on the notebook page. Without it, the sticker is still on the sheet. */
  page?: PagePlacement;
}

/** A sticker stuck on the page: x as a fraction of the page width, y in px from the top of the page, rotation in degrees. */
export interface PagePlacement {
  x: number;
  y: number;
  rot: number;
}

export const isSticker = (p: Pick<Piece, "shape">): boolean => p.shape !== "brick";
export const isBrick = (p: Pick<Piece, "shape">): boolean => p.shape === "brick";

/** Sticks a sticker on the page (or takes it back to the sheet with `null`). Bricks are left alone. */
export function withPage(pieces: readonly Piece[], id: string, page: PagePlacement | null): Piece[] {
  return pieces.map((p) => {
    if (p.id !== id || !isSticker(p)) return p;
    const { page: _old, ...rest } = p;
    return page ? { ...rest, page } : rest;
  });
}

export type RewardEvent =
  | { type: "gate_passed"; sourceId: string; attempts: number }
  | { type: "class_closed"; sourceId: string };

/** Chalk palette colours, as hex (the canvas can't read CSS variables): --gold/blue/green/red-chalk. */
export const TONE_COLOR: Record<Tone, string> = {
  gold: "#d9b168",
  blue: "#7fb0d8",
  green: "#7bc99a",
  red: "#e07864",
};

export const STICKER_SHAPES: readonly PieceShape[] = ["star", "bolt", "heart", "flame"];

/** One brick stud, in px. A 2x2 brick is 60 px wide. */
export const BRICK_UNIT = 30;
/** The most pieces one notebook keeps (the oldest go first) so the table never grinds to a halt. */
export const MAX_PIECES = 60;

/** The piece an event earns. `earnedCount` only picks which sticker comes up, so they cycle. */
export function pieceFor(event: RewardEvent, now: number, earnedCount: number): Piece {
  if (event.type === "class_closed") {
    return {
      id: `class:${event.sourceId}`,
      shape: STICKER_SHAPES[earnedCount % STICKER_SHAPES.length]!,
      tone: "gold",
      cols: 1,
      rows: 1,
      why: "Cerraste la clase.",
      earnedAt: now,
    };
  }
  const first = event.attempts <= 1;
  const second = event.attempts === 2;
  return {
    id: `gate:${event.sourceId}`,
    shape: "brick",
    tone: first ? "gold" : second ? "blue" : "green",
    cols: first ? 2 : second ? 2 : 1,
    rows: first ? 2 : 1,
    why: first
      ? "Superaste la compuerta a la primera."
      : second
        ? "Superaste la compuerta al segundo intento."
        : `Superaste la compuerta tras ${event.attempts} intentos: constancia.`,
    earnedAt: now,
  };
}

/** Adds a piece unless that source already paid; keeps the newest `MAX_PIECES`. */
export function addPiece(pieces: readonly Piece[], piece: Piece): Piece[] {
  if (pieces.some((p) => p.id === piece.id)) return [...pieces];
  return [...pieces, piece].slice(-MAX_PIECES);
}

/** The collision size of a piece in px. Stickers are round-ish: the same on both sides. */
export function pieceSize(p: Pick<Piece, "shape" | "cols" | "rows">): { w: number; h: number } {
  if (p.shape === "brick") return { w: p.cols * BRICK_UNIT, h: p.rows * BRICK_UNIT * 0.9 };
  return { w: BRICK_UNIT * 1.5, h: BRICK_UNIT * 1.5 };
}

/** Where a piece spawns: spread along the top, never two on the same spot. */
export function spawnX(index: number, width: number, pieceWidth: number): number {
  const usable = Math.max(1, width - pieceWidth * 2);
  // A golden-ratio walk spreads consecutive drops without looking regular.
  const t = (index * 0.618033988749895) % 1;
  return pieceWidth + t * usable;
}

export interface Placement {
  id: string;
  x: number;
  y: number;
  angle: number;
}

/** Remembers where each piece was left. Unknown ids are ignored. */
export function withPlacements(pieces: readonly Piece[], placements: readonly Placement[]): Piece[] {
  const byId = new Map(placements.map((p) => [p.id, p]));
  return pieces.map((p) => {
    const at = byId.get(p.id);
    return at ? { ...p, x: at.x, y: at.y, angle: at.angle } : p;
  });
}

// --- Persistence (per notebook, on this device) ------------------------------------------------

export interface KeyValueStore {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export const storageKey = (notebookId: string) => `learnkit.pieces.${notebookId}`;

const TONES: readonly string[] = ["gold", "blue", "green", "red"];
const SHAPES: readonly string[] = ["brick", "star", "bolt", "heart", "flame"];

function isPiece(v: unknown): v is Piece {
  if (typeof v !== "object" || v === null) return false;
  const p = v as Record<string, unknown>;
  const num = (x: unknown) => typeof x === "number" && Number.isFinite(x);
  return (
    typeof p.id === "string" &&
    typeof p.why === "string" &&
    typeof p.shape === "string" && SHAPES.includes(p.shape) &&
    typeof p.tone === "string" && TONES.includes(p.tone) &&
    num(p.cols) && (p.cols as number) >= 1 && (p.cols as number) <= 4 &&
    num(p.rows) && (p.rows as number) >= 1 && (p.rows as number) <= 4 &&
    num(p.earnedAt) &&
    (p.x === undefined || num(p.x)) &&
    (p.y === undefined || num(p.y)) &&
    (p.angle === undefined || num(p.angle)) &&
    (p.page === undefined ||
      (typeof p.page === "object" && p.page !== null && num((p.page as Record<string, unknown>).x) && num((p.page as Record<string, unknown>).y) && num((p.page as Record<string, unknown>).rot)))
  );
}

/** The saved pieces of a notebook; anything missing, corrupt or from a store that throws gives []. */
export function loadPieces(store: KeyValueStore | null, notebookId: string): Piece[] {
  try {
    const raw = store?.getItem(storageKey(notebookId));
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(isPiece).slice(-MAX_PIECES);
  } catch {
    return [];
  }
}

/** Best effort: the table still works for this session when storage is unavailable. */
export function savePieces(store: KeyValueStore | null, notebookId: string, pieces: readonly Piece[]): void {
  try {
    store?.setItem(storageKey(notebookId), JSON.stringify(pieces));
  } catch {
    /* private window, blocked site data, quota: ignore */
  }
}

export function browserStore(): KeyValueStore | null {
  try {
    return typeof window === "undefined" ? null : window.localStorage;
  } catch {
    return null;
  }
}

// --- Stickers on the page ---------------------------------------------------------------------------

/** Rendered size of a sticker stuck on the page, in px. */
export const STICKER_SIZE = 72;
const PAGE_MARGIN = 0.02;

/** Where a sticker dropped at (`clientX`, `clientY`) lands on a page whose box is `rect`: x as a fraction of
 *  the page width (kept inside it), y in px from the top of the page (so it scrolls with the content). */
export function toPagePlacement(clientX: number, clientY: number, rect: { left: number; top: number; width: number }, rot: number): PagePlacement {
  const x = rect.width > 0 ? (clientX - rect.left) / rect.width : 0.5;
  return {
    x: Math.round(Math.min(1 - PAGE_MARGIN, Math.max(PAGE_MARGIN, x)) * 1000) / 1000,
    y: Math.max(0, Math.round(clientY - rect.top)),
    rot: Math.round(rot * 10) / 10,
  };
}

/** A small, repeatable tilt for a sticker (-9 to 9 degrees) from its id, so stickers do not all sit straight. */
export function tiltFor(id: string): number {
  let h = 0;
  for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) | 0;
  return (Math.abs(h) % 19) - 9;
}
