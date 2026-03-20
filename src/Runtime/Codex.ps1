function Get-CshCodexCommand {
    return (Get-Command codex -ErrorAction SilentlyContinue)
}

function Assert-CshCodexAvailable {
    if (Get-CshCodexCommand) {
        return
    }

    throw 'codex was not found in PATH.'
}

function Remove-CshSessions {
    param(
        [Parameter(Mandatory = $true)][object[]]$Sessions,
        [Parameter(Mandatory = $true)][hashtable]$Index
    )

    $results = foreach ($session in $Sessions) {
        try {
            Remove-Item -Path $session.FilePath -Force -ErrorAction Stop
            Remove-CshAlias -Index $Index -SessionId $session.SessionId
            [pscustomobject]@{
                SessionId = $session.SessionId
                Success   = $true
                Message   = 'Deleted'
            }
        } catch {
            [pscustomobject]@{
                SessionId = $session.SessionId
                Success   = $false
                Message   = $_.Exception.Message
            }
        }
    }

    Save-CshIndex -Index $Index
    return @($results)
}

function Rename-CshSession {
    param(
        [Parameter(Mandatory = $true)][object]$Session,
        [Parameter(Mandatory = $true)][hashtable]$Index,
        [string]$Alias
    )

    Set-CshAlias -Index $Index -SessionId $Session.SessionId -Alias $Alias
    Save-CshIndex -Index $Index
}

function Resume-CshSession {
    param(
        [Parameter(Mandatory = $true)][object]$Session,
        [switch]$ShellMode
    )

    Assert-CshCodexAvailable

    if ($ShellMode -and $Session.ProjectExists) {
        Set-Location $Session.ProjectPath
        & codex resume $Session.SessionId
        return
    }

    if ($ShellMode -and -not $Session.ProjectExists) {
        Write-Warning "Project path no longer exists: $($Session.ProjectPath)"
    }

    & codex resume --cd $Session.ProjectPath $Session.SessionId
}

function Get-CshProfileBlock {
    $modulePath = (Join-Path (Get-CshProjectRoot) 'src/CodexSessionHub.psd1').Replace("'", "''")
    return @"
# >>> Codex Session Hub >>>
\$cshFzfPath = Join-Path \$env:LOCALAPPDATA 'Programs\fzf\bin'
if ((Test-Path \$cshFzfPath) -and ((\$env:Path -split ';') -notcontains \$cshFzfPath)) {
    \$env:Path = "\$cshFzfPath;\$env:Path"
}
function csx {
    Import-Module '$modulePath' -Force
    Invoke-CsxCli -Arguments \$args -ShellMode
}
Set-Alias cxs csx
# <<< Codex Session Hub <<<
"@
}

function Install-CshShellIntegration {
    $profilePath = Get-CshProfilePath
    Ensure-CshDirectory -Path (Split-Path -Parent $profilePath)

    $markerStart = '# >>> Codex Session Hub >>>'
    $markerEnd = '# <<< Codex Session Hub <<<'
    $content = if (Test-Path $profilePath) { Get-Content -Path $profilePath -Raw } else { '' }
    $block = Get-CshProfileBlock

    $pattern = [regex]::Escape($markerStart) + '.*?' + [regex]::Escape($markerEnd)
    if ($content -match $pattern) {
        $updated = [regex]::Replace($content, $pattern, $block, [System.Text.RegularExpressions.RegexOptions]::Singleline)
    } elseif ([string]::IsNullOrWhiteSpace($content)) {
        $updated = $block
    } else {
        $updated = $content.TrimEnd() + [Environment]::NewLine + [Environment]::NewLine + $block
    }

    Set-Content -Path $profilePath -Value $updated
    Write-Output "Shell integration installed at $profilePath"
}

function Uninstall-CshShellIntegration {
    $profilePath = Get-CshProfilePath
    if (-not (Test-Path $profilePath)) {
        Write-Output "Profile not found at $profilePath"
        return
    }

    $markerStart = '# >>> Codex Session Hub >>>'
    $markerEnd = '# <<< Codex Session Hub <<<'
    $content = Get-Content -Path $profilePath -Raw
    $pattern = [regex]::Escape($markerStart) + '.*?' + [regex]::Escape($markerEnd)
    $updated = [regex]::Replace($content, $pattern, '', [System.Text.RegularExpressions.RegexOptions]::Singleline).Trim()

    if ($updated) {
        Set-Content -Path $profilePath -Value ($updated + [Environment]::NewLine)
    } else {
        Remove-Item -Path $profilePath -Force
    }

    Write-Output "Shell integration removed from $profilePath"
}

function Invoke-CshDoctor {
    $sessionRoot = Get-CshSessionRoot
    $configRoot = Get-CshConfigRoot
    $profilePath = Get-CshProfilePath
    $fzfAvailable = Test-CshFzfAvailable
    $codexAvailable = [bool](Get-CshCodexCommand)
    $profileInstalled = $false

    if (Test-Path $profilePath) {
        $profileInstalled = (Get-Content -Path $profilePath -Raw) -match '# >>> Codex Session Hub >>>'
    }

    [pscustomobject]@{
        PowerShellVersion = $PSVersionTable.PSVersion.ToString()
        SessionRoot       = $sessionRoot
        SessionRootExists = Test-Path $sessionRoot
        ConfigRoot        = $configRoot
        FzfAvailable      = $fzfAvailable
        CodexAvailable    = $codexAvailable
        ProfilePath       = $profilePath
        ProfileInstalled  = $profileInstalled
        InstallHelp       = if ($fzfAvailable) { 'fzf detected' } else { 'Install fzf, then rerun csx doctor. See README for platform commands.' }
    }
}
