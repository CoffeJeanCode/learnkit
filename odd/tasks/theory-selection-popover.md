# Theory block refactor + text-selection contextual popover

## Objective
1. Refactor the `anchored_micro_theory` block prompt/schema so its 3 layers build a robust mental model: intuitive hook with an explicit analogy-boundary, causal mechanism (paired with the existing separate `declarative_visual_diagram` block when it needs a visual), and an explicit valid-case-vs-misconception contrast.
2. Add a text-selection-triggered contextual popover on theory blocks so a student can highlight any fragment/term and ask a free question anchored to that exact text, with Socratic-only answers (no leaking downstream exercise solutions).

## Why
User request (2026-09-29): pedagogical refactor of theory blocks + a "Selección de Texto y Popover Contextual" feature. See conversation for the full Spanish spec.

## Branch
`feature/theory-selection-popover` (created from `main`)

## Key findings from exploration (do not re-derive)
- Doubts popover **already exists**: `BlockLexicalAssistant` (`src/features/notebook/LexicalAssistantPopover.tsx`) + `ask_lexical_assistant` Tauri command (`src-tauri/src/commands/lexical_assistant.rs`) → `LexicalAssistantService` → `lexical_assistant_agent` (already Socratic, already anti-leak via `redact_leaked_answers()`, already supports free-text follow-up questions and history). It is a per-block button, NOT selection-anchored. **Decision: extend this stack instead of building a parallel `ask_theory_context`/`TheoryContextPopover` system**, to avoid duplicating the anti-leak guard.
- Theory block is `anchored_micro_theory` (not `anchored_exposition`), schema at `src/lib/schemas.ts:354-365` (Zod) and `src-tauri/src/domain/notebook.rs` (`GeneratedSectionBlock::AnchoredMicroTheory`), JSON-schema-for-LLM at `src-tauri/src/tools/notebook_tools.rs:400-406`, prompt at `src-tauri/src/agents/block_generator_agent.rs:202-212` (word cap enforced in `notebook_service/grounding.rs`). Rendered by `src/features/notebook/blocks/AnchoredMicroTheoryBlock.tsx`.
- No Tiptap in this repo (verified in `package.json`) — all prose rendering is a hand-rolled regex parser (`RichText.tsx`). The selection popover must use native `window.getSelection()`/`Range`, not a Tiptap bubble menu.
- **User decision (asked 2026-09-29):** the Capa-2 visual organizer stays a **separate** `declarative_visual_diagram` block (its anti-overlap rules already solved that problem) — do NOT embed a diagram field inside `anchored_micro_theory`. Instead strengthen grounding/prompt so a diagram block is reliably paired after a theory block whose mechanism involves system/architecture state.
- Pending, still-unresolved bug report from an earlier session (2026-09-29 08:30, not yet fixed per current diff): the doubts-popover container is too small, has a rendering issue, and needs a small "lemniscate" thinking animation. There's an unused, ready-made animated component (`ThinkingSketch` in `src/components/AgentLoader.tsx`, spiral SVG doodle) to fork into a lemniscate variant. In scope here since it's the same component being extended.
- Precedent to follow for any new optional field: the codebase has an established bugfix for `zod .optional()` (absent-key only) vs Rust `Option<T>` (accepts explicit `null`) mismatch — check the current convention (likely `.nullable().optional()` on the Zod side) before adding `analogyBoundary`, to avoid repeating that exact bug.
- No Tailwind; custom CSS with theme tokens (`--chalk`, `--paper-deep`, etc.) in `src/styles.css`, `prefers-reduced-motion` block at `styles.css:912-919` must cover any new animation.
- Uncommitted unrelated WIP already on disk (double-escaped-JSON repair in `notebook_tools.rs`, code-span font-size tweak in `styles.css`) — do not touch or revert; leave as-is.

## TDD mode
Unresolved/no explicit project config found. Default: ordinary functional checks (not strict RED-GREEN), matching existing repo convention of adding unit tests alongside behavior. Verification commands: `cargo test --lib` (from `src-tauri/`), `bun run typecheck`, `bun run build` (or repo's actual script names — confirm via `package.json`/`Makefile`).

## Tasks

- [x] T1 — Backend: extend `anchored_micro_theory` schema with an `analogyBoundary` field. DONE — `analogyBoundary?: string | null` (Zod, `.nullish()`) ↔ `analogy_boundary: Option<String>` with `#[serde(default, skip_serializing_if = "Option::is_none")]` (Rust), mirroring the `visual_aid` field's existing convention. Old blocks missing the key deserialize fine. Rendered as a 4th `theory-boundary` paragraph (🚧) only when present. Files touched: `src/lib/schemas.ts`, `src-tauri/src/domain/notebook.rs`, `src-tauri/src/tools/notebook_tools.rs`, `src/features/notebook/blocks/AnchoredMicroTheoryBlock.tsx`, `src/styles.css`.
- [x] T2 — Backend: strengthen `block_generator_agent.rs` prompt. DONE — Capa 1 boundary required (🚧, 1 line); Capa 3 (`frequentError`) now requires explicit "esto SÍ / esto NO, porque..." contrast; word budgets split: `systemRule` own ceiling 160-180 words (`MAX_SYSTEM_RULE_WORDS=180`), other 3 layers combined ≤100 words (`MAX_OTHER_LAYERS_WORDS=100`) in `grounding.rs`. Added pairing instruction: when `systemRule` describes system/architecture state, steer toward a follow-up `declarative_visual_diagram` block (never embedded) — matches user's confirmed decision.
- [x] T3 — Backend: selection-anchored questions on the existing lexical assistant. DONE — no signature change needed to `askLexicalAssistant`/`ask_lexical_assistant`/`LexicalAssistantService::ask`; existing `term`+`fragmentContext` already carry an arbitrary selection. Only a minimal prompt wording tweak in `lexical_assistant_agent.rs` (now explicitly covers "desde una sola palabra técnica hasta una oración completa"). Frontend should pass highlighted text as `term`, surrounding theory-block content as `fragmentContext`.

**Verification T1-T3**: `cargo test --lib` → 183 passed, 0 failed. `bunx tsc --noEmit` → exit 0, no errors.
- [ ] T4 — Frontend: selection-detection hook scoped to the theory block container (native Selection/Range API — no Tiptap), ignoring empty/whitespace selections, closing on outside click/Escape/scroll, computing popover position from `getBoundingClientRect`. New file under `src/features/notebook/`. Route: delegated writer (2+ non-trivial files together with T5).
- [ ] T5 — Frontend: selection-triggered popover UI — reuse/extend `LexicalAssistantPopover.tsx` (quoted fragment preview, "Explicar término" quick action + free-text question input, streaming/loading state with a new lemniscate thinking animation forked from `ThinkingSketch`), wired only onto the theory block. Also fixes the pending bug report: bigger container, rendering fix, animation. Files: `src/features/notebook/LexicalAssistantPopover.tsx` (or new sibling component), `src/features/notebook/blocks/index.tsx` / `AnchoredMicroTheoryBlock.tsx` (mount point), `src/components/AgentLoader.tsx` (new lemniscate variant), `src/styles.css` (sizing, animation, `prefers-reduced-motion`). Route: delegated writer (same as T4).
- [ ] T6 — Verification + commit: run `cargo test --lib`, typecheck/build, manual smoke via `run` skill if feasible; commit as one or more work-unit commits on this branch with Conventional Commit messages.

## Progress
Task file created 2026-09-29. Exploration done. User decision captured (Capa 2 visual stays separate). Implementation not started.

## Next step
Delegate T1+T2+T3 to one backend writer.
