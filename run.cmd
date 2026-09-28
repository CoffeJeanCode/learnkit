@echo off
rem Run LearnKit: `bun run tauri dev` (Vite + Tauri desktop window).
rem Delegates to scripts\run.ps1. Double-clickable.
rem
rem   run.cmd            launch the app
rem   run.cmd -Check     verify prerequisites without launching
setlocal EnableExtensions
set "ARGS="
:parse
if "%~1"=="" goto run
if /I "%~1"=="--check" (set "ARGS=%ARGS% -Check") else (set "ARGS=%ARGS% %~1")
shift
goto parse
:run
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\run.ps1"%ARGS%
exit /b %ERRORLEVEL%
