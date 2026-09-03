@echo off
setlocal
cd /d "%~dp0"
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\dev-windows.ps1" -Stop
set "LINGSHU_EXIT_CODE=%ERRORLEVEL%"
echo.
if "%LINGSHU_EXIT_CODE%"=="0" (
  echo LingShu has stopped. Press any key to close this window.
) else (
  echo Stop failed. Press any key to close this window.
)
pause >nul
exit /b %LINGSHU_EXIT_CODE%
