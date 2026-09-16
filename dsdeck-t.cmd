@echo off
setlocal
pushd "%~dp0" || exit /b 1
set "TEMP=%CD%\Temp\tooling"
set "TMP=%TEMP%"
set "TMPDIR=%TEMP%"
if not exist "%TEMP%" mkdir "%TEMP%"
node scripts\deck.mjs dev
set "deckExitCode=%errorlevel%"
popd
exit /b %deckExitCode%
