# Feature: Diagnostic battery honesty + beginner scaffolding sequence

## Objective
Real-user test ("Quiero montar mi primer servidor, y no tengo nada de
 conocimiento") exposed two UX/pedagogy failures:
1. A synthetic template battery ("¿qué asocias primero con {topic}?" with
   self-perception options) was shown to a declared beginner — and graded
   "No tengo ninguna intuición" as WRONG. Topic in the profile card was the
   raw chat sentence.
2. The first Linux class opened cold with `pwd` and immediately evaluated
   edge cases/-errors the student had never seen practiced (theory abrupt,
   evaluation before happy-path practice).

## Root causes (mapper-verified, file:line)
- `synth.rs:89-116` `synth_diagnostic_battery` — 3 hardcoded template
  questions, subjective options, "correct" = upbeat answer. Sole caller:
  `flow.rs:509` inside `force_close_session`.
- `synth.rs:5-31` `synth_profile_card` — topic = raw last message (≤80ch),
  no prefix stripping; `target_goal`/`entry_level` hardcoded.
- `block_generator_agent.rs` — no first-block rule against isolated-command
  openings; systemRule cap text says "160-180 palabras" (L344); no golden
  progression rule (theory → happy-path practice → THEN edge cases).
- `notebook_service/grounding.rs:30` `MAX_SYSTEM_RULE_WORDS = 180`;
  no positional guardrail against `heuristic_error_audit` right after the
  first theory. NOTE: call site fuses `is_escalation || is_regeneration`
  into ONE bool (generation.rs:1068-1072) — grounding cannot distinguish them.
- `lesson_composition.rs` — some `suggested_sequence` chains place
  HeuristicErrorAudit right after theory (hint-only, not enforced, but it
  feeds `compositionHint` shown to the model).
- `pedagogical_critic_agent.rs` — no "real incrementality" transversal check.
- Frontend `DiagnosticBatteryCard.tsx:54` already returns null when no
  battery; `DiagnosticBatteryStage` is mounted only when a pending battery
  exists (RoadmapView.tsx:504) → NO frontend change needed for task 1.

## Decisions (minimal-loss, honest-state-first)
- NEVER synthesize a battery: delete `synth_diagnostic_battery`; force-close
  seals with `diagnostic_battery: None` (matches `force_progress_from_gate3`
  which already passes through None). Silence beats nonsense.
- Guardrail for audit-after-first-theory is POSITIONAL (prior == exactly
  [AnchoredMicroTheory]) and exempt when the fused escalation/regeneration
  flag is set — otherwise in-place repair of a pre-existing audit would
  loop forever (required type == audit vs new rule).
- `hands_on_mission` in that position is ALREADY guarded (guided|faded +
  template required, grounding.rs:204-227) — verify, don't duplicate.
- MAX_SYSTEM_RULE_WORDS 180 → 240 per explicit request.
- systemRule renders via RichText (lists) instead of InlineText so the
  required numbered micro-steps display as steps.
- zod schema gets `.describe()` parity (no shape change).

## Checklist
- [x] T1a Delete `synth_diagnostic_battery`; force_close seals battery=None;
      English comment explains the real-user failure. (Also removed the now
      unused `DiagnosticQuestion` import at roadmap_service.rs:5; tests/mod.rs
      imports it explicitly.)
- [x] T1b `clean_topic` helper in synth.rs (strip leading intent verbs +
      trailing skill disclaimers, trim punctuation, capitalize, ≤80ch,
      fallback to raw); 5 unit tests. WRITER DEVIATION (accepted): compound
      verb prefixes (`quiero aprender ` etc.) were DROPPED from the strip
      table — they would eat the action verb ("Quiero montar mi primer
      servidor" → "Montar mi primer servidor" needs only `quiero `). Volition
      prefixes + 12 disclaimer clauses kept.
- [x] T1c roadmap_agent.rs battery rules: psychometric rule 4 "cero
      autopercepción" (L110-117) + prerequisite-calibration paragraph (L119-124);
      absolute_zero skip rule untouched.
- [x] T1d gates.rs force-close tests now assert `pending_diagnostic_battery`
      None (+ course-level None in the first). No pre-existing test asserted
      synth text (grep confirmed).
- [x] T2a block_generator_agent.rs: first-block no-cold-command rule (L231-237),
      systemRule ≤240 numbered micro-steps with input/output example (L347-355),
      Regla de Oro de progresión after rule 9 (L261-266).
- [x] T2b notebook_tools.rs schema descriptions parity (L490/494: 240,
      numbered steps); JSON shape unchanged.
- [x] T2c grounding.rs MAX_SYSTEM_RULE_WORDS=240; tests 180→240/241; doc
      "~180"→"~240" at domain/notebook.rs:431.
- [x] T2d AnchoredMicroTheoryBlock.tsx systemRule → RichText (import from
      `./RichText` — the components/reading path from the design does NOT
      exist); wrapper `<p>`→`<div>` (block content in p = invalid HTML).
      schemas.ts `.describe()` parity, no shape change.
- [x] T3a grounding.rs positional guardrail (L163-186): prior==[Theory] exactly,
      audit, fused escalation/regeneration flag false → user's verbatim message;
      English comment (positional rationale + in-place-repair exemption +
      pointer to existing mission_support_violations coverage — verified no
      hole, no duplicate rule). 2 tests (a+b / c+d incl. position-zero).
- [x] T3b lesson_composition.rs: NO chain actually places audit right after
      theory (writer verified all profiles: CyclicSystemic has a diagram
      between, QuantitativeMath a gate, SoftwareDebugging's audit is position
      0) — doc comment updated with the golden invariant (L81-89) instead;
      tests unchanged because no chain changed.
- [x] T3c pedagogical_critic_agent.rs "INCREMENTALIDAD REAL" after
      MONOTONICIDAD (L73-79), sibling RECHAZO format.
- [x] T4 Verify: cargo test 308 passed/0 failed (parent spot-check re-ran the
      suite independently — same result); bunx tsc --noEmit exit 0;
      bun run scripts/check-rendering.tsx PASS.

## Acceptance criteria
- Force-closed session → pending_diagnostic_battery None; no template text
  anywhere in src-tauri (grep "Asocias primero", "variable clave").
- synth topic: "Quiero montar mi primer servidor, y no tengo nada de
  conocimiento" → "Montar mi primer servidor".
- class [theory] + audit candidate → rejected with the specified message;
  [theory, gate] + audit → accepted; same-position regeneration exempt.
- 240-word systemRule passes; 241 rejected.
- All three verification commands green.

## Constraints / decisions
- TDD off (not configured): ordinary functional checks + tests alongside.
- Route: delegated direct (writer trigger: 8+ non-trivial files).
- Branch: fix/diagnostic-battery-and-scaffolding stacked on
  fix/notebook-block-salvage (both retest together with a real user).
- Delivery: single work-unit commit, Conventional Commit. <400 authored lines.
- Spanish for model-prompt text + user-facing violation messages (they are
  already Spanish); English for code comments and test names.

## Progress
- [x] Exploration + mapper (8 reality-mismatches resolved above)
- [x] Implementation (delegated direct, one writer; route trigger: 8+
      non-trivial files)
- [x] Verification (writer: 3 commands green; parent spot-check re-ran
      `cargo test` → 308 passed/0 failed)

## Evidence
- Authored: +303/−61 across 14 files (under the 400-line delivery budget;
  single work-unit commit).
- `cargo test` (src-tauri): 308 passed, 0 failed (new: 5 clean_topic unit
  tests, 2 grounding guardrail tests; updated: 2 word-ceiling tests,
  2 force-close battery tests).
- `bunx tsc --noEmit`: exit 0. `bun run scripts/check-rendering.tsx`: PASS.
- Template strings eradicated: grep "variable clave" → 0; synth battery fn
  gone (sole caller flow.rs:509 now seals None).
- Commit hash recorded in Engram mirror
  (`odd/diagnostic-battery-and-scaffolding/tasks`) after freeze — deliberately
  not self-referenced in this file.
