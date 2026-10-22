# Feature: Live-LLM end-to-end checks (Xiaomi MiMo)

## Objective
Exercise every shipped pedagogy part against a REAL model (Xiaomi MiMo) and
report observed compliance: roadmap diagnostic for an absolute beginner, and
a full notebook class (theory → scaffolded practice → gates → closure).

## Scope
- `examples/roadmap_e2e.rs`: add a Xiaomi model pin and the real-user
  beginner scenario ("Quiero montar mi primer servidor...").
- New `examples/notebook_e2e.rs`: seal a course, generate class 1 block by
  block, drive gates with scripted student answers (one deliberate wrong
  first attempt, one voluntary easier-step request), and assert/report the
  pedagogy invariants.

## Checks reported per run
- Roadmap: no self-perception battery options; battery None or objective;
  clean topic title.
- Notebook: first block is non-cold theory with numbered steps; block after
  first theory is not heuristic_error_audit; missions declare supportLevel
  and guided/faded carry workedTemplate/interactiveSteps; interactionMode
  variety; closure initialPrediction cites the real first gate answer;
  grounding/critic retries counted.

## Checklist
- [x] T1 Write notebook_e2e harness + Xiaomi pin (delegated writer; mapping
      trigger: needs service/store/orchestrator APIs across 4+ files).
- [~] T2 Run both harnesses against Xiaomi (run 1: 401 bad key extraction; run 2 2026-10-22: both stalled on 120s provider timeouts — see Evidence; needs
      user action).
- [ ] T3 Report.

## Constraints
- API key only via env var `LEARNKIT_E2E_API_KEY`; never printed, never read
  from the app vault. TDD off; harness compiles under `cargo build --examples`.
- Branch `perf/toon-prompt-optimization` (user directive: same branch).

## Evidence (run 2, 2026-10-22, xiaomi mimo-v2.6-flash)
- Roadmap: 5 of 6 diagnostic turns hit PROMPT_TIMEOUT_SECS=120
  (providers/factory.rs:18); one salvaged plan failed grounding
  (2 interactiveBlocks, needs 3-4); run aborted with Timeout.
- Notebook: block 0 generated in 138s and accepted by the critic. It opens
  with context (terminal as "chat with the computer"), uses 4 numbered steps
  with literal `$ ls` transcripts. Check (a) reported FAIL as a harness false
  negative (example heuristic missed transcripts; fixed). Block 1 timed out
  3x at 120s, so the run stalled. Checks b-f were not reachable.
