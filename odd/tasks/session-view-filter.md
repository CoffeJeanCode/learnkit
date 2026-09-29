# Vista única Sesión: filtro Conversación | Plan

## Objective
Collapse the topbar's `Conversación` and `Plan` sections into ONE view with an internal filter that shows either the conversation or the plan (2 states, mutually exclusive). `Clases` stays a separate section.

## Why
User request (2026-09-29): "Deja el plan y conversación en una misma vista, la diferencia es que va haber un filtro para mostrar la conversación o solo el plan".
User decision (question): **filter domain = 2 states** (`Conversación | Plan`), not 3.

## Key facts from exploration (do not re-derive)
- Topbar has **3** tabs today: `Conversación | Plan | Clases` — `src/components/Header.tsx:71-98`; flags at `Header.tsx:26-34`.
- View state is zustand `useUi` (`src/stores/ui.ts:9-26`): `view: View`, `lastMainView: MainView = roadmap|plan|notebook`. No router, no URL.
- Render switch: `src/App.tsx:45-53` — `providers → notebook → plan → (catch-all) RoadmapView`.
- Live conversation = `src/features/roadmap/RoadmapView.tsx` (NOT `ChatView.tsx`, which + `AgentsView`/`WorkflowsView` are dead code — out of scope, do not delete).
- Session plan = `src/features/notebook/SessionPlanView.tsx` (container) → `SessionPlanTab.tsx`.
- Same session scope: `useRoadmap` holds `session` + `messages`; `SessionPlanView:17,24` reads `useRoadmap(s => s.session)` → no data-model change needed.
- Existing `setView("plan")` / `setView("roadmap")` call sites (just-sealed jump `RoadmapView:159-185`, `SessionsDrawer:68,103`, cross-view buttons) become **filter switches** for free if `"roadmap"`/`"plan"` remain the two filter states.

## Design decisions (settled — do not relitigate)
1. **Reuse `"roadmap"` and `"plan"` as the two filter values.** No store/type change; every existing `setView` caller keeps working and now switches the filter.
2. **New component `src/components/SessionView.tsx`**: renders a filter row + (`view === "plan"` ? `<SessionPlanView/>` : `<RoadmapView/>`).
3. **Filter row is its own `.view` sibling** (do NOT wrap the children — `RoadmapView` already renders `.view roadmap`, `SessionPlanView` renders `.view plan`; nesting `.view` would double the wrapper). Reuse the existing `.topbar-tabs` + `.tab` chip styles; mirror `Header`'s `aria-current` pattern.
4. **Header collapses to 2 tabs**: one merged tab labelled **`Sesión`** (covers both) + `Clases`. The merged tab's active state covers `view === "roadmap" || view === "plan"`; its click keeps the current filter when already inside the session view, otherwise resumes `lastMainView === "plan" ? "plan" : "roadmap"`.
5. **`Plan` chip disabled when there is no session** (`session === null`), mirroring `Header.tsx:38`'s `hasSession` gate; if `view === "plan"` and session is null, fall back to `roadmap`.
6. Leave alone: dead views, `justSealed` auto-jump (it already calls `setView("plan")` → now flips the filter), drawer jumps, scroll/sticky-composer coupling (children stay direct descendants of `.workspace`, so `closest(".workspace")` and the sticky `.composer.dock` keep working).

## TDD mode
Off — no unit runner covers view switching. Verification: `bunx tsc --noEmit`, `bun run scripts/check-rendering.tsx`, plus parent structural readback of the diff (seams listed above).

## Tasks
- [x] T1 — `src/components/SessionView.tsx` (new, 52 lines): `nav.view.topbar-tabs.session-filter` with `Conversación`/`Plan` chips (`aria-current="page"`), `Plan` disabled when `session === null`, `inPlan = view === "plan" && hasSession`, child = `SessionPlanView | RoadmapView`.
- [x] T2 — `src/App.tsx`: `view === "roadmap" || view === "plan" ? <SessionView/>`; `providers`/`notebook` branches and the catch-all untouched; `SessionPlanView` import swapped for `SessionView`.
- [x] T3 — `src/components/Header.tsx`: one `Sesión` tab (`inSession` covers both views), `openSession` keeps the current filter or resumes `lastMainView`; dead `inConversation`/`inPlan`/`openConversation`/`openPlan` removed; stale comments updated.
- [x] T4 — `src/styles.css`: 4-line `.session-filter { margin-bottom: 14px; }` only (other session's 92-line diff untouched — verified via targeted diff).

## Route
T1-T4 → one delegated writer (4 files, one new). Parent verifies with typecheck, render checks and diff readback.

## Progress
- 2026-09-29: exploration done, filter domain decided (2 states), design settled, task file created.
- 2026-09-29: T1-T4 implemented by one delegated writer (`general`, success, no design deviations).
- Parent verification (re-run by parent, not just reported): `bunx tsc --noEmit` → exit 0; `bun run scripts/check-rendering.tsx` → PASS (16 `[ok]`); structural readback of `SessionView.tsx`, the `App.tsx`/`Header.tsx` diffs and the `.session-filter` CSS hunk — all match design decisions 1-6.
- Known limits: no runtime visual check (app needs the Tauri/backend runtime); the concurrent session's hunks in `styles.css`, `check-rendering.tsx` and other files were left untouched.

## Next step
**User decision (2026-09-29): leave it uncommitted — no commit, no review for now.** Work stays verified in the worktree. When the concurrent session (`feature/theory-selection-popover`) finishes, decide how to split both works into commits — note my `.session-filter` hunk in `src/styles.css` shares the file with their ~92 lines, so staging must be selective. Deferred: runtime visual check in the real app.

