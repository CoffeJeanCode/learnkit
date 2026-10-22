# Feature: Token optimization — TOON payloads + prompt reduction

## Objective
Cut per-request prompt tokens on the hot paths (block generation, roadmap
turns) two ways, per user directive "mejorar la optimización de tokens,
usando toon, reduciendo prompts":
1. Encode ALL structured data embedded in prompt text as TOON (spec v4.4)
   instead of pretty-printed JSON.
2. Reduce the static prompt texts by removing content that duplicates the
   tool JSON-schemas already sent on every call.

## Root causes / current state (mapper-verified)
- 11 prompt-side `serde_json::to_string_pretty` sites: notebook
  `render.rs` L241/278/317/342/362/380/403, roadmap `render.rs` L78/100/127,
  `lexical_assistant_service.rs` L62. Pretty JSON measured **+34% chars** vs
  compact on a representative 3-block class context (worse as `contentSoFar`
  grows, because of nesting).
- Static floor per block call ≈ **6,010 words** system prompt
  (`block_generator_agent.rs:37-576`) + **~2,000 words** tool schema
  (`notebook_tools.rs` single_block_json_schema L459-667 + visual_aid
  L380-451 + block_template_schemas L672-754) ≈ **8k words resent per block
  attempt** (incl. retries/escalation). Roadmap: 2,841-word prompt
  (`roadmap_agent.rs:20-269`) + ~1.1-1.4k tool schema per turn.
- Duplication: block-generator `# CATÁLOGO DE BLOQUES` (L337-433) and
  `# PLANTILLAS` (L434-462) restate in prose what the tool schema oneOf
  descriptions already carry; roadmap prompt restates tool rules that live
  in `roadmap_tools.rs` descriptions (L88-92, L137-146). Paid twice each call.
- Output side: native tool-call JSON via serde (`notebook_tools.rs:71-83`
  custom Deserialize, 5 specialized factory paths with real tool schemas).
  **Output format must NOT change — TOON is input-only.**
- No token counting/cost utilities exist (`PLAIN_MAX_TOKENS`,
  `output_budget()` are output caps). No history sent for notebook (state =
  JSON re-injected); roadmap sends `draft.messages` (bounded by flow,
  MAX_GATHER_TURNS=2).

## Key facts
- Official Rust crate exists: **`toon-format` 0.7.0** (org `toon-format`,
  repo `toon-format/toon-rust`, spec **v4.4**), API
  `encode<T: Serialize>(&T, &EncodeOptions) -> ToonResult<String>`,
  `encode_default`, `decode_default`. **`default` feature = `cli`** (drags
  clap/tiktoken-rs/ratatui/arboard/syntect) → MUST add with
  `--no-default-features`.
- Providers are BYOK runtime-selected (Anthropic/OpenAI/Gemini/OpenRouter/
  DeepSeek/Xiaomi) — input is plain text, so TOON is safe for all; outputs
  stay tool-JSON for all.
- Prompt-content test guard: only `lexical_assistant_service/tests.rs:30-38`
  asserts literal prompt text (we do NOT touch that prompt).
  `app_state.rs:310/316` compares prompt equality vs definition (safe when
  editing constants in place). Input-shape tests:
  `notebook_service/tests.rs:869` (`contains("el hook es vago")`),
  `:957` (`contains("NOTA_DEL_SISTEMA")` — appended OUTSIDE the payload),
  `:1446` source-lint test (precedent for adding a new lint).

## Decisions
- Helper module `src-tauri/src/prompt_encoding.rs`: `toon_payload<T:
  Serialize>(&T) -> String` = `toon_format::encode` with default options;
  on Err → `tracing::warn!` + **compact** `serde_json::to_string` fallback
  (never pretty, never empty — old `.unwrap_or_default()` could emit "").
- Swap ALL 11 sites (uniformity; mixed formats buy nothing and confuse
  reading of Dev-panel traces).
- Add ONE short TOON orientation line to each input envelope via a shared
  const (`TOON_HINT`), so weaker flash models orient without cost in the
  system prompt.
- Prompt reduction = **migrate-don't-delete**: cut prompt text only where
  the tool schema already carries the same information; if the prompt holds
  required nuance the schema lacks, KEEP it (rules/prohibitions) or MOVE it
  into the schema description (descriptive). Never remove: escalation/close/
  graduation flows, pedagogical rules (incl. everything shipped in
  7f98553: first-block rule, numbered systemRule/240, Regla de Oro), section
  navigation headers, output contract.
- NOT touched (report as out-of-scope levers): `contentSoFar` depth
  (quality), critic/grader prompts (873/677/286 words — small), schema
  shapes, output format, message history (none/already bounded).

## Checklist
- [x] T1a `cargo add toon-format --no-default-features` in src-tauri;
      confirm Cargo.lock does NOT pull clap/tiktoken/ratatui.
- [x] T1b `prompt_encoding.rs`: `toon_payload` (TOON → compact-JSON
      fallback) + `TOON_HINT` const; unit tests: round-trip via
      `decode_default` == `to_value`, strict smaller than pretty on a
      representative array-bearing payload, <= compact.
- [x] T2a Swap the 11 `to_string_pretty` sites; append `TOON_HINT` in each
      input envelope; keep `NOTA_DEL_SISTEMA`/note appends outside payload.
- [x] T2b Verify/adjust input-shape tests (869/957) + add source lint
      forbidding `to_string_pretty` in the 3 prompt-path files (style of
      tests.rs:1446 lint).
- [ ] T3a block_generator_agent.rs: dedupe CATÁLOGO/PLANTILLAS vs tool
      schema under migrate-don't-delete; record before/after word counts.
- [ ] T3b roadmap_agent.rs: dedupe tool-rule restatements the same way.
- [ ] T4 Verify: cargo test, bunx tsc --noEmit, bun run
      scripts/check-rendering.tsx; report word/char deltas per payload.

## Acceptance criteria
- Zero `to_string_pretty` in prompt paths (lint test enforces).
- Round-trip test proves TOON payload decodes to the same Value.
- Representative block input: TOON chars < pretty chars (test-asserted).
- Both system prompts strictly smaller; every rule/prohibition from 7f98553
  still present (grep assertions in tests for the new rules keep passing).
- Output remains tool-JSON (no changes in tools/*.rs Deserialize paths).
- All three verification commands green.

## Constraints / decisions
- TDD off: ordinary functional checks + tests alongside.
- Route: delegated direct (writer trigger: 7-8 non-trivial files).
- Branch: `perf/toon-prompt-optimization` stacked on
  `fix/diagnostic-battery-and-scaffolding` (same test-cycle lineage).
- Delivery: single work-unit commit; forecast is deletion-heavy on prompt
  text so authored lines may approach/exceed the 400-line planning heuristic
  — record actual counts in Evidence; record slice boundary (TOON encoding
  vs prompt-cut) if split commits prove cleaner during implementation.
- Spanish for prompt text/hints (match existing register); English for code
  comments and test names.

## Progress
- [x] Exploration + mapper (prompt anatomy, 11 pretty sites, duplication map)
- [x] Spec/API research (official toon-format 0.7.0, spec v4.4, features)
- [~] Implementation (T1+T2 done; T3a partial: only the retrieval catalog entry deduped; T3b pending)
- [x] Verification of T1/T2: cargo test 312 passed, tsc OK, check-rendering PASS (2026-10-22)

## Evidence
- (to be filled: word counts before/after, payload char deltas, commit hash
  mirrored in Engram `odd/toon-prompt-optimization/tasks`)
