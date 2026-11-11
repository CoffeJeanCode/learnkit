# Runs the Rust unit tests on Windows. Extra args go to `cargo test`
# (e.g. .\scripts\test-rust.ps1 visual_repair).
#
# On the MSVC toolchain the test binary has no app manifest (tauri embeds it in
# the exe only), so the loader binds comctl32 v5 and every test dies at startup
# with STATUS_ENTRYPOINT_NOT_FOUND. This embeds a Common-Controls v6 manifest
# into the test link only, in a separate target dir so the app's build cache
# stays untouched. (On GNU, build.rs already handles it via embed-resource.)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$manifest = Join-Path $root "src-tauri\Cargo.toml"

$rustHost = (rustc -vV | Select-String '^host: ').ToString().Substring(6).Trim()
if ($rustHost -like "*windows-msvc") {
    $env:CARGO_TARGET_DIR = Join-Path $root "src-tauri\target-msvc-tests"
    $appManifest = (Join-Path $root "src-tauri\windows\test-common-controls.manifest") -replace '\\', '/'
    $env:RUSTFLAGS = "-C link-arg=/MANIFEST:EMBED -C link-arg=/MANIFESTINPUT:$appManifest"
}

cargo test --manifest-path $manifest --lib @args
exit $LASTEXITCODE
