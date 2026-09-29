# Feature: Word-style margin comment popover (unified)

**Status**: in progress
**Route**: delegated direct (writer trigger: 3+ non-trivial files — LexicalAssistantPopover.tsx, useTextSelectionPopover.ts, blocks/index.tsx, styles.css)
**TDD**: off (no project TDD config; verification = `bunx tsc --noEmit` + `bun run scripts/check-rendering.tsx` + structural readback)
**Delivery**: NO commits, NO push, NO review unless the user explicitly asks (standing rule).
**Verification constraint**: NO browser/headless Chrome testing (standing user rule).

## Objective

Both entry points for asking the assistant — the text-selection popover and the
per-block "¿Dudas?" button — must open ONE unified comment surface that sits
BESIDE its block like a Word/Google Docs margin comment, while keeping the
selected text visible (persistent highlight + quote header).

## Why

- Today the selection popover is `position: fixed` centered on the selection's
  anchor point, which can COVER the selected text ("no puedo ver qué estoy
  seleccionando").
- Two popovers exist with duplicated ask/history state
  (`BlockLexicalAssistant` own states vs `useLexicalAssistantThread`).

## Design decisions (settled — do not relitigate)

1. **One surface, two triggers.** Selection and the ¿Dudas? button both render
   the same comment component, mounted per block (`.theory-selection-zone`
   wrapper owns it; the button opens the same slot for its block).
   Path A's duplicated ask/history state folds onto `useLexicalAssistantThread`.
2. **Position = margin slot, `position: fixed`:**
   - `x = blockRect.right + 12` (over the 24px gap + 300px sticky sidebar —
     intentional, like Word painting over the margin; z-index stays 40 > sidebar).
   - If the viewport cannot fit the comment at that x
     (`x + width > viewportWidth - 10`), fall back to today's behavior: centered
     on the selection anchor (or the button), clamped to viewport.
   - `y`: selection → align near selection top (`selectionTop - 12`, fall back
     to `+26` below when no room above); button → block top. Then clamp so the
     comment fits the viewport (`10 … viewportHeight - height - 10`).
   - Comment re-anchors on scroll (capture) and resize for BOTH paths.
   - While open, the comment scrolls/re-anchors with content; the persistent
     selection highlight (`::highlight(popover-selection)`) and the
     `.selection-quote` header stay as-is.
3. **Width**: `min(360px, calc(100vw - 20px))`; max-height and internal history
   scrolling unchanged (`.lexical-popover-history`).
4. **Narrow viewports (<860px, single column, no sidebar)**: the same formula
   naturally degrades — when there is no room at the right it falls back to the
   near-selection clamp. No separate branch.
5. **Thread semantics preserved**: selecting a NEW distinct selection resets the
   thread (current `key={selection.text}` behavior); button-open thread uses
   block context. Opening via selection while a button thread is open switches
   to the selection context (reset), same as today.

## Selection visibility acceptance (user's literal ask)

- Selected text stays highlighted while the comment is open (exists — keep).
- The quote of the selection renders at the top of the comment (exists — keep).
- The comment itself no longer covers the selection in the standard layout
  (margin placement) — verified structurally (position math), not in a browser.

## Constraints

- Concurrent session writes the same repo: RE-READ every file immediately before
  editing; narrow content-based edits only; never rewrite whole files; never
  revert foreign hunks (git hunks may interleave in styles.css).
- Skills to load first: `C:\Users\Jean Pierre Ortiz\.agents\skills\frontend-ui-engineering\SKILL.md`
  and `C:\Users\Jean Pierre Ortiz\.config\opencode\skills\work-unit-commits\SKILL.md`
  (commit guidance read for awareness only — do NOT commit).
- Engram is down: `mem_save` will likely fail; attempt once for major
  discoveries, note "engram unavailable", do not retry.

## Tasks

- [x] T1 — Unify state: refactor `BlockLexicalAssistant` in
      `src/features/notebook/LexicalAssistantPopover.tsx` to use
      `useLexicalAssistantThread` (drop duplicated ask/history states).
      DONE — `BlockLexicalAssistant` is now a trigger-only component
      (`open`/`onToggle`/`triggerRef` props, no state); its ask/history/
      busy/error logic was deleted and lives only in
      `useLexicalAssistantThread`, called once by the unified
      `LexicalMarginComment` (both modes). History autoscroll effect moved
      into the unified surface (it previously existed only on the button
      path, so selection mode inherits it).
- [x] T2 — Margin positioning: single hook (extend
      `useTextSelectionPopover.ts` or add alongside) computing fixed coords from
      the block wrapper rect + optional selection rect per Design §2; attach
      scroll (capture) + resize re-anchoring for both triggers; remove the old
      dropdown flip walker for the button path once unused.
      DONE — `useMarginCommentPosition` added to
      `useTextSelectionPopover.ts`: `x = blockRect.right + 12` when
      `x + w ≤ vw − 10`, else centered on `anchor.x` (selection) / trigger
      center (button), clamped `[10, vw − w − 10]`; `y = selectionTop − 12`
      (fallback `+26` below) or block top, clamped `[10, vh − h − 10]`;
      `window` scroll (capture) + resize listeners + `ResizeObserver` on the
      comment (replaces the old `history.length/busy/error` effect deps).
      Flip walker (`up` state, scroll-container walk) and
      `.lexical-popover-up` removed — grep showed no other consumer.
- [x] T3 — Mount wiring in `src/features/notebook/blocks/index.tsx`: button and
      selection both open the same per-block comment slot (block element ref
      threaded to the positioning hook — today `TheorySelectionAssistant`
      receives no element ref).
      DONE — `DynamicNotebookBlock` hoists `blockRef` (section),
      `triggerRef` (¿Dudas? button) and `assistantOpen` above the parse
      early-return (hook order must not depend on parse success), passes
      them to `BlockLexicalAssistant` and
      `TheorySelectionAssistant` (+ `buttonTerm`); the zone wrapper owns the
      single `LexicalMarginComment` slot (selection wins over button open,
      `key={selection:${text}}` / `key="button"` preserve thread-reset
      semantics).
- [x] T4 — Styles in `src/styles.css`: `.margin-comment` (or repurposed
      `.block-lexical-dropdown`/`.selection-lexical-popover` rules) — width
      min(360px, 100vw-20px), z-index 40, no layout impact; remove obsolete
      flip/dropdown positioning rules only after confirming no other consumer.
      DONE — `.margin-comment` (+ `.ready`, `> .alert`) added with
      `position: fixed`, width `min(360px, calc(100vw − 20px))`,
      `max-height: min(70vh, 560px)`, z-index 40; removed
      `.block-lexical-dropdown`, `.block-lexical-dropdown.lexical-popover-up`,
      `.block-lexical-dropdown > .alert`, `.selection-lexical-popover(+.ready)`
      after repo-wide grep (only historical ODD docs still mention them).
      Kept: `.selection-quote`, `.selection-popover-close`,
      `.lexical-popover-history`/`.lexical-turn*`/`.lexical-popover-input`,
      `::highlight(popover-selection)`, reduced-motion block untouched.
- [x] T5 — Responsive/narrow fallback + edge cases: comment fits viewport at
      1280/1024/860/600 widths by formula (structural check of the math);
      sidebar overlay does not break sticky behavior.
      DONE — bun desk-check snippet (temp, not committed) computed the
      formula from the real layout rules (workspace padding 32, view 1180
      centered, grid `1fr 300px` gap 24, media ≤860 single column) for
      vw 1280/1024/860/600 × scrollbar {0,17}: ALL WIDTHS FIT (left/top
      inside `[10, vw−w−10]`/`[10, vh−h−10]` everywhere). Margin mode
      engages at 1280 only with a ≥16px scrollbar (view centers → block
      right edge shifts); otherwise the near-selection fallback runs —
      both are on-screen by clamp. Fixed comment is out of flow, so the
      sticky sidebar (z auto < 40) keeps its behavior.
      Formula summary also documented in `useTextSelectionPopover.ts`.
- [x] T6 — Verification + structural readback: `bunx tsc --noEmit`,
      `bun run scripts/check-rendering.tsx` (31 checks baseline), esbuild CSS
      parse; report `<command>: <result>` per command. No browser.
      DONE — `bunx tsc --noEmit`: 0 errors;
      `bun run scripts/check-rendering.tsx`: PASS (31/31 ok);
      `bunx esbuild src/styles.css …`: "Done in 30ms", exit 0.
      Full structural readback of all four edited files performed.

## Acceptance criteria

- [x] One component/hook path serves both triggers (no duplicated ask state left).
- [x] Comment anchors at block right margin when room exists; near-selection
      fallback otherwise (formula verified in code).
- [x] Selected text highlight + quote header visible while open.
- [x] tsc 0 errors; check-rendering PASS (all baseline checks); CSS parses.
- [x] No commits created.

## Progress log

- 2026-09-29: exploration complete (map: two popovers, sidebar owns right margin,
  fixed vs absolute mechanics, quote/highlight already present). Scope decided
  with user: both entry points unified.
- 2026-09-29: T1–T6 implemented (writer pass, no commits). Files touched:
  `LexicalAssistantPopover.tsx` (trigger-only button + unified
  `LexicalMarginComment`), `useTextSelectionPopover.ts` (new
  `useMarginCommentPosition` hook + desk-check comment), `blocks/index.tsx`
  (hoisted refs/open state + wiring), `styles.css` (`.margin-comment` in,
  dropdown/flip rules out). Known edge: at vw 1280 the margin slot depends
  on scrollbar width (≥16px → margin, else near-selection fallback) —
  accepted, both paths clamp on-screen per Design §2. Verification:
  tsc 0 errors, check-rendering PASS 31/31, esbuild exit 0. NO browser
  verification performed (standing rule).
