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
- [x] T1 Baseline: real `Usage` per model turn (hook `on_model_turn_finished`) into tracing + dev trace; `#[ignore]` size-report test. Commit 96230f9. (Totals NOT accumulated into PromptOutput/AgentOutput: 8+ construction sites, deferred.)
- [x] T2 Anthropic prompt caching on the stable prefix, all 6 Anthropic arms. Commit e41a348.
- [x] T3 Tool schema dedupe (no `$ref`; see finding). Commit 10b7188.
- [x] T4 Prompt dedupe + stale "SOLO el notebook" directive fixed. Commit e89a29e.
- [x] T5 Roadmap Gate 1 text dropped (32ca4ed); TOON_HINT hoisted into the 7 system prompts (c2784a0).
- [x] T6 Final size report vs baseline; docs.

Verification: `cd src-tauri && cargo test --lib` observed green after every task: 343 passed after T1, 344 after T2, 345 after T3, 345 after T4/T5a, 346 after T5b (1 ignored = the size report). Not run: live e2e (needs API keys).

## Route declaration
Delegated direct, one writer (2+ non-trivial files, many files).

## Progress / evidence
### Size report: BASELINE (T1) vs AFTER (T6)
Bytes; tokens = bytes/4; prefix = system prompt + serialized tool definitions. Run: `cargo test --lib size_report -- --ignored --nocapture`.

BASELINE:
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
AFTER (TOON hint now inside `sys B`; the 165 B per-input copy is gone):
```
agent [scope]                          sys B   sys tok   tools B tools tok prefix tok
notebook_generator                     41825     10456     14690      3672      14128
pedagogical_critic                      5899      1474       524       131       1605
notebook_gate_grader                    4796      1199      1391       347       1546
closure_feedback_grader                 2009       502       825       206        708
lexical_assistant                       2347       586         0         0        586
diagnostic_analyst [Assessment]         3827       956       933       233       1189
roadmap [All]                          17050      4262      5836      1459       5721
roadmap [Assessment]                   17050      4262       933       233       4495
roadmap [Battery]                      17050      4262       996       249       4511
roadmap [Propose]                      17050      4262      3621       905       5167
```
DELTAS:

| agent [scope] | prefix tok before | after | delta | sys B before->after | tools B before->after |
|---|---|---|---|---|---|
| notebook_generator | 17045 | 14128 | -2917 (-18%) | 42208->41825 | 25975->14690 |
| pedagogical_critic | 1567 | 1605 | +38 (+2%) | 5746->5899 | 524->524 |
| notebook_gate_grader | 1502 | 1546 | +44 (+2%) | 4622->4796 | 1391->1391 |
| closure_feedback_grader | 664 | 708 | +44 (+6%) | 1835->2009 | 825->825 |
| lexical_assistant | 543 | 586 | +43 (+7%) | 2173->2347 | 0->0 |
| diagnostic_analyst [Assessment] | 1146 | 1189 | +43 (+3%) | 3653->3827 | 933->933 |
| roadmap [All] | 6886 | 5721 | -1165 (-17%) | 19457->17050 | 8090->5836 |
| roadmap [Assessment] | 5097 | 4495 | -602 (-12%) | 19457->17050 | 933->933 |
| roadmap [Battery] | 5299 | 4511 | -788 (-15%) | 19457->17050 | 1742->996 |
| roadmap [Propose] | 6146 | 5167 | -979 (-16%) | 19457->17050 | 5129->3621 |

The small agents' prefix grows by ~43 tok because the TOON hint (~44 tok) moved from every user input into the (cacheable) system prompt; per call the total is neutral and the user-input side shrinks by the same amount. The generator runs up to 3 attempts per block, so its -2917 tok is paid per attempt.
Not measured (needs live calls): real billed tokens and cache hit rate; read `model_turn_usage` logs / dev-trace notes (`kind=usage`) from a live run.

### T2 note
Anthropic arms use `with_automatic_caching()` + `with_static_prefix_cache_ttl(OneHour)` (helper `anthropic_cached` in providers/factory.rs). Unit-tested only on the builder flags; real caching behavior is unverified without a live Anthropic call.

