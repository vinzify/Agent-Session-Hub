@echo off
setlocal
pwsh -NoProfile -File "%~dp0csx.ps1" __preview "%~1"
