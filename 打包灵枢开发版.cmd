@echo off
setlocal
cd /d "%~dp0"
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\package-dev-windows.ps1"
set "LINGSHU_EXIT_CODE=%ERRORLEVEL%"
echo.
if "%LINGSHU_EXIT_CODE%"=="0" (
  echo Package completed. Press any key to close this window.
) else (
  echo Package failed. Press any key to close this window.
)
pause >nul
exit /b %LINGSHU_EXIT_CODE%
