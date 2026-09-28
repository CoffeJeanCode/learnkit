# LearnKit bootstrap (PowerShell). Installs whatever toolchain is missing,
# then the JS and Rust dependencies.
#
#   .\scripts\setup.ps1           # install what's missing + bun install + cargo fetch
#   .\scripts\setup.ps1 -Check    # verify only; exits non-zero if something is missing
[CmdletBinding()]
param([switch]$Check)

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

$script:Fails = 0
function Step($Text) { Write-Host ""; Write-Host $Text }
function Ok($Text)   { Write-Host "  [ok] $Text" -ForegroundColor Green }
function Bad($Text)  { Write-Host "  [!!] $Text" -ForegroundColor Red; $script:Fails++ }
function Warn($Text) { Write-Host "  [..] $Text" -ForegroundColor Yellow }

# So the rest of this run sees tools installed moments ago.
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:USERPROFILE\.bun\bin;$env:PATH"

function Test-Command([string]$Name) { return [bool](Get-Command $Name -ErrorAction SilentlyContinue) }
function Test-Node18 {
    if (-not (Test-Command 'node')) { return $false }
    $major = 0
    try { $major = [int]((node --version) -replace '^v', '').Split('.')[0] } catch { return $false }
    return $major -ge 18
}
function Get-RustHost {
    # MSVC is only usable when a Visual Studio linker is around; otherwise the
    # repo's documented GNU path is the one that builds (see README).
    # `cl.exe` is the MSVC compiler (System32 ships an unrelated `link.exe`).
    if (Test-Command 'cl') { return $null }
    $vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vsWhere) { if (& $vsWhere -latest -property installationPath) { return $null } }
    return 'x86_64-pc-windows-gnu'
}

Step '1/5 Rust (rustup + cargo, stable)'
if ((Test-Command 'rustup') -and (Test-Command 'cargo') -and (Test-Command 'rustc')) {
    Ok (rustc --version)
} elseif ($Check) {
    Bad 'rustup/cargo not found (install: https://rustup.rs)'
} else {
    $installer = Join-Path $env:TEMP 'rustup-init.exe'
    Invoke-WebRequest -Uri 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-gnu/rustup-init.exe' -OutFile $installer
    $hostTarget = Get-RustHost
    if ($hostTarget) {
        Warn "no MSVC linker detected - installing the GNU toolchain ($hostTarget, documented in README)"
        & $installer -y --default-toolchain stable -t $hostTarget --no-modify-path
    } else {
        & $installer -y --default-toolchain stable --no-modify-path
    }
    Remove-Item $installer -ErrorAction SilentlyContinue
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    if ((Test-Command 'rustc') -and (Test-Command 'cargo')) { Ok (rustc --version) }
    else { Bad 'rustup install did not put cargo on PATH' }
}

Step '2/5 Bun (JS package manager + script runner)'
if (Test-Command 'bun') {
    Ok "bun $(bun --version)"
} elseif ($Check) {
    Bad 'bun not found (install: https://bun.sh)'
} else {
    & powershell -NoProfile -ExecutionPolicy Bypass -Command "irm bun.sh/install.ps1 | iex"
    $env:PATH = "$env:USERPROFILE\.bun\bin;$env:PATH"
    if (Test-Command 'bun') { Ok "bun $(bun --version)" }
    else { Bad 'bun install failed (open a new PowerShell and re-run)' }
}

Step '3/5 Node >= 18 (Tauri CLI internals)'
if (Test-Node18) {
    Ok "node $(node --version)"
} elseif (Test-Command 'node') {
    Warn "node $(node --version) is older than 18 - upgrade it (Tauri CLI needs >= 18)"
    $script:Fails++
} elseif ($Check) {
    Bad 'node not found (need >= 18)'
} elseif (Test-Command 'winget') {
    Warn 'node missing - installing Node.js LTS via winget'
    winget install --id OpenJS.NodeJS.LTS -e --accept-source-agreements --accept-package-agreements
    $env:PATH = "$env:ProgramFiles\nodejs;$env:LOCALAPPDATA\Programs\nodejs;$env:PATH"
    if (Test-Node18) { Ok "node $(node --version)" }
    else { Warn 'node still not visible - open a new PowerShell and re-run' }
} else {
    Warn 'node missing - install Node >= 18 from https://nodejs.org'
}

Step '4/5 JS dependencies (bun install)'
if ($Check) {
    if (Test-Path 'node_modules') { Ok 'node_modules present' } else { Bad 'node_modules missing (run: bun install)' }
} elseif (Test-Command 'bun') {
    bun install
    if ($LASTEXITCODE -eq 0) { Ok 'bun install done' } else { Bad "bun install failed (exit $LASTEXITCODE)" }
} else {
    Warn 'skipped (bun missing)'
}

Step '5/5 Rust dependencies (cargo fetch)'
if ($Check) {
    if (Test-Command 'cargo') { Ok 'cargo available' } else { Bad 'cargo missing' }
} elseif (Test-Command 'cargo') {
    cargo fetch --manifest-path (Join-Path $Root 'src-tauri\Cargo.toml')
    if ($LASTEXITCODE -eq 0) { Ok 'crates fetched' } else { Bad "cargo fetch failed (exit $LASTEXITCODE)" }
} else {
    Warn 'skipped (cargo missing)'
}

Write-Host ""
if ($script:Fails -gt 0) {
    Write-Host "$($script:Fails) prerequisite(s) missing. Fix them, then run: .\run.ps1 (or run.cmd / make run)" -ForegroundColor Red
    exit 1
}
Write-Host 'All set. Run the app with one of:' -ForegroundColor Green
Write-Host '  .\run.ps1           (PowerShell)'
Write-Host '  run.cmd             (cmd, double-click)'
Write-Host '  make run            (bash: bash scripts/run.sh)'
exit 0
