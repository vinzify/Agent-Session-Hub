Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:ModuleRoot = Split-Path -Parent $PSCommandPath
$script:ProjectRoot = Split-Path -Parent $script:ModuleRoot

$loadOrder = @(
    'Common\Providers.ps1'
    'Common\Paths.ps1'
    'Common\Formatting.ps1'
    'Metadata\IndexStore.ps1'
    'Sessions\Parser.ps1'
    'Fzf\Browser.ps1'
    'Runtime\Codex.ps1'
    'Runtime\Claude.ps1'
    'Commands\Cli.ps1'
)

foreach ($relativePath in $loadOrder) {
    . (Join-Path $script:ModuleRoot $relativePath)
}

Export-ModuleMember -Function Invoke-CsxCli, Invoke-ClxCli
