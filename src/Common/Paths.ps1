function Get-CshProjectRoot {
    return $script:ProjectRoot
}

function Get-CshDefaultSessionRoot {
    return (Join-Path $HOME '.codex\sessions')
}

function Get-CshSessionRoot {
    if ($env:CODEX_SESSION_HUB_SESSION_ROOT) {
        return $env:CODEX_SESSION_HUB_SESSION_ROOT
    }

    return (Get-CshDefaultSessionRoot)
}

function Get-CshConfigRoot {
    if ($env:CODEX_SESSION_HUB_CONFIG_ROOT) {
        return $env:CODEX_SESSION_HUB_CONFIG_ROOT
    }

    if ($IsWindows) {
        return (Join-Path $env:APPDATA 'CodexSessionHub')
    }

    return (Join-Path $HOME '.config/codex-session-hub')
}

function Get-CshIndexPath {
    return (Join-Path (Get-CshConfigRoot) 'index.json')
}

function Get-CshShimPath {
    return (Join-Path (Get-CshProjectRoot) 'bin/csx.ps1')
}

function Get-CshQueryShimPath {
    return (Join-Path (Get-CshProjectRoot) 'bin/csx-query.cmd')
}

function Get-CshPreviewShimPath {
    return (Join-Path (Get-CshProjectRoot) 'bin/csx-preview.cmd')
}

function Get-CshProfilePath {
    return $PROFILE.CurrentUserCurrentHost
}

function Get-CshLauncherRoot {
    if ($IsWindows) {
        return ''
    }

    return (Join-Path $HOME '.local/bin')
}

function Get-CshShellProfilePath {
    if ($IsWindows) {
        return (Get-CshProfilePath)
    }

    $shellPath = [string]$env:SHELL
    $zshProfile = Join-Path $HOME '.zprofile'
    $bashProfile = Join-Path $HOME '.bash_profile'
    $defaultProfile = Join-Path $HOME '.profile'

    if ($shellPath -match '(^|/)zsh$') {
        return $zshProfile
    }

    if ($shellPath -match '(^|/)bash$') {
        return $bashProfile
    }

    if (Test-Path $zshProfile) {
        return $zshProfile
    }

    if (Test-Path $bashProfile) {
        return $bashProfile
    }

    return $defaultProfile
}

function Ensure-CshDirectory {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path $Path)) {
        New-Item -ItemType Directory -Force -Path $Path | Out-Null
    }
}

function Normalize-CshPath {
    param([string]$Path)

    if ([string]::IsNullOrWhiteSpace($Path)) {
        return ''
    }

    $trimmed = $Path.Trim() -replace '^[\\/]{2}\?[\\/]', ''

    # Keep Windows drive-style paths stable even when parsed on Unix runners.
    if ($trimmed -match '^[A-Za-z]:[\\/]') {
        return ($trimmed -replace '/', '\').TrimEnd('\', '/')
    }

    if ((-not $IsWindows) -and (Test-Path -LiteralPath $trimmed)) {
        try {
            return ((Resolve-Path -LiteralPath $trimmed).ProviderPath).TrimEnd('\', '/')
        } catch {
        }
    }

    try {
        $fullPath = [System.IO.Path]::GetFullPath($trimmed)
    } catch {
        $fullPath = $trimmed
    }

    return $fullPath.TrimEnd('\', '/')
}
