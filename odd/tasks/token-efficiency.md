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
### T1 BASELINE (size report, bytes; tokens = bytes/4; prefix = system + tools)
Run: `cargo test --lib size_report -- --ignored --nocapture`
```
agent [scope]                          sys B   sys tok   tools B tools tok    TOON B prefix tok
notebook_generator                     42208     10552     25975      6493       165     17045
pedagogical_critic                      5746      1436       524       131       165      1567
notebook_gate_grader                    4622      1155      1391       347       165      1502
closure_feedback_grader                 1835       458       825       206       165       664
lexical_assistant                       2173       543         0         0       165       543
diagnostic_analyst [Assessment]         3653       913       933       233       165      1146
roadmap [All]                          19457      4864      8090      2022       165      6886
roadmap [Assessment]                   19457      4864       933       233       165      5097
roadmap [Battery]                      19457      4864      1742       435       165      5299
roadmap [Propose]                      19457      4864      5129      1282       165      6146
```
Usage telemetry: `StopWhenCaptured::on_model_turn_finished` logs `model_turn_usage` (tracing::info + dev-trace note kind=usage). Not accumulated into PromptOutput/AgentOutput (PromptOutput is built at 8+ sites; skipped as a large refactor); e2e summary line now points to the per-turn trace.

## Next step
Delegate writer.
