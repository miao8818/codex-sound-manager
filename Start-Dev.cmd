@echo off
setlocal
chcp 65001 >nul
cd /d "%~dp0"
call npm install
if errorlevel 1 exit /b 1
call npm run tauri -- dev
if errorlevel 1 exit /b 1
endlocal
