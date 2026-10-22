# Token efficiency of LLM calls (restructure, not trim)

Branch: `perf/token-efficiency` (stacked on `test/agent-loop-verification`) · Created: 2026-10-22

## Objective
Cut input tokens per LLM call by deduplicating and restructuring prompts, tool schemas and per-call
input, WITHOUT losing pedagogical rules. Every reduction is measured (bytes/est. tokens) before and after.

## Evidence (audit, estimates at ~4 bytes/token)
- notebook_generator prefix per call: system prompt ~10.6k + `publish_notebook_block` schema ~6.9k tokens,
  re-sent on each of up to 3 attempts. `visual_aid_json_schema()` (2.5 KB) is expanded 5x inline.
- Same rule stated up to 5x across prompt, schema, validator (word caps, isOptimal, no consecutive blockType).
- Roadmap Battery/Propose/All scopes all send the full 19.5 KB roadmap prompt.
- No cache markers for Anthropic; real token usage is not logged anywhere (rig exposes `Usage`).
- `render_turn_input` directive says `confirm_syllabus_plan` takes "SOLO el notebook" but the tool has no params.

## Verification mode
TDD off (unconfigured). Runner `cargo test --lib` in `src-tauri`. Delivery: ask-on-risk; ~400 lines/task heuristic.

## Out of scope (second round, needs real numbers first)
Digesting `contentSoFar`; splitting the generator prompt by mode (breaks the cacheable prefix);
dropping `proposed_plan` from the confirm turn (revisions need the prior plan); lexical history cap.

## Tasks
- [ ] T1 Baseline: log real `Usage` per model turn (hook `on_model_turn_finished`) into dev trace + e2e summary; add an `#[ignore]` size-report test printing prefix bytes/est. tokens per agent and scope
- [ ] T2 Anthropic prompt caching on stable prefix (rig `with_automatic_caching` / static prefix), all Anthropic arms
- [ ] T3 Tool schema dedupe: `visualAid` and template sub-schemas defined once; verify provider support for `$ref/$defs` (Gemini!) before using it, else dedupe structurally; shorten repeated field descriptions
- [ ] T4 Prompt dedupe: each rule stated once in model-visible text (+ validator); fix stale "SOLO el notebook" directive
- [ ] T5 Roadmap: stop duplicating Gate 1/identity text between analyst and roadmap prompts; hoist TOON_HINT into the system prompts
- [ ] T6 Final size report vs baseline; docs

## Route declaration
Delegated direct, one writer (2+ non-trivial files, many files).

## Progress / evidence
(none yet)

## Next step
Delegate writer.
