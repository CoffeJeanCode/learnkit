import { useCallback, useEffect, useLayoutEffect, useRef, useState, type RefObject } from "react";

/** Viewport anchor point for a floating popover — the end (right edge) of
 *  the selection's bounding rect, at its top. */
export interface SelectionAnchor {
  x: number;
  y: number;
}

export interface DetectedSelection {
  /** Trimmed selected text — never empty/whitespace-only. */
  text: string;
  anchor: SelectionAnchor;
  /** `cloneRange()` of the selection that produced `text`. Kept so the
   *  visual highlight can persist after the browser selection itself
   *  collapses (clicking the popover's input does exactly that) — registered
   *  via the Custom Highlight API below. */
  range: Range;
}

/** Registry key for the Custom Highlight API — MUST match the
 *  `::highlight(...)` rule in `styles.css`. One name, reused for every
 *  popover selection (registering a new one replaces the previous). */
const SELECTION_HIGHLIGHT_NAME = "popover-selection";

/** Registry key for the "already answered" mark left behind when the popover
 *  closes over a fragment the assistant has already helped with — MUST match
 *  `::highlight(popover-selection-saved)` in `styles.css`. Kept in a SEPARATE
 *  name so the live selection and the saved mark never fight for one slot. */
const SAVED_HIGHLIGHT_NAME = "popover-selection-saved";

/** Caller-supplied behaviour the hook cannot infer on its own. */
export interface TextSelectionPopoverOptions {
  /** Asked on every close path: should this fragment's highlight survive the
   *  close? The caller answers "yes" only while the assistant already has an
   *  answer for it, which is what turns the mark into "help is here" instead
   *  of a stray highlight. */
  keepOnClose?: () => boolean;
}

function readContainedSelection(container: HTMLDivElement | null): DetectedSelection | null {
  if (!container) return null;
  const sel = window.getSelection();
  if (!sel || sel.rangeCount === 0 || sel.isCollapsed) return null;
  const range = sel.getRangeAt(0);
  // Scopes detection to THIS container only — a selection started in another
  // notebook block (which has its own hook instance) resolves to null here,
  // closing this popover instead of misfiring. Selections inside a plain
  // <input>/<textarea> never reach `window.getSelection()` at all (that's a
  // separate browser selection model), so they're already excluded for free.
  if (!container.contains(range.commonAncestorContainer)) return null;
  const text = sel.toString().trim();
  if (!text) return null;
  const rect = range.getBoundingClientRect();
  if (rect.width === 0 && rect.height === 0) return null;
  // Clone: the live range from `getRangeAt` tracks mutations/selection
  // changes, and the highlight below must outlive the selection itself.
  return { text, anchor: { x: rect.right, y: rect.top }, range: range.cloneRange() };
}

/** Detects a real (non-empty, non-collapsed) text selection made inside
 *  `containerRef`'s subtree and exposes it plus a viewport anchor point to
 *  float a contextual popover near it. Scoped to the container: selections
 *  elsewhere on the page never surface here. Callers should mount their
 *  floating popover OUTSIDE `containerRef` (e.g. as a sibling) and attach
 *  the returned `popoverRef` to its root node, so selecting text inside the
 *  popover itself (its quoted preview, its answers) doesn't re-trigger
 *  detection or get treated as an outside click.
 *
 *  Closes on Escape, a mousedown outside both the container and the
 *  popover, or the selection moving somewhere else entirely. A collapsed
 *  selection only closes it when the interaction was NOT inside the popover
 *  (focusing/clicking/Tab-ing into the popover must keep it open so the
 *  student can keep typing and copy the answer). Recomputes (rather than
 *  going stale) on scroll, since the selection's bounding rect moves with
 *  the page — but a scroll alone never closes it. */
