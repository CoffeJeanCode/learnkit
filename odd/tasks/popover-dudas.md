# Popover "¿Dudas?": contenedor grande + fix de render + lemiscata

## Objective
Make the per-block `BlockLexicalAssistant` popover usable: a bigger container, three measured rendering defects fixed, and a small lemniscate (∞) animation while it thinks.

## Why
User request (2026-09-29 08:30): "Al popover sobre dudas haz el contenedor más grande y revisa porque no permite renderizar correctamente. y añade una animación pequeña de lemiscata al pensar".
User decision (question, 2026-09-29 08:49): **this session implements it now**, scoped to the popover only.

## Scope / authorized
- IN: `src/features/notebook/LexicalAssistantPopover.tsx`, `src/styles.css` (popover block only, lines ~886-920).
- OUT: theory-block/selection-popover work by the *concurrent session* on branch `feature/theory-selection-popover` (`schemas.ts`, `notebook.rs`, `AnchoredMicroTheoryBlock.tsx`, `block_generator_agent.rs`, …) — do NOT touch or revert those; they are live WIP from another writer.

## Key findings from the diagnosis (do not re-derive — measured, headless Chrome, viewport 1424x805, `.workspace` 726px)
1. **Anchoring**: dropdown only ever opens downward (`top: calc(100% + 6px)`), no collision check, while `.workspace` is `overflow-y: auto`. Measured: dropdown `top=870` vs workspace bottom `805` → 100% invisible; it also grew the workspace scroll area `clientH=726 → scrollH=1274`.
2. **Horizontal spill**: `.lexical-turn-answer { white-space: pre-wrap }` has no `overflow-wrap` → one long unbroken code token made `.lexical-popover-history` `scrollWidth=544` inside a `356px` card; `.card` has no `overflow`, so text paints outside the border.
3. **New answer invisible**: history capped `max-height: 40vh` (`clientH=254` of `scrollH=466`), `scrollTop=0`, no auto-scroll → newest turn `HIDDEN_BELOW_FOLD=true`.
4. **Size**: width `min(360px, 80vw)`, dropdown `max-height: 60vh`, history `max-height: 40vh`.

## TDD mode
Off — no unit-test runner covers CSS/interaction (repo checks = `bunx tsc --noEmit` + `bun run scripts/check-rendering.tsx`). Verification is those two commands plus re-running the headless-Chrome layout harness (`%TEMP%\opencode\popover-harness.html`, extended for the flip class).

## Tasks
- [x] T1 — Size: dropdown `width: min(440px, 92vw)`, `max-height: min(70vh, 620px)`; history loses its hard `40vh` cap and becomes `flex: 1 1 auto; min-height: 0`; `flex: none` on `.lexical-popover-input` and the error `.alert` so only history absorbs the space. Verified: width 440, max-height 563.5px (=70vh), history `clientH=359` (was 254, old cap 322).
- [x] T2 — Anchoring: `up/down` placement state; `useLayoutEffect` measure on open (+ `resize` + capturing `scroll`) against the nearest scroll container; `.lexical-popover-up` flips to `bottom: calc(100% + 6px)`. Verified in harness: high trigger → `up=false`, fits (below 523 ≥ needed 189); low trigger with 6px below → `up=true`, popover `top=194..757` fully inside the workspace (was 548px outside).
- [x] T3 — Overflow + auto-scroll: `overflow-wrap: anywhere` + `min-width: 0` on turns; history auto-scrolls to bottom on `[history, busy]`. Verified: `H-SPILL: ninguno` (was `scrollWidth=544` in a 356px card); `V-SPILL: OK`.
- [x] T4 — Lemniscate: ∞ SVG (`pathLength={1}`) with `lex-inf-draw` + `lex-inf-wobble`, plus `.lex-lemniscate *` in the `prefers-reduced-motion` block.

## Verification (parent, 2026-09-29)
- `bunx tsc --noEmit` → exit 0 (re-run by parent, not just the writer)
- `bun run scripts/check-rendering.tsx` → `PASS` (16 `[ok]`)
- Headless-Chrome layout harness (both scenarios above) → all assertions true
- `git diff -- src/styles.css` → writer's hunks present; the concurrent agent's hunks (`0.85em` code span, `.lemniscate-doodle` utility) intact — nothing reverted

## Progress
2026-09-29: diagnosis complete → T1-T4 implemented by one delegated writer → verified (typecheck, render checks, two-scenario layout harness, diff integrity). NOT committed (user preference: no commits without an explicit request).

## Review (RDD on by default)
Preflight STATUS returned `collect` with `intended_untracked_selection_required`; the candidate is the WHOLE worktree (14 paths) mixing the concurrent session's in-flight T5 and stale WIP. **User decision 09:0x: stop the review here** — `gentle-ai review start` was never invoked, no freeze, no receipt, no authority burned.

## Next step
Nothing pending on this feature. When the concurrent session finishes: (a) decide which lemniscate survives (mine `.lex-lemniscate` is wired up; theirs `.lemniscate-doodle` at `styles.css:587` is unused), (b) commit, (c) run the review on a clean candidate if wanted.