### T3 finding ($ref / $defs)
Not used. rig-core 0.42: Anthropic passes schemas through (it only resolves refs for strict tools); OpenAI/OpenRouter/DeepSeek/Xiaomi pass through unmodified, so `$ref` support depends on the backend (unverifiable offline for DeepSeek, Xiaomi and OpenRouter-routed models); Gemini inlines refs but its `Schema` conversion reads only the FIRST `oneOf` variant (and top-level `properties` if present), so hoisting `visualAid` to the top level would change what Gemini sees. Done instead: full `visualAid` schema kept once (declarative_visual_diagram), the 4 optional ones use a natural-language pointer; shared label/feedback/consequence hints declared once; accepted arguments unchanged (deserialization is Rust-side). Side note, pre-existing and untouched: Gemini sees only the first oneOf variant of `publish_notebook_block`.

### T4 / T5 rule consolidation (old locations -> single remaining location)
- "No same blockType twice in a row" (generator prompt x5: COMPOSICIÓN intro, "Nunca el mismo...", closing of the 5 profiles, RETO DE TRANSFERENCIA exception, catalog footer, PLANTILLAS footer) -> one rule statement in COMPOSICIÓN DINÁMICA (+ validator); the transfer exception and the template-equivalence paragraph remain as references, not restatements.
- Exactly one `isOptimal` (generator prompt catalog 3, tool schema `branches`, critic prompt) -> tool schema `branches` description (+ grounding.rs validator). Critic keeps only its judgment criterion (the marked branch is genuinely best).
- Word caps (systemRule 240, other layers 100) (prompt catalog 1 + schema variant/field descriptions) -> generator prompt catalog 1 only (+ grounding.rs). Schema layer fields keep only a short label.
- Per-layer content rules of anchored_micro_theory (prompt vs schema field descriptions) -> prompt only.
- No-visualAid-in-theory + pairing rule (prompt catalog 1 and COMPOSICIÓN) -> COMPOSICIÓN pairing rule; catalog entry is a one-line pointer.
- visualAid shape (5 variants x 2.5 KB) -> one full schema; optional ones carry "Opcional. Mismo shape que...".
- Gate variant descriptions (prediction gate modes, branching/audit/mission visualAid-optional sentences, supportLevel/isTransfer rules, spaced retrieval) -> prompt only (catalog, ANDAMIAJE rule 9, RETO DE TRANSFERENCIA, MEMORIA COGNITIVA). Stale "closure obligatorio entre el bloque 3 y el 5" removed from the closure schema (contradicted the no-fixed-count design; no validator enforces it).
- Roadmap deliverable `other` rule / `description` rule / vague-verb ban (prompt + schema) -> schema; prompt keeps the type catalog and examples. `objective`, `focus`, `weeklyGoal`, milestone `deliverable` (prompt + schema) -> prompt (richer); schema keeps types only. Capstone field rules -> prompt. Micromodule count/hours/sum (prompt + schema) -> prompt DOSIFICACIÓN. `interactiveBlocks` "student never sees" (prompt x2 + schema) -> CERO META-LENGUAJE section.
- Battery psychometric rules (prompt + `present_diagnostic_battery` description + schema `options`/`dimension`/`diagnosticInsight`) -> prompt Compuerta 2 / REGLAS PSICOMÉTRICAS.
- propose_syllabus_plan / propose_capstone_project ordering ("misma turno", x3) -> prompt; confirm_syllabus_plan "only after confirmation / no params" (prompt + tool) -> tool description.
- Roadmap Gate 1 section (~2.1 KB) and the monosyllabic-answers section -> replaced by a 4-line "already done, never re-ask" note: roadmap agent is only invoked (All/Battery/Propose) after the profile exists (`turn_agent_and_scope`); the analyst prompt keeps Gate 1.
- TOON_HINT (10 sites in user inputs) -> once per TOON-consuming system prompt (7 agents) via `with_toon_hint`; all 7 built-ins are re-registered from code on every startup (app_state.rs), so persisted prompts cannot go stale.
- Stale directive fixed: `render_turn_input` no longer says `confirm_syllabus_plan` takes "SOLO el notebook de la primera clase".

## Next step
Review and decide on push/PR (delivery is the user's). Live e2e with real keys would confirm the usage logs, cache hits and that no pedagogical regression appears.
