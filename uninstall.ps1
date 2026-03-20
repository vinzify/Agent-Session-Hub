Import-Module (Join-Path $PSScriptRoot 'src/CodexSessionHub.psd1') -Force
Invoke-CsxCli -Arguments @('uninstall-shell')
