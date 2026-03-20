param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Arguments
)

$modulePath = Join-Path (Split-Path -Parent $PSScriptRoot) 'src/CodexSessionHub.psd1'
Import-Module $modulePath -Force
Invoke-CsxCli -Arguments $Arguments
