# Popover "¿Dudas?": selección, copia, markdown y cobertura de bloques

## Objective
Fix four bugs in the text-selection assistant popover: (A) focusing the popover input deselects/closes the "texto a preguntas", (B) the popover's answer cannot be copied, (C) the answer does not render markdown, (D) the selection feature must trigger on ANY block type, not only `anchored_micro_theory`.

## Why
User report (2026-09-29): "Cuando se hace focus en el input del popover se deseleccionar el texto a preguntas y no permite copiar la respuesta del popover y tampoco renderiza el markdown" + "la feature de seleccion deberia ser cualquier bloque". Constraint: **no Chrome/browser testing** — verification is typecheck + the repo's render checks only.

## Root causes (from read-only investigation — do not re-derive)
- `src/` has ZERO `removeAllRanges`/`.blur(`/`preventDefault()`/`user-select`/`pointer-events` → A and B are state/JSX-driven, not CSS.
- **A+B (shared cause)**: `useTextSelectionPopover.ts:52` runs `setSelection(readContainedSelection(...))` on EVERY document `selectionchange`. `readContainedSelection` returns `null` when the selection is collapsed (`:19`) OR not inside `containerRef` (`:26`). The popover is a SIBLING of the container (`LexicalAssistantPopover.tsx:257-261`), so:
  - clicking the input collapses the document selection → `null` → `:262` unmounts the popover while the user is typing (A);
  - selecting text inside the popover (to copy the answer) is "outside the container" → `null` → unmount mid-selection → clipboard gets nothing (B).
  - The mousedown guard at `:63-71` DOES whitelist the popover (`:65`) but the `selectionchange` path bypasses it. Same for `onScroll` (`:72`), which also closes when the selection is collapsed (popover open + input focused + scroll = closes).
- **C**: answer is a bare string in `<p>` (`LexicalAssistantPopover.tsx:357` selection popover, `:135` button dropdown); no renderer imported. `.lexical-turn-answer` (`styles.css:912`) uses `pre-wrap`, so `**bold**`/`1.` lists print literally. Backend returns the model text verbatim (`lexical_assistant_service.rs:49-50`) — markdown is expected, not anomalous.
- **D**: the ONLY gate is `blocks/index.tsx:148-156` — `content.blockType === "anchored_micro_theory"` decides whether `TheorySelectionAssistant` wraps the block. `deriveBlockLexicalContext()` (`blocks/index.tsx:39-74`) already handles all 8 block types exhaustively, and `BlockLexicalAssistant` is already mounted unconditionally at `:140-145`. Block types: `src/lib/schemas.ts:354-436` — 7 of 8 are excluded today. All block renderers produce static selectable DOM (no Lexical/contentEditable).

## Model answer format (source: `src-tauri/src/agents/lexical_assistant_agent.rs:17-50`)
"SIEMPRE 3 CAPAS" (analogía / mecanismo / límite), 2-4 lines each, no explicit markdown mandate and no ban → the model commonly emits `**bold**` labels, `1.`/`-` lists and inline code. Model = claude-sonnet-4-6.

