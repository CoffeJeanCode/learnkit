# Run LearnKit: `bun run tauri dev` (Vite dev server + Tauri desktop window).
#
#   .\scripts\run.ps1           # launch the app
#   .\scripts\run.ps1 -Check    # verify prerequisites without launching
[CmdletBinding()]
param([switch]$Check)

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:USERPROFILE\.bun\bin;$env:PATH"

function Test-Command([string]$Name) { return [bool](Get-Command $Name -ErrorAction SilentlyContinue) }
function Test-Node18 {
    if (-not (Test-Command 'node')) { return $false }
    $major = 0
    try { $major = [int]((node --version) -replace '^v', '').Split('.')[0] } catch { return $false }
    return $major -ge 18
}

$fails = 0
if ($Check) {
    Write-Host 'Prerequisites for `bun run tauri dev`:'
    if ((Test-Command 'rustc') -and (Test-Command 'cargo')) { Write-Host "  [ok] $(rustc --version)" -ForegroundColor Green }
    else { Write-Host '  [!!] rust/cargo missing - run: .\scripts\setup.ps1' -ForegroundColor Red; $fails++ }
    if (Test-Command 'bun') { Write-Host "  [ok] bun $(bun --version)" -ForegroundColor Green }
    else { Write-Host '  [!!] bun missing - run: .\scripts\setup.ps1' -ForegroundColor Red; $fails++ }
    if (Test-Node18) { Write-Host "  [ok] node $(node --version)" -ForegroundColor Green }
    else { Write-Host '  [!!] node >= 18 missing - run: .\scripts\setup.ps1' -ForegroundColor Red; $fails++ }
    if (Test-Path 'node_modules') { Write-Host '  [ok] node_modules present' -ForegroundColor Green }
    else { Write-Host '  [!!] dependencies not installed - run: .\scripts\setup.ps1' -ForegroundColor Red; $fails++ }
    if ($fails -gt 0) { exit 1 }
    Write-Host ''
    Write-Host 'Would run: bun run tauri dev'
    exit 0
}

foreach ($dep in 'cargo', 'bun', 'node') {
    if (-not (Test-Command $dep)) {
        Write-Host "Missing $dep. Run the bootstrap first: .\scripts\setup.ps1 (or setup.cmd / make setup)" -ForegroundColor Red
        exit 1
    }
}
if (-not (Test-Path 'node_modules')) {
    Write-Host 'node_modules missing. Run: .\scripts\setup.ps1 (or setup.cmd / make setup)' -ForegroundColor Red
    exit 1
}

& bun run tauri dev
exit $LASTEXITCODE
