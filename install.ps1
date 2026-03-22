param(
    [string]$Repository = 'vinzify/Codex-Session-Hub',
    [string]$Ref = 'master',
    [string]$InstallRoot,
    [switch]$SkipShellIntegration
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-CshDefaultInstallRoot {
    if ($IsWindows) {
        return (Join-Path $env:LOCALAPPDATA 'AgentSessionHub')
    }

    return (Join-Path $HOME '.local/share/agent-session-hub')
}

function Get-CshDefaultFzfHelp {
    if ($IsWindows) {
        return 'winget install junegunn.fzf'
    }

    if ($IsMacOS) {
        return 'brew install fzf'
    }

    return 'Install fzf with your distro package manager, for example: apt install fzf'
}

function Test-CshRepoRoot {
    param([Parameter(Mandatory = $true)][string]$Path)

    return (Test-Path (Join-Path $Path 'src/AgentSessionHub.psd1')) -and (Test-Path (Join-Path $Path 'bin/csx.ps1')) -and (Test-Path (Join-Path $Path 'bin/clx.ps1'))
}

function Resolve-CshSourceRoot {
    param(
        [Parameter(Mandatory = $true)][string]$Repository,
        [Parameter(Mandatory = $true)][string]$Ref
    )

    if ($env:CODEX_SESSION_HUB_SOURCE_ROOT -and (Test-CshRepoRoot -Path $env:CODEX_SESSION_HUB_SOURCE_ROOT)) {
        return [pscustomobject]@{
            Root        = $env:CODEX_SESSION_HUB_SOURCE_ROOT
            Temporary   = $false
            Description = "local override at $($env:CODEX_SESSION_HUB_SOURCE_ROOT)"
        }
    }

    $scriptPath = ''

    $psCommandPathVariable = Get-Variable -Name PSCommandPath -Scope Script -ErrorAction SilentlyContinue
    if ($psCommandPathVariable -and -not [string]::IsNullOrWhiteSpace([string]$psCommandPathVariable.Value)) {
        $scriptPath = [string]$psCommandPathVariable.Value
    }

    if ([string]::IsNullOrWhiteSpace($scriptPath) -and $MyInvocation -and $MyInvocation.MyCommand) {
        $pathProperty = $MyInvocation.MyCommand.PSObject.Properties['Path']
        if ($pathProperty -and -not [string]::IsNullOrWhiteSpace([string]$pathProperty.Value)) {
            $scriptPath = [string]$pathProperty.Value
        }
    }

    $localRoot = ''
    if (-not [string]::IsNullOrWhiteSpace($scriptPath)) {
        $localRoot = Split-Path -Parent $scriptPath
    }

    if (-not [string]::IsNullOrWhiteSpace($localRoot) -and (Test-CshRepoRoot -Path $localRoot)) {
        return [pscustomobject]@{
            Root        = $localRoot
            Temporary   = $false
            Description = "local source at $localRoot"
        }
    }

    $archiveUrl = "https://github.com/$Repository/archive/refs/heads/$Ref.zip"
    $tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('agent-session-hub-install-' + [guid]::NewGuid().ToString('N'))
    $archivePath = Join-Path $tempRoot 'source.zip'
    $extractRoot = Join-Path $tempRoot 'extract'

    New-Item -ItemType Directory -Force -Path $tempRoot | Out-Null
    New-Item -ItemType Directory -Force -Path $extractRoot | Out-Null

    Invoke-WebRequest -Uri $archiveUrl -OutFile $archivePath
    Expand-Archive -LiteralPath $archivePath -DestinationPath $extractRoot -Force

    $repoRoot = Get-ChildItem -Path $extractRoot -Directory | Select-Object -First 1
    if (-not $repoRoot -or -not (Test-CshRepoRoot -Path $repoRoot.FullName)) {
        throw "Unable to locate Agent Session Hub sources in downloaded archive: $archiveUrl"
    }

    return [pscustomobject]@{
        Root        = $repoRoot.FullName
        Temporary   = $true
        TempRoot    = $tempRoot
        Description = "downloaded archive from $archiveUrl"
    }
}

function Install-CshPayload {
    param(
        [Parameter(Mandatory = $true)][string]$SourceRoot,
        [Parameter(Mandatory = $true)][string]$DestinationRoot
    )

    if (Test-Path $DestinationRoot) {
        Remove-Item -LiteralPath $DestinationRoot -Recurse -Force
    }

    New-Item -ItemType Directory -Force -Path $DestinationRoot | Out-Null

    foreach ($relativePath in @('bin', 'src', 'README.md', 'LICENSE', 'CHANGELOG.md', 'install.ps1', 'uninstall.ps1', 'install.sh', 'uninstall.sh')) {
        $sourcePath = Join-Path $SourceRoot $relativePath
        if (-not (Test-Path $sourcePath)) {
            throw "Required install asset is missing: $sourcePath"
        }

        Copy-Item -Path $sourcePath -Destination (Join-Path $DestinationRoot $relativePath) -Recurse -Force
    }
}

function Install-CshShellIntegration {
    param([Parameter(Mandatory = $true)][string]$InstalledRoot)
    $modulePath = Join-Path $InstalledRoot 'src/AgentSessionHub.psd1'
    Import-Module $modulePath -Force
    return @(Invoke-CsxCli -Arguments @('install-shell'))
}

function Get-CshPostInstallState {
    param(
        [Parameter(Mandatory = $true)][string]$InstalledRoot,
        [bool]$ProfileInstalled
    )

    $profilePath = if ($IsWindows) {
        $PROFILE.CurrentUserCurrentHost
    } else {
        $shellPath = [string]$env:SHELL
        if ($shellPath -match '(^|/)zsh$') {
            Join-Path $HOME '.zprofile'
        } elseif ($shellPath -match '(^|/)bash$') {
            Join-Path $HOME '.bash_profile'
        } else {
            Join-Path $HOME '.profile'
        }
    }
    $modulePath = Join-Path $InstalledRoot 'src/AgentSessionHub.psd1'
    $launcherPath = if ($IsWindows) { '' } else { Join-Path $HOME '.local/bin/csx' }
    $claudeLauncherPath = if ($IsWindows) { '' } else { Join-Path $HOME '.local/bin/clx' }

    [pscustomobject]@{
        ModulePath       = $modulePath
        ProfilePath      = $profilePath
        LauncherPath     = $launcherPath
        ClaudeLauncherPath = $claudeLauncherPath
        ProfileInstalled = $ProfileInstalled
        FzfAvailable     = [bool](Get-Command fzf -ErrorAction SilentlyContinue)
        CodexAvailable   = [bool](Get-Command codex -ErrorAction SilentlyContinue)
        ClaudeAvailable  = [bool](Get-Command claude -ErrorAction SilentlyContinue)
    }
}

$resolvedInstallRoot = if ($InstallRoot) { $InstallRoot } else { Get-CshDefaultInstallRoot }
$source = $null

try {
    $source = Resolve-CshSourceRoot -Repository $Repository -Ref $Ref
    Install-CshPayload -SourceRoot $source.Root -DestinationRoot $resolvedInstallRoot
    $profileInstalled = $false
    if (-not $SkipShellIntegration) {
        $shellInstallOutput = @(Install-CshShellIntegration -InstalledRoot $resolvedInstallRoot)
        $shellInstallOutput | ForEach-Object { Write-Host $_ }
        $profileInstalled = $true
    }
    $postInstall = Get-CshPostInstallState -InstalledRoot $resolvedInstallRoot -ProfileInstalled $profileInstalled

    Write-Host "Installed Agent Session Hub to $resolvedInstallRoot"
    Write-Host "Source: $($source.Description)"

    if (-not $postInstall.FzfAvailable) {
        Write-Warning "fzf was not found in PATH. Install it with: $(Get-CshDefaultFzfHelp)"
    }

    if (-not $postInstall.CodexAvailable) {
        Write-Warning 'codex was not found in PATH. Install Codex CLI before using csx.'
    }

    if (-not $postInstall.ClaudeAvailable) {
        Write-Warning 'claude was not found in PATH. Install Claude Code before using clx.'
    }

    if (-not $SkipShellIntegration) {
        if ($IsWindows) {
            Write-Host "Shell integration installed in $($postInstall.ProfilePath)"
            Write-Host 'Reload your shell with: . $PROFILE'
        } else {
            Write-Host "Shell integration installed in $($postInstall.ProfilePath)"
            Write-Host "Launchers installed in $(Split-Path -Parent $postInstall.LauncherPath)"
            Write-Host "Reload your shell with: source $($postInstall.ProfilePath)"
        }
        Write-Host 'Then run: csx doctor and clx doctor'
    } else {
        Write-Host 'Shell integration was skipped.'
    }
} finally {
    if ($source -and $source.Temporary -and (Test-Path $source.TempRoot)) {
        Remove-Item -LiteralPath $source.TempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
