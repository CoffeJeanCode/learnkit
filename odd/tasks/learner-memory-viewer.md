# Visor del perfil cognitivo del alumno (LearnerCognitiveMemory)

## Objective
Add a read-only screen where the user can see the `LearnerCognitiveMemory` data that is already persisted per learner but currently has zero frontend consumer.

## Why
User request (2026-09-29, es): "Añade una forma ver la informacion almacenada sobre feedback del usuario". Clarified via AskUserQuestion: of the three "feedback" concepts found in the codebase (per-block grading feedback, diagnostic battery results, learner cognitive profile), the user picked **the learner cognitive profile** — the one that today has no viewer at all.

## Key facts from prior exploration (do not re-derive)
- `src-tauri/src/domain/learner_memory.rs` — `LearnerCognitiveMemory` struct: calibrated baseline, EMA'd `cognitive_metrics` (prediction accuracy, error-audit detection, preferred abstraction level, friction tolerance), `recurring_misconceptions` (domain concept + error pattern + resolved flag), spaced-retrieval queue.
- Persisted as one JSON blob per learner in the `learner_memory` SQLite table via `src-tauri/src/notebook_store.rs::get_learner_memory` / `save_learner_memory`. Learner id is currently a single hardcoded `"local"` value.
- No Tauri command currently exposes `get_learner_memory` to the frontend — none found in `src-tauri/src/commands/notebook.rs`, none in `src/lib/tauri.ts`.
- Frontend nav has no router: `src/stores/ui.ts` holds `view: View` (zustand), `src/App.tsx:41-56` is a plain switch, `src/components/Header.tsx` renders the nav buttons. Existing full-screen overlay pattern to mirror: the "⚙ Proveedores" button (`Header.tsx:106-108`) toggles `view` to `"providers"` and back via `lastMainView`; `ProvidersView.tsx` is the component to use as a structural/styling reference for a new standalone screen.

## Design decisions (settled)
1. New Tauri command (read-only) that wraps `notebook_store::get_learner_memory` for the `"local"` learner id, returned as-is (no new persistence, no mutation).
2. New `View` variant (e.g. `"learner-memory"`) added to `src/types.ts`, wired the same way `"providers"` is: a header button that toggles it and restores `lastMainView` on close. Do not touch the existing `Sesión`/`Clases` tabs or `inSettings` logic beyond adding the new branch.
3. New component (e.g. `src/features/learner-memory/LearnerMemoryView.tsx`) rendering: baseline + current cognitive metrics, the recurring misconceptions list (flagging resolved vs open), and the spaced-retrieval queue — plain read-only display, no editing.
4. No new database schema, no changes to how the memory is written — this task is purely additive (one query command + one view).

## TDD mode
Off — no unit runner covers view switching in this codebase (see `session-view-filter.md` precedent). Verification: `bunx tsc --noEmit`, `cargo check` (or the project's existing Rust check command) for the new Tauri command, plus parent structural readback.

## Tasks
- [x] T1 — Backend: `get_learner_memory` in `notebook_service.rs` (+`commands/notebook.rs` +`lib.rs` invoke_handler).
- [x] T2 — `src/lib/tauri.ts`: `getLearnerMemory()` wrapper + TS types (camelCase fields, snake_case enum values — verified against Rust `#[serde(rename_all)]`).
- [x] T3 — `src/types/index.ts`: `"learner-memory"` added to `View`. `src/stores/ui.ts` untouched (generic handling already covers it).
- [x] T4 — `src/components/Header.tsx`: "🧠 Memoria" button; `inSettings` generalized to `inOverlay = inSettings || inMemory` so both overlays hide the session chip/tabs consistently.
- [x] T5 — `src/features/learner-memory/LearnerMemoryView.tsx` (new, 146 lines) + `src/App.tsx` wiring. No `styles.css` changes — reuses existing `.card`/`.badge`/`.dod-list`/`.hint-warn` classes.

## Route
T1-T5 → one delegated writer (backend + frontend, 6+ files). Parent verifies with `tsc --noEmit` and a Rust check, plus structural readback.

## Progress
- 2026-09-29: exploration done (3 feedback concepts mapped), user picked the cognitive profile, task file created.
- 2026-09-29: T1-T5 implemented by one delegated writer (`general-purpose`, success, no design deviations from the settled decisions — only an implementation necessity: `inOverlay` generalization in `Header.tsx`, not a design change).
- Parent verification: `bunx tsc --noEmit` re-run by parent → clean (spot check). `cargo check` (writer-reported) → `Finished dev profile ... in 1m52s`, no warnings/errors. Structural readback of `LearnerMemoryView.tsx` and the `Header.tsx`/`App.tsx`/`types/index.ts` diffs — matches design decisions 1-4.
- Known limits: no runtime visual check (Tauri app not launched); RDD/native review not invoked for this candidate.

## Next step
Not committed — following this repo's established precedent ([[session-view-filter]]: user left similar work uncommitted since the branch has other concurrent unrelated changes). Awaiting user decision on whether/how to commit.