export function useTextSelectionPopover(
  containerRef: RefObject<HTMLDivElement | null>,
  options: TextSelectionPopoverOptions = {},
) {
  const [selection, setSelection] = useState<DetectedSelection | null>(null);
  /** A fragment the student already got help with, kept after the popover
   *  closed so its mark — and a click on that mark — can bring the help back.
   *  Only ever written by `closeSelection`, so it is either `null` or the
   *  fragment that just went away: never two at once, never alongside a live
   *  `selection`. */
  const [saved, setSaved] = useState<DetectedSelection | null>(null);
  const popoverRef = useRef<HTMLDivElement | null>(null);
  // Options are read from inside long-lived listeners whose effect deps never
  // include them — a plain closure would call the FIRST `keepOnClose` forever.
  const optionsRef = useRef(options);
  optionsRef.current = options;
  // Where the last pointer press landed: the collapsed-selection rule below
  // needs to tell "the student clicked the popover (its input — that
  // collapses the document selection, the popover must stay open)" from a
  // click on the rest of the page (which must still close it).
  const lastPointerDownRef = useRef<Node | null>(null);
  // Mirror of `selection` for the event handlers: `closeSelection` is a stable
  // callback (its listeners are registered once), so it cannot read state.
  const selectionRef = useRef<DetectedSelection | null>(null);

  /** The single close path. Leaves the fragment highlighted (and stored for a
   *  click-to-reopen) when the caller says the student already has an answer
   *  for it; otherwise drops the mark exactly as before. */
  const closeSelection = useCallback(() => {
    const current = selectionRef.current;
    if (!current) return;
    if (optionsRef.current.keepOnClose?.()) setSaved(current);
    setSelection(null);
  }, []);

  useEffect(() => {
    selectionRef.current = selection;
  }, [selection]);

  useEffect(() => {
    const onPointerDown = (e: PointerEvent) => {
      lastPointerDownRef.current = e.target as Node | null;
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, []);

  useEffect(() => {
    const onSelectionChange = () => {
      const popover = popoverRef.current;
      const sel = window.getSelection();
      if (sel && sel.rangeCount > 0 && !sel.isCollapsed) {
        // (1) A selection INSIDE the popover: keep it — this is how the
        // student copies the answer. The popover is a sibling of
        // `containerRef`, so `readContainedSelection` would return null and
        // close the popover mid-copy without this rule.
        if (popover?.contains(sel.getRangeAt(0).commonAncestorContainer)) return;
        // (2) A fresh selection inside the container replaces BOTH the live
        // popover and any saved mark — one mark per block, newest wins.
        const next = readContainedSelection(containerRef.current);
        if (next) {
          setSaved(null);
          setSelection(next);
          return;
        }
        // (3) A non-collapsed selection ANYWHERE else took the selection over
        // (another block, another pane) — close, but a fragment this block
        // already answered keeps its mark.
        closeSelection();
        return;
      }
      // (4) Collapsed or no selection at all: keep it ONLY when the
      // interaction is happening inside the popover — the last pointer press
      // landed there (clicking the input) or focus lives there (Tab). Any
      // other collapse closes the popover, as it always did.
      const active = document.activeElement;
      if (popover && (popover.contains(lastPointerDownRef.current) || popover.contains(active))) return;
      closeSelection();
    };
    document.addEventListener("selectionchange", onSelectionChange);
    return () => document.removeEventListener("selectionchange", onSelectionChange);
  }, [containerRef, closeSelection]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      // Popover open → close it (keeping the mark when it was already
      // answered); nothing open but a mark left behind → drop the mark, the
      // student is backing out.
      if (selection) closeSelection();
      else setSaved(null);
    };
    const onPointerDown = (e: MouseEvent) => {
      const target = e.target as Node;
      if (popoverRef.current?.contains(target)) return;
      // A mousedown that starts a fresh selection inside the container is
      // left alone here — the resulting `selectionchange` (empty during the
      // drag, then the new selection on mouseup) drives the state instead.
      if (containerRef.current?.contains(target)) return;
      closeSelection();
    };
    // Scroll must NEVER close: only refresh the anchor when the selection
    // is still readable inside the container. A collapsed selection with the
    // popover open (input focused) used to resolve to null here and close it.
    const onScroll = () => {
      const next = readContainedSelection(containerRef.current);
      if (next) setSelection(next);
    };
    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("mousedown", onPointerDown);
    window.addEventListener("scroll", onScroll, { capture: true, passive: true });
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("mousedown", onPointerDown);
      window.removeEventListener("scroll", onScroll, true);
    };
  }, [selection, containerRef, closeSelection]);

  // Reopen a saved mark. The Custom Highlight paints over the text but is NOT
  // hit-testable, while the underlying text nodes still take the click — so
  // membership is decided against the stored range's client rects. The default
  // action (placing a caret / starting a drag) is cancelled, which is also
  // what stops the `selectionchange` a caret would fire from closing the
  // popover one tick after it reopens.
  useEffect(() => {
    if (!saved || selection) return;
    const containsPoint = (x: number, y: number) =>
      Array.from(saved.range.getClientRects()).some((r) => x >= r.left && x <= r.right && y >= r.top && y <= r.bottom);
    const onSavedMouseDown = (e: MouseEvent) => {
      if (!containsPoint(e.clientX, e.clientY)) return;
      e.preventDefault();
      e.stopPropagation();
      setSelection(saved);
      setSaved(null);
    };
    // Affordance: the mark itself is the only clue the help is still there,
    // so the pointer changes over it instead of making the student guess.
    const onSavedMouseMove = (e: MouseEvent) => {
      const over = containsPoint(e.clientX, e.clientY);
      if (over !== (document.body.style.cursor === "pointer")) document.body.style.cursor = over ? "pointer" : "";
    };
    document.addEventListener("mousedown", onSavedMouseDown, true);
    document.addEventListener("mousemove", onSavedMouseMove);
    return () => {
      document.removeEventListener("mousedown", onSavedMouseDown, true);
      document.removeEventListener("mousemove", onSavedMouseMove);
      document.body.style.cursor = "";
    };
  }, [saved, selection]);

  // Keep the highlight painted while the popover is open, even after the
  // document selection itself collapses (focusing the input, selecting text
  // INSIDE the popover to copy it). The Custom Highlight API paints over the
  // normal selection rendering; the native `::selection` rule still covers
  // the live drag. Every access is guarded — a browser without the API just
  // loses the persistent highlight exactly as before, and never throws.
  // The SAVED mark uses its own registry slot and its own CSS rule so it
  // survives the popover closing instead of dying with `selection`.
  // NOTE: TS 5.8's lib.dom types `HighlightRegistry` with only `forEach`,
  // so the `set`/`delete` methods are cast to their real signatures.
  useEffect(() => {
    if (typeof CSS === "undefined" || !CSS.highlights || typeof Highlight === "undefined") return;
    const registry = CSS.highlights as unknown as {
      set(name: string, value: Highlight): void;
      delete(name: string): boolean;
    };
    if (selection) registry.set(SELECTION_HIGHLIGHT_NAME, new Highlight(selection.range));
    else registry.delete(SELECTION_HIGHLIGHT_NAME);
    if (saved) registry.set(SAVED_HIGHLIGHT_NAME, new Highlight(saved.range));
    else registry.delete(SAVED_HIGHLIGHT_NAME);
    return () => {
      registry.delete(SELECTION_HIGHLIGHT_NAME);
      registry.delete(SAVED_HIGHLIGHT_NAME);
    };
  }, [selection, saved]);

  return { selection, saved, popoverRef, clear: closeSelection };
}

