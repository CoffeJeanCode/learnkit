# Feature: Salvage captured tool output + stop deterministic MaxTurns retries

## Objective
Session log 4bf58944 showed ~142s of wasted model compute and 0 blocks generated:
rig's `MaxTurnsError` fires WHILE PREPARING the next turn — after the model's
`publish_notebook_block` / `submit_diagnostic_assessment` tool call already
EXECUTED and wrote into the shared capture — yet every layer returned `Err`
before reading the capture, discarding the deliverable. Retries resent the
identical input (classified "transient"), deterministically failing again.

## Root causes (verified with code refs)
1. Capture discarded on `Err`: `factory.rs:416` (notebook), `factory.rs:307-313`
   (diagnostic), `generation.rs:981` (`?` before reading capture), `flow.rs:63-75`
   (fresh capture per attempt, read only on Ok).
2. MaxTurnsError-with-no-publish classified as transient → blind resend with
   UNCHANGED input (`loop_runner.rs:110-115` sets no corrective note) →
   deterministic failure ×5.
3. Trace contradiction: last-attempt transient error traced as
   `failed_permanently` (`loop_runner.rs:116-121`) even though `is_transient()`.
4. Duplicate full loops: no backend outer retry exists; second loop is a second
   external invoke (React StrictMode double-mount / Reintentar / prefetch)
   against a still-empty document — both chains burn full budgets.

## Why it matters (user-visible)
- Notebook: same-concept duplicate micro-theories (blocks re-generated because
  the first successful publish was thrown away); class stalled at block 2.
- Roadmap: session force-closed with synthetic values (`synth.rs` placeholder
  goal, totalWeeks=4 instead of 8) even though the real assessment WAS submitted
  in a prior turn.

## Scope (minimal-loss, per user's established preference)
- Salvage on Err in BOTH agent paths (notebook block + diagnostic).
- MaxTurnsError-with-empty-capture → treated as EmptyAttempt so the existing
  corrective note breaks determinism (notebook loop only).
- Trace verdict honesty: `retries_exhausted` vs `failed_permanently`.
- Per-document in-flight guard so duplicate invokes can't run two chains.
- NOT in scope: prompt text for salvaged diagnostic turns (empty text is fine,
  session state drives rendering), hardcoded `tool_calls` in factory (logging
  only), model-assumed weeklyCommitmentHours (prompt-level, separate issue).

## Checklist
- [x] T1: `AppError::is_agent_turn_limit()` (match "MaxTurnsError") + tests — `src-tauri/src/error.rs` (error.rs:59-68)
- [x] T2: Salvage capture on Err in `Orchestrator::run_notebook_block_agent` and
      `run_diagnostic_agent` (capture.clone() to runner; on Err + capture
      populated → warn + dev-trace note + emit completed + Ok with empty text) +
      3 scripted-runner tests — `src-tauri/src/orchestration/orchestrator.rs`
      (notebook: ~line 484-508; diagnostic: ~line 382-413)
- [x] T3: `BlockGenerationJob::generate()`: blind `?` → `run_result`; Err +
      `is_agent_turn_limit()` + empty capture → `Ok(None)` (EmptyAttempt
      corrective note) + test — `src-tauri/src/application/notebook_service/generation.rs`
      (~line 1018-1050; test in notebook_service/tests.rs)
- [x] T4: loop_runner trace: last-attempt transient error →
      `retries_exhausted` verdict (behavior unchanged, pinned by test) —
      `src-tauri/src/orchestration/loop_runner.rs` (~line 116-124)
- [x] T5: In-flight chain guard (OnceLock<Mutex<HashSet>>, RAII ChainGuard on
      document_id) in `run_background_chain` + unit test —
      `src-tauri/src/application/notebook_service/generation.rs` (~line 672-721)
- [x] T6: `cargo check` Finished; `cargo test --lib` → 301 passed, 0 failed
      (294 pre-existing + 7 new)

## Deviations
- T5: `static Mutex<HashSet>` doesn't compile (HashSet::new isn't const) —
  used `OnceLock` fallback (same pattern as `tools::EMITTER`).
- T3: dedicated `MaxTurnsRunner` test double instead of extending CriticRunner.
- No `cargo fmt`: project doesn't enforce rustfmt (no rustfmt.toml, CI runs
  only `cargo test`, baseline whole-tree differs); new code follows the wide
  line house style.

## Acceptance criteria
- A runner that publishes a block THEN errors (MaxTurnsError) yields a persisted
  block, no retry (block_calls == 1).
- A runner that errors with MaxTurnsError WITHOUT publishing gets the EmptyAttempt
  corrective note on the next attempt (inputs[1] contains "NOTA_DEL_SISTEMA").
- Diagnostic run that captures an assessment then errors → Ok, capture read.
- Two rapid `spawn_background_next_block` calls for one document → one chain.
- Full lib test suite passes.

## Constraints / decisions
- TDD off (not configured for this project): ordinary functional checks; tests
  added alongside behavior.
- Route: delegated direct (writer trigger: 5+ non-trivial files).
- Delivery: single work-unit commit on feature branch, Conventional Commit.
- Windows/PowerShell: cargo writes progress to stderr — verify via "Finished" /
  "test result: ok" lines, not exit code.

## Progress
- [x] Exploration + root-cause mapping (subagent, read-only)
- [x] Implementation (T1-T6)
- [x] Verification (cargo test --lib: 301 passed; parent spot-check re-ran the suite)

## Evidence
- Branch: fix/notebook-block-salvage (off local main @ 742f50c)
- Commit: single work-unit commit on this branch,
  "fix(notebook): salvage captured tool output when a run hits the turn cap"
  (6 files, +455/-7; includes this task doc). Hash: see `git log` / Engram
  mirror (self-referential hash can't live inside the commit itself).
- cargo test --lib: 301 passed, 0 failed (parent spot-check re-ran the suite)
- RDD: `review assess` → unassessable (OpenCode runtime not eligible for
  immutable receipt review); preflight STATUS → terminal
  `immutable_review_transport_unsupported`. Review lifecycle unavailable on
  this runtime; mode left untouched (user-owned switch).
