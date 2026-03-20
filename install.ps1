Import-Module (Join-Path $PSScriptRoot 'src/CodexSessionHub.psd1') -Force
Invoke-CsxCli -Arguments @('install-shell')
