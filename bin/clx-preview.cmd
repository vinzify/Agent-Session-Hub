@echo off
setlocal
pwsh -NoProfile -File "%~dp0clx.ps1" __preview "%~1"