// --- Margin-comment positioning (both triggers) ----------------------------
//
// Word-style margin comment: `position: fixed`, parked BESIDE its block
// (`x = blockRect.right + 12`, over the 24px grid gap and the sticky
// sidebar — intentional, z-index 40 paints above it) so the selected text
// underneath is never covered. When the viewport cannot fit the comment at
// that x (`x + width > viewportWidth - 10`), it falls back to the previous
// behavior: horizontally centered on the selection anchor (or, for the
// ¿Dudas? button, on the button itself), clamped to the viewport.
//
// Structural desk-check of the formula (block right edge derived from the
// layout rules in `styles.css`: workspace padding 26/32, `.view.notebook`
// max-width 1180 centered, shell grid `1fr 300px` gap 24, block padding 16;
// comment width = min(360, vw - 20); see T5 in
// `odd/tasks/popover-margin-comment.md`):
//   vw 1280 → marginX ≈ 918 (no-scrollbar case): 918 + 360 = 1278 >
//            1270 → narrow fallback (fits, clamped) — borderline: a
//            scrollbar ≥ 16px pulls the centered view left and the margin
//            slot fits instead.
//   vw 1024 → marginX ≈ 680: 1040 > 1014 → fallback (fits).
//   vw 860  → single column (media ≤ 860): marginX ≈ 840: 1200 > 850 →
//            fallback (fits).
//   vw 600  → single column: marginX ≈ 580: 940 > 590 → fallback (fits).
// In every case the final clamp `10 … vw - width - 10` guarantees the
// comment is fully on-screen; `width ≤ vw - 20` makes that range non-empty.

/** Viewport margin (px) kept around the comment on every edge. */
const COMMENT_VIEWPORT_MARGIN = 10;
/** Horizontal gap between the block's right edge and the comment. */
const COMMENT_BLOCK_GAP = 12;
/** Selection mode: comment top sits this far ABOVE the selection's top… */
const COMMENT_ABOVE_OFFSET = 12;
/** …or this far BELOW it when there is no room above (mirrors the old
 *  popover's above/below flip). */
const COMMENT_BELOW_OFFSET = 26;

