@echo off
rem Thin wrapper: run build.ps1 (in this folder) and forward all arguments.
rem Examples:
rem   build.bat
rem   build.bat -Platform linux/arm/v7 -Save
rem   build.bat -SkipRust
setlocal
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build.ps1" %*
exit /b %ERRORLEVEL%