## Design decisions (settled — do not relitigate)
1. **Fix A+B in the hook's `selectionchange` logic, not by preventing focus.** New rules while a popover selection is active:
   - non-collapsed selection **inside the popover** → keep state (lets the user select/copy the answer);
   - non-collapsed selection **inside the container** → update state (fresh selection, existing behavior);
   - non-collapsed selection **elsewhere** → close (a different block took over);
   - collapsed/no selection → keep ONLY when the interaction is inside the popover (last `pointerdown` target in popover, or `document.activeElement` inside popover — covers keyboard Tab); otherwise close (preserves today's behavior).
   - `onScroll` must NOT close: only recompute the anchor when `readContainedSelection` yields a fresh value.
2. **Persist the visual highlight** while the popover is open: store a `range.cloneRange()` in `DetectedSelection` and register it via the Custom Highlight API (`CSS.highlights` + `Highlight`) under one constant name; delete it when the state clears. MUST be guarded (`typeof` checks / optional chaining) so browsers without the API just lose the highlight as today — never throw. The existing `.theory-selection-zone ::selection` rule stays for the native drag.
3. **Markdown: extend the repo's hand-rolled renderer, add NO dependency.** `package.json` has no markdown library and `RichText.tsx:3-11` states it is deliberately not one. Extend `InlineText` with `**bold**` → `<strong>` (NEVER inside inline-code spans) and `RichText` with contiguous list blocks (`- `/`* ` → `<ul>`, `1. ` → `<ol>`) plus heading lines (`#{1,6} `) rendered as a bold paragraph (strip the `#`s — no new CSS classes invented). Code fences keep working exactly as now; do not parse markdown inside fences.
4. **Render both answers with `RichText`**: `LexicalAssistantPopover.tsx:135` and `:357` — `<RichText text={turn.answer} className="lexical-turn-answer" />` replacing the `<p>` (RichText emits `<div>` and must NEVER nest inside `<p>` — `RichText.tsx:77-79`). Adjust `.lexical-turn-answer` CSS only if the `<div>` nesting requires it (keep `font-size`/`pre-wrap`/`overflow-wrap`).
5. **D = remove the mount gate**: `blocks/index.tsx:148-156` wraps EVERY block in `TheorySelectionAssistant` (props already available for all 8 types). Keep the `.theory-selection-zone` class name (cosmetic rename is out of scope). Update the stale "theory only" comments in `LexicalAssistantPopover.tsx:175,236` and `styles.css:943`.
6. **No commits** (standing user rule), **no Chrome/headless testing** (explicit user instruction).

## TDD mode
Partial: the repo's `scripts/check-rendering.tsx` (node/bun, NOT a browser) must gain cases for bold, ordered/unordered lists and heading stripping, then PASS. A/B/D behavior is interaction-level and has no test runner → covered by typecheck + code reasoning only; disclose honestly as not machine-verified.

## Tasks
- [ ] T1 — `useTextSelectionPopover.ts`: new `selectionchange` rules (decision 1) + `range` in `DetectedSelection` + guarded highlight registration (decision 2) + scroll no longer closes.
- [ ] T2 — markdown: extend `blocks/RichText.tsx` (bold, lists, heading-strip — decisions 3) and render both popover answers with `RichText` (decision 4); CSS only if required.
- [ ] T3 — `blocks/index.tsx`: remove the `anchored_micro_theory` gate (decision 5) + fix stale comments.
- [ ] T4 — `scripts/check-rendering.tsx`: new cases; full suite PASS.

## Route
T1-T4 → one writer (6 files, 3 of them concurrently touched by another session → re-read immediately before each edit, narrow edits only, never reformat or revert foreign hunks).

## Verification (no Chrome)
1. `bunx tsc --noEmit` → exit 0
2. `bun run scripts/check-rendering.tsx` → PASS including new cases
3. `git diff --stat` scoped to the 6 files

## Tasks
- [x] T1 — `useTextSelectionPopover.ts`: `range: Range` in `DetectedSelection` (cloned), 4-rule `selectionchange` (popover→keep, container→update, elsewhere→close, collapsed→keep only when pointer/focus is inside the popover), `onScroll` no longer closes, guarded Custom Highlight (`popover-selection`) registered/cleaned up. Escape/mousedown-outside and the return shape untouched.
- [x] T2 — `RichText.tsx`: `splitBold()` (never inside code spans), contiguous `ul`/`ol`, `#{1,6} ` heading lines → bold paragraph (markers stripped, no new classes); both popover answers now `<RichText text={turn.answer} className="lexical-turn-answer" />` (`:136`, `:360`); CSS `.lexical-turn-answer .rich-text-p { white-space: pre-wrap }` so the answer keeps run fidelity over the component default.
- [x] T3 — `blocks/index.tsx:147-178`: `TheorySelectionAssistant` now wraps ALL 8 block conditionals (verified: every block type still renders inside it); stale "theory only" comments updated in `LexicalAssistantPopover.tsx` and `styles.css:943`.
- [x] T4 — `scripts/check-rendering.tsx`: +15 cases (bold, bold vs `<code>`, `<ol>`, `<ul>`, heading strip, fence regression with literal `**`).

## Verification (no Chrome, per user constraint)
1. `bunx tsc --noEmit` → exit 0 (re-run by parent after the writer).
2. `bun run scripts/check-rendering.tsx` → PASS, **31 checks** (16 pre-existing + 15 new), re-run by parent.
3. Structural readback by parent: hook rules match decisions 1-2; all 8 block types still render inside the wrapper; both answers use `RichText`; `::highlight(popover-selection)` rule present.

## Progress
- 2026-09-29: investigation done, root causes confirmed, design settled, task file created.
- 2026-09-29: T1-T4 implemented by one delegated writer (success); parent re-ran both checks and read back every critical seam.
- **Honest limits**: bugs A, B and D are NOT machine-verified — no test runner covers `selectionchange`/focus behavior and the user forbade browser testing. They rest on typecheck + line-by-line reasoning against the reported scenarios. The Custom Highlight API paints only on Chromium 122+/Safari 18.2+ (Tauri WebView2 qualifies); elsewhere the highlight degrades to today's behavior by design.
- Engram mirror for this feature is PENDING — `mem_save` failed repeatedly this turn with "could not confirm Engram session registration" (transient server/identity issue); the local doc above is the source of truth.

## Next step
Done, uncommitted (standing rule). Runtime smoke test by the user when convenient: focus the input (popover must stay open + highlight persist), select an answer and copy, check a `**bold**`/list answer, and select text in any block type.
