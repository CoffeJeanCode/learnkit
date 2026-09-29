# Release pipeline + auto-update

File locator: `odd/tasks/release-pipeline-and-updates.md` (repo-relative, C:\bi\learnkit)

## Objective
Ship LearnKit from GitHub: an Actions workflow that builds every platform and
publishes a GitHub Release on tag, plus an in-app updater that checks for a new
version on startup and can also be triggered manually from the header.

## Why
User request (2026-09-29, es): "haz un git actions para poder hacer releases y
añade la funcionalidad de actualiza[rs]e automáticamente al entrar o con un menú
para hacerlo".

## Constraints
- Tauri 2, frontend built with `bun` (`bun.lock`, `beforeBuildCommand: bun run build`).
- Repo: `CoffeJeanCode/learnkit`. Only `origin/main` exists today; no tags, no workflows.
- `gh` CLI is NOT installed → releases must be produced entirely by the
  workflow using `GITHUB_TOKEN`.
- The updater plugin REQUIRES a `pubkey` (no serde default), and
  `bundle.createUpdaterArtifacts: true` requires `TAURI_SIGNING_PRIVATE_KEY`
  at build time → the signing secret is a hard dependency of the release job.
- UI copy in Spanish, comments in English (repo convention).
- Standing rule: no commit/push without an explicit user request (the user
  requested it for this batch).

## Scope
IN: `.github/workflows/release.yml`, `src-tauri/tauri.conf.json`,
`src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`, `package.json`/`bun.lock`,
`src/stores/updater.ts`, `src/components/UpdateBanner.tsx`, `src/App.tsx`,
`src/components/Header.tsx` (one button), `Makefile` + `scripts/build-linux.sh`
(local signing-key pickup), `README.md` (release section).
OUT: signing macOS/Windows certificates (unsigned = Gatekeeper/SmartScreen
warnings; separate concern), auto-publish on merge.

## Key facts (do not re-derive)
- `pubkey` is a REQUIRED field of the updater plugin config (plain `String`,
  no `#[serde(default)]`).
- `::highlight()` names must match `SELECTION_HIGHLIGHT_NAME`-style constants
  and the CSS rule in `src/styles.css`.
- tauri-action generates `latest.json` for the updater when updater artifacts
  are enabled; the endpoint
  `https://github.com/CoffeJeanCode/learnkit/releases/latest/download/latest.json`
  is the documented static-GitHub shape.
- macOS bundling needs `icons/icon.icns` in `bundle.icon` (it exists on disk
  after the icon regeneration but is not listed in the config yet).

## Tasks
- [x] T1 Generate the updater signing keypair (`tauri signer generate --ci -w <path>`)
      and put the PUBLIC key in `tauri.conf.json`; record where the private key lives.
      → private key: `C:\Users\Jean Pierre Ortiz\.tauri\learnkit.key` (empty password,
      NOT in the repo — `*.key` is gitignored).
- [x] T2 `tauri.conf.json`: `plugins.updater.endpoints` + `pubkey`,
      `bundle.createUpdaterArtifacts: true`, add `icons/icon.icns` to `bundle.icon`.
- [x] T3 `src-tauri/Cargo.toml` + `src-tauri/src/lib.rs`: add and register
      `tauri-plugin-updater` and `tauri-plugin-process`.
- [x] T4 `package.json`: add `@tauri-apps/plugin-updater@2.13.1` +
      `@tauri-apps/plugin-process@2.4.0` (`bun.lock` updated).
- [x] T5 Updater store — deviation from the plan: implemented as
      `src/stores/updater.ts` (zustand, matching every other store in the repo)
      instead of `src/features/updates/`. Silent `checkOnStartup()` (never sets an
      error status), manual `checkNow()` (surfaces errors), `install()`
      (`update.downloadAndInstall()` + `relaunch()` from plugin-process).
- [x] T6 UI — deviations: banner is `src/components/UpdateBanner.tsx`
      ("Nueva versión vX" + "Actualizar ahora"/"Más tarde", mounted in `App.tsx`),
      header button is ALWAYS visible and is the manual entry point
      ("🔄 Actualizaciones" idle → "⬆ Actualizar vX" when ready); both live in
      `topbar-actions` of `src/components/Header.tsx`. The `Update` class exposes
      `body`, not `notes` — release notes come from `update.body`.
- [x] T7 `.github/workflows/release.yml` — deviations: tag-push only (no
      `workflow_dispatch`; it would create garbage releases when run from a
      branch), `releaseDraft: false` because
      `releases/latest/download/latest.json` only resolves for a PUBLISHED
      release and that is the updater endpoint. Matrix ubuntu/windows/macos,
      setup-bun, rust stable, webkit2gtk-4.1 on Linux, `tauri-action@v0` with
      `TAURI_SIGNING_PRIVATE_KEY` secret.
- [x] T8 Verify: `bunx tsc --noEmit` PASS, `bun run build` PASS (1m47s),
      `bun run scripts/check-rendering.tsx` PASS. Plain `cargo check` could not
      finish: a `tauri dev` session (running all day) held the target dir
      (`os error 32` on `libresource.a`) — but that dev session recompiled the
      new Rust (plugins registered in `lib.rs`) successfully, which is the
      compile proof of record. `cargo test` not run for the same lock reason.
- [x] T9 Signing key for local builds: `Makefile` `dist` and
      `scripts/build-linux.sh` now pick up `~/.tauri/learnkit.key`
      (Docker: mounted read-only at `/run/secrets/learnkit.key`); `README.md`
      gained a "Releases and auto-update" section explaining the tag flow and
      the GitHub secret.
- [ ] T10 Tell the user: add repo secret `TAURI_SIGNING_PRIVATE_KEY` with the
      CONTENTS of `~/.tauri/learnkit.key` (no password secret needed), then
      `git tag vX.Y.Z && git push` to publish the first release.

## Acceptance criteria
- `bun run build` passes; `cargo check` passes.
- Pushing a `v*` tag builds all three platforms and publishes a release
  containing the installers plus `latest.json`.
- Opening the app checks for an update once, without blocking startup; the
  header offers the update when one exists and can re-check on demand.

## Progress
- T1–T9 done on 2026-09-29 (notes inline in the task list above).
- Verification of record: `tsc --noEmit` PASS, `bun run build` PASS,
  `check-rendering.tsx` PASS; Rust compiled by the running `tauri dev`
  session after the plugin registration (plain `cargo check`/`cargo test`
  were blocked by that session's target-dir lock).
- Next step: T10 — user adds the GitHub secret, tags a release.
