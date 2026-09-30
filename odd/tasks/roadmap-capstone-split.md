# Roadmap capstone split

## Objective
Keep the Gate-3 syllabus tool call under the provider output budget
(DeepSeek hard cap: 8192 output tokens) by moving `capstoneProject` out of
`propose_syllabus` into its own `propose_capstone_project` tool call.

## Problem
The roadmap agent failed with `finish_reason=Length`: one tool call carried the
full milestone tree plus the capstone project.

## Scope
- New tool `propose_capstone_project` (`CapstoneProjectArgs`), registered in
  `DiagnosticToolScope::Propose` and `::All`.
- `RoadmapCapture.propose_capstone_project` field.
- `capstoneProject` removed from `syllabus_json_schema`; raw args keep
  `serde(default)` so older payloads still parse.
- `flow.rs` merges both captures only once both are present; a missing capstone
  is a grounding rejection that re-asks for it.
- Prompt and tests updated.

## Out of scope
`total_weeks` is uncapped, so a very long course can still overflow. The fix
would be batching milestones, which is a separate feature.

## Tasks
- [x] T1 — Finish the split (tool, capture, merge, prompt, tests). Route: delegated writer (9 files, trigger: writer).

## Checks
- `cargo test` (src-tauri) — 199 passed; 0 failed; 0 ignored (baseline 196 + 3 new tests: `propose_capstone_project_writes_into_the_shared_capture`, `propose_syllabus_plan_schema_no_longer_embeds_capstone_project`, `capstone_project_schema_requires_its_3_fields`).
- `node node_modules/typescript/bin/tsc --noEmit` (repo root) — clean, no output, exit 0.

## TDD
Off (no project or session configuration). Ordinary functional checks.

## Progress
- `c830ee0` (separate work on the same branch): the notebook generator and the critic now receive the content of prior blocks.
- The first writer attempt stopped on a rate limit. Partial edits compiled under `cargo build`, but `cargo test` failed with `missing field propose_capstone_project` in `harness::ScriptedStep` (4 call sites).
- Second writer pass (this one) found the core split (tool, schema extraction, `RoadmapCapture` field, flow.rs merge/rejection, factory.rs/tools/mod.rs wiring, roadmap_agent.rs prompt) already correctly implemented by the first writer. Remaining work:
  - Added `propose_capstone_project: None` to the 2 bare `ScriptedStep` literals in `tests/gates.rs` (lines 7, 76) that were missing the new field.
  - Added `propose_capstone_project: Some(full_capstone_args())` to the 2 `ScriptedStep` literals in `tests/sessions.rs` (lines 35/39-46, 184/188-195) whose embedded `propose_syllabus_plan` expected a successful seal — without a capstone capture, flow.rs's new "missing capstone" branch rejects the proposal, so these would have stalled instead of sealing.
  - Fixed 4 pre-existing rejection tests in `tests/rejections.rs` that regressed once the capstone-missing check landed: `a_monolithic_micromodule_over_5_hours_is_rejected`, `a_micromodule_without_an_objective_is_rejected`, and `a_micromodule_deliverable_with_other_type_and_a_too_short_description_is_rejected` each needed `propose_capstone_project: Some(full_capstone_args())` added so grounding reaches `syllabus_violations` and reports the SPECIFIC violation under test, instead of short-circuiting on the generic "capstone missing" rejection. `a_capstone_project_missing_verifiable_evidence_is_rejected` was rewired to drive its missing-evidence case through `propose_capstone_project` (a new `CapstoneProjectArgs` with empty `verifiable_evidence`) instead of the syllabus's own now-vestigial nested `capstone_project` field, since that field is no longer where the model-supplied capstone actually arrives.
  - Found and fixed a real bug in `tests/harness.rs`: `SeqRunner::run_diagnostic_execution` never copied `step.propose_capstone_project` into the capture (unlike `ScriptedRunner`/`FlakyRunner`, which did it correctly). This silently dropped the capstone on every `SeqRunner`-scripted propose step, which starved `retry_recreates_the_propose_turn_after_the_last_answer_died_mid_model`'s scripted-step queue (the dropped capstone triggered flow.rs's internal grounding-retry loop, which tried to pop a 3rd step that was never queued) — panicked with "scripted step available". Fixed by adding the missing field assignment.
  - Verified the frontend TS mirror (`src/lib/schemas.ts`'s `RoadmapSyllabusPackageSchema`) needs no change: the split only affects the wire format of the two backend tool calls, not the merged `syllabus` shape returned to the frontend, which still always carries a populated `capstoneProject`.
- Nothing committed, staged, or pushed. `Cargo.toml` and `gen/schemas/*.json` were left untouched (pre-existing line-ending diffs only).
