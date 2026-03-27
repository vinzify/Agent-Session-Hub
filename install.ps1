param(
    [string]$Repository = 'vinzify/Agent-Session-Hub',
    [string]$Version = 'latest',
    [string]$InstallRoot,
    [string]$BinRoot,
    [switch]$SkipShellIntegration
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

function Test-AshLocalSource {
    param([Parameter(Mandatory = $true)][string]$Path)

    return (Test-Path (Join-Path $Path 'Cargo.toml')) -and (Test-Path (Join-Path $Path 'src/main.rs'))
}

function Get-AshScriptRoot {
    if ($PSCommandPath) {
        return (Split-Path -Parent $PSCommandPath)
    }

    if ($MyInvocation.MyCommand.Path) {
        return (Split-Path -Parent $MyInvocation.MyCommand.Path)
    }

    return (Get-Location).Path
}

function Get-AshWindowsTarget {
    switch ([System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture) {
        'X64' { return 'x86_64-pc-windows-msvc' }
        'Arm64' { return 'aarch64-pc-windows-msvc' }
        default { throw "Unsupported Windows architecture: $([System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture)" }
    }
}

function Get-AshReleaseUrl {
    param(
        [Parameter(Mandatory = $true)][string]$Repository,
        [Parameter(Mandatory = $true)][string]$Version,
        [Parameter(Mandatory = $true)][string]$Target
    )

    if ($Version -eq 'latest') {
        return "https://github.com/$Repository/releases/latest/download/agent-session-hub-$Target.zip"
    }

    return "https://github.com/$Repository/releases/download/$Version/agent-session-hub-$Target.zip"
}

function Get-AshBuiltBinary {
    param([Parameter(Mandatory = $true)][string]$SourceRoot)

    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo) {
        throw 'cargo was not found in PATH. Install Rust from https://rustup.rs/ to build from source.'
    }

    & $cargo.Source build --release --manifest-path (Join-Path $SourceRoot 'Cargo.toml')
    if ($LASTEXITCODE -ne 0) {
        throw 'cargo build failed.'
    }

    $binaryPath = Join-Path $SourceRoot 'target/release/agent-session-hub.exe'
    if (-not (Test-Path $binaryPath)) {
        throw "Built binary not found at $binaryPath"
    }

    return $binaryPath
}

function Get-AshDownloadedBinary {
    param(
        [Parameter(Mandatory = $true)][string]$Repository,
        [Parameter(Mandatory = $true)][string]$Version
    )

    $target = Get-AshWindowsTarget
    $tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('agent-session-hub-install-' + [guid]::NewGuid().ToString('N'))
    $archivePath = Join-Path $tempRoot 'release.zip'
    $extractRoot = Join-Path $tempRoot 'extract'
    New-Item -ItemType Directory -Path $extractRoot -Force | Out-Null

    $url = Get-AshReleaseUrl -Repository $Repository -Version $Version -Target $target
    Invoke-WebRequest -Uri $url -OutFile $archivePath
    Expand-Archive -LiteralPath $archivePath -DestinationPath $extractRoot -Force

    $binary = Get-ChildItem -Path $extractRoot -Filter 'agent-session-hub.exe' -Recurse | Select-Object -First 1
    if (-not $binary) {
        throw "Unable to locate agent-session-hub.exe in archive $url"
    }

    return [pscustomobject]@{
        BinaryPath = $binary.FullName
        TempRoot   = $tempRoot
    }
}

function Install-AshBinary {
    param(
        [Parameter(Mandatory = $true)][string]$BinaryPath,
        [Parameter(Mandatory = $true)][string]$ResolvedInstallRoot,
        [Parameter(Mandatory = $true)][string]$ResolvedBinRoot
    )

    $installBinRoot = Join-Path $ResolvedInstallRoot 'bin'
    New-Item -ItemType Directory -Path $installBinRoot -Force | Out-Null
    New-Item -ItemType Directory -Path $ResolvedBinRoot -Force | Out-Null

    $installedBinary = Join-Path $installBinRoot 'agent-session-hub.exe'
    Copy-Item -Path $BinaryPath -Destination $installedBinary -Force
    Copy-Item -Path $installedBinary -Destination (Join-Path $ResolvedBinRoot 'agent-session-hub.exe') -Force
    $cmdTemplate = @'
@echo off
"%~dp0agent-session-hub.exe" --provider {0} %*
'@
    Set-Content -Path (Join-Path $ResolvedBinRoot 'csx.cmd') -Value ($cmdTemplate -f 'codex')
    Set-Content -Path (Join-Path $ResolvedBinRoot 'clx.cmd') -Value ($cmdTemplate -f 'claude')
    Set-Content -Path (Join-Path $ResolvedBinRoot 'cxs.cmd') -Value "@echo off`r`ncsx %*`r`n"

    return $installedBinary
}

$scriptRoot = Get-AshScriptRoot
$resolvedInstallRoot = Get-AshInstallRoot
$resolvedBinRoot = Get-AshBinRoot
$download = $null

try {
    $binaryPath = if (Test-AshLocalSource -Path $scriptRoot) {
        Get-AshBuiltBinary -SourceRoot $scriptRoot
    } else {
        $download = Get-AshDownloadedBinary -Repository $Repository -Version $Version
        $download.BinaryPath
    }

    [void](Install-AshBinary -BinaryPath $binaryPath -ResolvedInstallRoot $resolvedInstallRoot -ResolvedBinRoot $resolvedBinRoot)

    if (-not $SkipShellIntegration) {
        & (Join-Path $resolvedInstallRoot 'bin/agent-session-hub.exe') install-shell
        if ($LASTEXITCODE -ne 0) {
            throw 'csx install-shell failed.'
        }
    }

    Write-Host "Installed Agent Session Hub to $resolvedInstallRoot"
    Write-Host "Command shims installed in $resolvedBinRoot"
    if ($SkipShellIntegration) {
        Write-Host 'Shell integration was skipped.'
    } else {
        Write-Host 'Run: csx doctor and clx doctor'
    }
}
finally {
    if ($download -and (Test-Path $download.TempRoot)) {
        Remove-Item -LiteralPath $download.TempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
