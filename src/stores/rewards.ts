import { create } from "zustand";
import { sfx } from "../lib/sound";
import {
  type Piece,
  type PagePlacement,
  type Placement,
  type RewardEvent,
  addPiece,
  browserStore,
  loadAllEarnedPieces,
  loadPieces,
  pieceFor,
  savePieces,
  withPage,
  withPlacements,
} from "../lib/rewards";

interface RewardsState {
  /** Pieces per notebook, loaded from this device the first time a notebook is seen. */
  byNotebook: Record<string, Piece[]>;
  /** Whether the table is open. */
  open: boolean;
  /** The piece just earned, so the table can say why it appeared. Cleared when closed. */
  lastEarned: Piece | null;
  load: (notebookId: string) => void;
  earn: (notebookId: string, event: RewardEvent) => void;
  place: (notebookId: string, placements: readonly Placement[]) => void;
  /** Sticks a sticker on the notebook page (or `null` to put it back on the sheet). */
  placeSticker: (notebookId: string, id: string, page: PagePlacement | null) => void;
  setOpen: (open: boolean) => void;
}

export const useRewards = create<RewardsState>((set, get) => ({
  byNotebook: {},
  open: false,
  lastEarned: null,

  load: (notebookId) => {
    const store = browserStore();
    const stored = loadPieces(store, notebookId);
    const inMemory = get().byNotebook[notebookId] ?? [];
    const existing = stored.length > 0 ? stored : inMemory;
    // Also collect all unplaced stickers earned anywhere across the student's notebook sessions
    const allPieces = loadAllEarnedPieces(store);
    const unplacedStickers = allPieces.filter((p) => p.shape !== "brick" && !p.page);

    const existingIds = new Set(existing.map((p) => p.id));
    const merged = [...existing];
    for (const sticker of unplacedStickers) {
      if (!existingIds.has(sticker.id)) {
        merged.push(sticker);
        existingIds.add(sticker.id);
      }
    }

    set((s) => ({ byNotebook: { ...s.byNotebook, [notebookId]: merged } }));
  },

  earn: (notebookId, event) => {
    get().load(notebookId);
    const current = get().byNotebook[notebookId] ?? [];
    const stickers = current.filter((p) => p.shape !== "brick").length;
    const piece = pieceFor(event, Date.now(), stickers);
    const next = addPiece(current, piece);
    // The same source never pays twice (re-passing a gate after a reload, say).
    if (next.length === current.length && current.some((p) => p.id === piece.id)) return;
    savePieces(browserStore(), notebookId, next);
    set((s) => ({ byNotebook: { ...s.byNotebook, [notebookId]: next }, open: true, lastEarned: piece }));
    // The chord says how it was earned: more notes, the sooner (see `earnNotes`).
    sfx.earn(piece);
  },

  place: (notebookId, placements) => {
    const next = withPlacements(get().byNotebook[notebookId] ?? [], placements);
    savePieces(browserStore(), notebookId, next);
    set((s) => ({ byNotebook: { ...s.byNotebook, [notebookId]: next } }));
  },

  placeSticker: (notebookId, id, page) => {
    const store = browserStore();
    const current = get().byNotebook[notebookId] ?? [];
    const next = withPage(current, id, page);
    savePieces(store, notebookId, next);

    if (page) {
      // Remove from any other notebook if it was placed there previously
      const otherNotebooks = { ...get().byNotebook };
      for (const [otherId, pieces] of Object.entries(otherNotebooks)) {
        if (otherId !== notebookId && pieces.some((p) => p.id === id)) {
          const cleaned = pieces.filter((p) => p.id !== id);
          otherNotebooks[otherId] = cleaned;
          savePieces(store, otherId, cleaned);
        }
      }
      set({ byNotebook: { ...otherNotebooks, [notebookId]: next } });
    } else {
      set((s) => ({ byNotebook: { ...s.byNotebook, [notebookId]: next } }));
    }
  },

  setOpen: (open) => set({ open, lastEarned: open ? get().lastEarned : null }),
}));
