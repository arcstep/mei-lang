@echo off
REM Cold-start wrapper: requires Git Bash (bash.exe on PATH).
setlocal
set "SCRIPT_DIR=%~dp0"
where bash >nul 2>&1
if errorlevel 1 (
  echo error: bash not found. Install Git for Windows and ensure bash is on PATH.
  exit /b 1
)
bash "%SCRIPT_DIR%bootstrap.sh" %*
exit /b %ERRORLEVEL%
