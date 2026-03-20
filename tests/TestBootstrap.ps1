$script:TestRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$script:ProjectRoot = Split-Path -Parent $script:TestRoot
$script:ModuleRoot = Join-Path $script:ProjectRoot 'src'

$loadOrder = @(
    'Common/Paths.ps1'
    'Common/Formatting.ps1'
    'Metadata/IndexStore.ps1'
    'Sessions/Parser.ps1'
    'Fzf/Browser.ps1'
    'Runtime/Codex.ps1'
    'Commands/Cli.ps1'
)

foreach ($relativePath in $loadOrder) {
    $sourcePath = $script:ModuleRoot
    foreach ($part in ($relativePath -split '/')) {
        $sourcePath = Join-Path $sourcePath $part
    }

    if (-not (Test-Path -LiteralPath $sourcePath)) {
        throw "Missing test bootstrap source file: $sourcePath"
    }

    . $sourcePath
}
