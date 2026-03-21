param(
    [string]$InstallRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-CshDefaultInstallRoot {
    if ($IsWindows) {
        return (Join-Path $env:LOCALAPPDATA 'CodexSessionHub')
    }

    return (Join-Path $HOME '.local/share/codex-session-hub')
}

function Get-CshProfilePath {
    return $PROFILE.CurrentUserCurrentHost
}

function Uninstall-CshShellIntegration {
    param([Parameter(Mandatory = $true)][string]$ResolvedInstallRoot)

    $modulePath = Join-Path $ResolvedInstallRoot 'src/CodexSessionHub.psd1'
    if (-not (Test-Path $modulePath)) {
        return $false
    }

    Import-Module $modulePath -Force
    [void]@(Invoke-CsxCli -Arguments @('uninstall-shell'))
    return $true
}

$resolvedInstallRoot = if ($InstallRoot) { $InstallRoot } else { Get-CshDefaultInstallRoot }

[void](Uninstall-CshShellIntegration -ResolvedInstallRoot $resolvedInstallRoot)

if (Test-Path $resolvedInstallRoot) {
    Remove-Item -LiteralPath $resolvedInstallRoot -Recurse -Force
    Write-Host "Removed Codex Session Hub from $resolvedInstallRoot"
} else {
    Write-Host "Install root not found at $resolvedInstallRoot"
}

Write-Host 'Reload your shell with: . $PROFILE'
