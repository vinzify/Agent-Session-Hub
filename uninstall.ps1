param(
    [string]$InstallRoot,
    [string]$BinRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-AshInstallRoot {
    if ($InstallRoot) {
        return $InstallRoot
    }

    return (Join-Path $env:LOCALAPPDATA 'AgentSessionHub')
}

function Get-AshBinRoot {
    if ($BinRoot) {
        return $BinRoot
    }

    return (Join-Path (Get-AshInstallRoot) 'bin')
}

$resolvedInstallRoot = Get-AshInstallRoot
$resolvedBinRoot = Get-AshBinRoot
$exePath = Join-Path $resolvedInstallRoot 'bin/agent-session-hub.exe'

if (Test-Path $exePath) {
    & $exePath uninstall-shell *> $null
}

foreach ($name in @('agent-session-hub.exe', 'csx.cmd', 'clx.cmd', 'cxs.cmd')) {
    $path = Join-Path $resolvedBinRoot $name
    if (Test-Path $path) {
        Remove-Item -LiteralPath $path -Force
    }
}

if (Test-Path $resolvedInstallRoot) {
    Remove-Item -LiteralPath $resolvedInstallRoot -Recurse -Force
    Write-Host "Removed Agent Session Hub from $resolvedInstallRoot"
} else {
    Write-Host "Install root not found at $resolvedInstallRoot"
}
