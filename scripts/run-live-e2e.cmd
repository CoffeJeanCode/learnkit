@echo off
rem Live e2e against Xiaomi: roadmap_e2e (scenario "principiante real") then notebook_e2e.
rem Requires LEARNKIT_E2E_API_KEY in the environment (never echoed).
rem Model: LEARNKIT_E2E_MODEL (default mimo-v2.6-flash; only flash/pro are allowed).
setlocal
if "%LEARNKIT_E2E_API_KEY%"=="" (
  echo ERROR: LEARNKIT_E2E_API_KEY no esta definida en el entorno. 1>&2
  exit /b 1
)
set "LEARNKIT_E2E_PROVIDER=xiaomi"
if "%LEARNKIT_E2E_MODEL%"=="" set "LEARNKIT_E2E_MODEL=mimo-v2.6-flash"
set "OUT=%TEMP%\learnkit-e2e"
if not exist "%OUT%" mkdir "%OUT%"
cd /d "%~dp0..\src-tauri" || exit /b 1

echo START roadmap %DATE% %TIME% model=%LEARNKIT_E2E_MODEL% > "%OUT%\status.txt"
set "LEARNKIT_E2E_SCENARIO=principiante real"
cargo run --example roadmap_e2e > "%OUT%\roadmap.log" 2>&1
echo DONE roadmap exit=%ERRORLEVEL% %DATE% %TIME% >> "%OUT%\status.txt"

set "LEARNKIT_E2E_SCENARIO="
echo START notebook %DATE% %TIME% >> "%OUT%\status.txt"
cargo run --example notebook_e2e > "%OUT%\notebook.log" 2>&1
echo DONE notebook exit=%ERRORLEVEL% %DATE% %TIME% >> "%OUT%\status.txt"
echo Logs en %OUT%
endlocal
