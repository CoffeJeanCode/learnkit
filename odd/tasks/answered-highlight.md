# Keep the assistant's help highlighted after the popover closes

File locator: `odd/tasks/answered-highlight.md` (repo-relative, C:\bi\learnkit)

## Objective
When the student has already received an answer from the assistant for a
selected fragment, closing the popover must NOT erase the highlight. The
highlight stays as a "help is here" mark, and clicking it reopens the same
margin comment with its history intact.

## Why
User request (2026-09-29, es): "Cuando haya una ayuda, debería dejar el
highlight para revisar la ayuda del asistente".

Today `useTextSelectionPopover` paints `::highlight(popover-selection)` only
while its `selection` state is non-null, and every close path
(Escape / outside mousedown / competing selection) calls `setSelection(null)`,
whose effect cleanup runs `registry.delete(...)`. So the moment the student
clicks away, the mark disappears and the only way back is to re-select the
exact same text — and even then the thread is keyed
`selection:${selection.text}`, so history survives only by coincidence of the
same key.

## Constraints
- React owns the DOM → do NOT wrap ranges in `<mark>`/`<span>` by hand; React
  reconciliation will drop manual DOM mutations on the next render.
- `::highlight()` painting is not hit-testable, but the underlying text nodes
  still receive pointer events, so a range membership test on click is enough.
- Highlight API access must stay guarded (already is) — browsers without it
  degrade to no persistent mark, never a throw.
- UI copy Spanish, comments English.

## Scope
IN: `src/features/notebook/useTextSelectionPopover.ts`,
`src/features/notebook/LexicalAssistantPopover.tsx`, `src/styles.css`.
OUT: persistence across restarts (would need a backend table + range
re-hydration) — session-scoped only, unless the user asks otherwise.

## Tasks
- [ ] T1 Hook: add `answered` awareness — when the popover closes and the
      current thread has ≥1 turn, move the range into a `savedSelection`
      state instead of dropping it, and paint `::highlight(popover-selection-saved)`.
- [ ] T2 Hook: reopen on click — when no popover is open, a click whose
      coordinates fall inside any saved range's client rects restores
      `selection` + thread so the comment reopens with its history.
- [ ] T3 Popover: keep `useLexicalAssistantThread` keyed per saved selection so
      the history is not wiped on close/reopen; only reset when the student
      asks about a different fragment or the block unmounts.
- [ ] T4 CSS: add `::highlight(popover-selection-saved)` with a softer,
      "already answered" treatment distinct from the live selection.
- [ ] T5 Escape / selecting new text clears the saved mark (one saved mark per
      block); unmounting the block clears it too.
- [ ] T6 Verify: `bun run build`.

## Acceptance criteria
- Select → ask → answer → click away: the fragment stays highlighted in the
  "answered" style.
- Clicking the highlighted fragment reopens the margin comment with the answer
  still there.
- Selecting a different fragment in the same block replaces the saved mark.
- Escape or leaving the notebook clears it.

## Progress
- [ ] Not started (2026-09-29).