export interface MarginCommentPosition {
  left: number;
  top: number;
  /** False until the first measured layout pass, so the comment can stay
   *  `visibility: hidden` and never flash at an unmeasured point. */
  ready: boolean;
}

/** Computes fixed viewport coordinates for the per-block margin comment.
 *
 *  - `y`: with a selection → `selectionTop - 12` (fall back to
 *    `selectionTop + 26` when there is no room above); button → the block's
 *    top. Always clamped to `10 … viewportHeight - height - 10`.
 *  - `x`: prefer `blockRect.right + 12` when it fits
 *    (`x + width ≤ viewportWidth - 10`), else centered on the selection
 *    anchor (or, without one, on the ¿Dudas? trigger button), clamped.
 *  - Re-anchors on scroll (capture) and resize for BOTH triggers while
 *    mounted, and whenever its own content changes size (a new history
 *    turn, the busy row, an error) via `ResizeObserver`. */
export function useMarginCommentPosition({
  blockRef,
  triggerRef,
  popoverRef,
  anchor,
}: {
  /** `section.notebook-block-wrapper` owning the comment — its right edge
   *  anchors the margin slot, its top anchors the button mode. */
  blockRef: RefObject<HTMLElement | null>;
  /** The ¿Dudas? button — fallback centering anchor when there is no
   *  selection (button mode). */
  triggerRef: RefObject<HTMLElement | null>;
  /** The comment's root node: measured for size, and the element whose
   *  `left`/`top` we set. */
  popoverRef: RefObject<HTMLDivElement | null>;
  /** Viewport anchor of the selection (x = its right edge, y = its top),
   *  or `null` in button mode. */
  anchor: SelectionAnchor | null;
}): MarginCommentPosition {
  const anchorX = anchor?.x ?? null;
  const anchorY = anchor?.y ?? null;
  const [pos, setPos] = useState<MarginCommentPosition>({ left: 0, top: 0, ready: false });

  const measure = useCallback(() => {
    const el = popoverRef.current;
    if (!el) return;
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const blockRect = blockRef.current?.getBoundingClientRect() ?? null;
    const triggerRect = triggerRef.current?.getBoundingClientRect() ?? null;

    // Vertical: near the selection top (or the block top for the button),
    // then clamped so the whole comment stays on screen.
    let top: number;
    if (anchorY != null) {
      top = anchorY - COMMENT_ABOVE_OFFSET; // prefer aligned near the top
      if (top < COMMENT_VIEWPORT_MARGIN) top = anchorY + COMMENT_BELOW_OFFSET; // no room above → below it
    } else {
      top = blockRect ? blockRect.top : COMMENT_VIEWPORT_MARGIN;
    }
    top = Math.min(Math.max(top, COMMENT_VIEWPORT_MARGIN), vh - h - COMMENT_VIEWPORT_MARGIN);

    // Horizontal: margin slot right of the block when the viewport fits it…
    const marginX = blockRect ? blockRect.right + COMMENT_BLOCK_GAP : Number.NEGATIVE_INFINITY;
    const fallbackX =
      anchorX != null
        ? anchorX // …else centered on the selection anchor (old behavior)…
        : triggerRect
          ? triggerRect.left + triggerRect.width / 2 // …or on the button…
          : blockRect
            ? blockRect.right
            : vw / 2;
    const left = marginX + w <= vw - COMMENT_VIEWPORT_MARGIN ? marginX : fallbackX - w / 2;
    const clampedLeft = Math.min(Math.max(left, COMMENT_VIEWPORT_MARGIN), vw - w - COMMENT_VIEWPORT_MARGIN);

    // Bail out when nothing moved (scroll/ResizeObserver storms must not
    // re-render the comment for nothing).
    setPos((prev) => (prev.ready && prev.left === clampedLeft && prev.top === top ? prev : { left: clampedLeft, top, ready: true }));
  }, [anchorX, anchorY, blockRef, popoverRef, triggerRef]);

  useLayoutEffect(() => {
    measure();
    window.addEventListener("resize", measure);
    window.addEventListener("scroll", measure, { capture: true, passive: true }); // capture: scroll doesn't bubble
    // Content growth (new turn / busy row / error) changes the height the
    // clamp depends on — re-measure instead of tracking history state.
    const el = popoverRef.current;
    const observer = el && typeof ResizeObserver !== "undefined" ? new ResizeObserver(measure) : null;
    if (el && observer) observer.observe(el);
    return () => {
      window.removeEventListener("resize", measure);
      window.removeEventListener("scroll", measure, true);
      observer?.disconnect();
    };
  }, [measure, popoverRef]);

  return pos;
}
