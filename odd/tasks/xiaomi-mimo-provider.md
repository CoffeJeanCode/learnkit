# Xiaomi MiMo provider

## Objective
Add first-party `xiaomi` provider for MiMo models, selectable in Providers + onboarding, reusing OpenAI-compatible client.

## Why
User request (2026-10-15): "Añade el proveedor de xiaomi para usar mimo."

## Branch
`feature/xiaomi-mimo-provider` (to create from `main`)

## Key findings from exploration (do not re-derive)
- Providers are closed enum `ProviderKind` in `src-tauri/src/domain/provider.rs:7-64` (5 variants: OpenAI, Anthropic, Gemini, OpenRouter, DeepSeek), mirrored as TS union in `src/types/index.ts:1`.
- All behavior fans out via exhaustive `match`: `display_name()`, `default_base_url()`, `suggested_models()` (domain), seed IDs in `src-tauri/src/providers/registry.rs:20-43`, env keys + fallback model in `src-tauri/src/state/app_state.rs:173-224`, 8 construction sites + verify/list in `src-tauri/src/providers/factory.rs`.
- Frontend (`ProvidersView.tsx`, `RoadmapView.tsx` onboarding select, `stores/providers.ts`, `lib/tauri.ts`) is data-driven from `list_providers()` — no hardcoded lists except `AgentsView.tsx:12` new-agent default `openai`.
- `schemas.ts` has no provider enum — no change needed there.
- OpenAI-compatible branch (`GET {base}/models` + Bearer) covers OpenAI/OpenRouter/DeepSeek in `verify_connection`/`list_models`; MiMo should join it if Bearer + `/models` confirmed.
- `grep -i xiaomi|mimo`: zero matches — greenfield.
- Recommended impl: new `Xiaomi` variant, `rig_core::providers::openai::Client` + custom `base_url` in all factory arms (rig 0.42 has no xiaomi module).

## TDD mode
Unresolved/no explicit project config found (no `sdd-init/learnkit` memory). Default: ordinary functional checks (not strict RED-GREEN). Verification: `cargo test` from `src-tauri/`, `bunx tsc --noEmit`, `bun run build`.

## Delivery forecast
~6 files touched, <100 lines changed. Single PR, well under 400-line budget. No chain strategy needed.

## Tasks

- [x] T1 — Research: CONFIRMED first-party API exists (2026-10-15, sources mimo.mi.com). Base `https://api.xiaomimimo.com/v1` (placeholder `api.mimo.xiaomi.com` WRONG). Auth Bearer preferred (rig-compatible) + `api-key` alt. Models `mimo-v2.6-pro` (default), `mimo-v2.6-flash`, `mimo-v2.6-pro-ultraspeed` (v2.5 deprecated, dies 2026-10-21). Env `MIMO_API_KEY` (+alias `XIAOMI_API_KEY`). GET /v1/models yes. Output ceiling 131072. Tool-calling supported.
- [x] T2 — Backend: add `Xiaomi` variant (`domain/provider.rs`: all/display_name/default_base_url/suggested_models) + `registry.rs` default_id + test count 5→6. DONE 2026-10-15 + `ensure_defaults()` backfill + `app_state.rs` wiring.
- [x] T3 — Backend: `factory.rs` 6 prompt builders + verify/list OpenAI-compatible branch + output_budget; `app_state.rs` ENV_KEY_CANDIDATES + MODEL_FOR. DONE 2026-10-15 (`output_budget` Xiaomi 131072, `MIMO_API_KEY`/`XIAOMI_API_KEY`, fallback `mimo-v2.6-pro`).
- [x] T4 — Frontend: `src/types/index.ts` union + verify data-driven UI picks it up; optional AgentsView default untouched. DONE 2026-10-15 (`tsc --noEmit` clean).
- [ ] T5 — Verification: `cargo test`, `tsc --noEmit`, manual Providers UI (configure key, test, list models).

## Progress
2026-10-15: exploration done, feature doc created. Research pending.
2026-10-15: T2-T4 implemented (uncommitted writer work consolidated); `cargo check` + `cargo check --tests` clean (isolated target dir; default target dir has a pre-existing file-lock from concurrent cargo processes), `bunx tsc --noEmit` clean. T5 remaining: full `cargo test` run + manual Providers UI.

## Next step
Run T1 research, then implement T2-T4 in one bounded writer, verify T5.
