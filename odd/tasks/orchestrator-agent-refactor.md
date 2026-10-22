# Orchestrator agent refactor (terminal tools, validators, prompts)

Branch: `perf/toon-prompt-optimization` · Created: 2026-10-22

## Objective
Remove the production failures found in the roadmap / notebook_block / pedagogical_critic logs:
`MaxTurnsError` loops (+100s per block), critic fail-open that drops rejections, an impossible
hours/sessions rule that force-closes sessions, and false-positive option-length rejections.

## Authorized scope
- `src-tauri/src/providers/factory.rs`, `tools/*`, `orchestration/orchestrator.rs`,
  `application/notebook_service/generation.rs`, `application/roadmap_service/{grounding,synth}.rs`,
  `domain/pedagogy_guardrails.rs`, `domain/roadmap.rs` (docs), `agents/roadmap_agent.rs`,
  `tools/roadmap_tools.rs` (schema), related tests, `docs/flujos-agente-y-generacion.md`.
- Out of scope: frontend, other providers' budgets, the 15% battery rule, notebook_generator and
  pedagogical_critic prompt text (only the roadmap prompt section was supplied).

## Verification mode
- TDD: **off** (source: unconfigured; tests present do not enable it). Runner: `cargo test --lib`
  from `src-tauri`. Behavior changes still ship with tests.
- Delivery strategy: `ask-on-risk` (default). Forecast: ~450 authored lines.

## Evidence (mapper report, verified against code)
- Loop: every flow runs `agent.prompt(..).max_turns(N)`; rig 0.42 re-calls the LLM after each tool result.
- Rig 0.42 exposes `AgentHook` (`on_tool_call` -> `Skip/Stop`, `on_tool_result` -> `Stop`) via
  `PromptRequest::add_hook`. A `Stop` returns `Err(PromptError::PromptCancelled { reason, .. })`.
- Captures are plain overwrite (last call wins), so a second `publish_notebook_block` overwrites the first.
- Critic `Err` arm (`generation.rs` ~1124) ignores the retained capture and fails open.
- Validator: `micromodule_violations` requires exactly 2 sessions, <=4h each, spread <=1h, sum == pace
  (tolerance 0.5h). 14h/week is unsatisfiable. `synth.rs` fallback hard-codes 2 sessions.
- Option length: `OPTION_LENGTH_TOLERANCE = 0.20` in `pedagogy_guardrails.rs`.

## Decisions
- Session count per week: `min = max(2, ceil(pace/4))`, `max = max(5, min)`. The brief's cap of 5
  makes weeks over 20h unsatisfiable, and the profile allows up to 80h.
- Spread: validator tolerance 1.5h (as briefed); prompt target stays 1h so the model has margin.
- Terminal tools are scope-aware completeness checks on the capture, not on a single tool name.
- A hook stop is mapped to success only when it carries our own stop reason.

## Tasks
- [x] T1 Terminal-tool stop hook + first-call-wins (factory.rs, tools/hooks.rs): all five flows; `PromptCancelled` mapped to success
- [x] T2 Critic salvage before fail-open (orchestrator.rs, generation.rs)
- [x] T3 Session-count rule: `grounding.rs`, `synth.rs`, `roadmap_tools.rs` schema, tests
- [x] T4 Hybrid option-length rule (25% AND >25 chars) in `pedagogy_guardrails.rs`, tests
- [x] T5 Roadmap prompt: replace the hours section and the other "2 sesiones" mentions; docs line 160

## Route declaration
- T1-T5: delegated direct, one writer (mandatory trigger: 2+ non-trivial files, 4+ files touched).

## Progress / evidence
- T1 `31a7e1c`: `cargo test --lib` 321 passed, 0 failed.
- T2 `9845403`: `cargo test --lib` 324 passed, 0 failed.
- T3 `4adf151`: `cargo test --lib` 331 passed, 0 failed.
- T4 `cc83fb9`: `cargo test --lib` 334 passed, 0 failed.
- T5 (this commit): `cargo test --lib` 334 passed, 0 failed (prompt/docs only, no pinned prompt text). `cargo clippy --lib` could not run (build script hit os error 32, file locked).

## Next step
Review the branch; push/PR remain the user's decision.
