@echo off
setlocal
cd /d "%~dp0"
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\dev-windows.ps1"
set "LINGSHU_EXIT_CODE=%ERRORLEVEL%"
if not "%LINGSHU_EXIT_CODE%"=="0" (
  echo.
  echo Startup failed. Press any key to close this window.
  pause >nul
)
exit /b %LINGSHU_EXIT_CODE%
