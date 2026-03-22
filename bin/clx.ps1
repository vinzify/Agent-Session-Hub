param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Arguments
)

$modulePath = Join-Path (Split-Path -Parent $PSScriptRoot) 'src/AgentSessionHub.psd1'
Import-Module $modulePath -Force
Invoke-ClxCli -Arguments $Arguments
