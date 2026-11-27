@echo off
rem osdy-code-desktop ocd shim (installed by the Windows installer)
rem `ocd` opens the current folder as a workspace, `ocd <folder>` that folder.
if "%~1"=="workspace" goto workspace
start "" "%~dp0..\gentle-shell-gpui.exe" --cwd %*
exit /b %errorlevel%
:workspace
"%~dp0..\gentle-shell-gpui.exe" %*
exit /b %errorlevel%
