@echo off
rem LearnKit bootstrap (cmd). Installs missing toolchain (rustup, bun, node),
rem then JS + Rust dependencies. Delegates to scripts\setup.ps1.
rem
rem   setup.cmd           install what's missing
rem   setup.cmd -Check    verify only, non-zero exit if something is missing
setlocal EnableExtensions
set "ARGS="
:parse
if "%~1"=="" goto run
if /I "%~1"=="--check" (set "ARGS=%ARGS% -Check") else (set "ARGS=%ARGS% %~1")
shift
goto parse
:run
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\setup.ps1"%ARGS%
exit /b %ERRORLEVEL%
