function Get-CshProviderCommand {
    param([string]$Provider = 'codex')

    $binaryName = Get-CshProviderBinaryName -Provider $Provider
    return (Get-Command $binaryName -ErrorAction SilentlyContinue)
}

function Assert-CshProviderAvailable {
    param([string]$Provider = 'codex')

    if (Get-CshProviderCommand -Provider $Provider) {
        return
    }

    $binaryName = Get-CshProviderBinaryName -Provider $Provider
    throw "$binaryName was not found in PATH."
}

function Remove-CshSessions {
    param(
        [Parameter(Mandatory = $true)][object[]]$Sessions,
        [Parameter(Mandatory = $true)][hashtable]$Index,
        [string]$Provider = 'codex'
    )

    if (-not (Test-CshProviderSupportsDelete -Provider $Provider)) {
        throw "{0} session delete is not supported." -f (Get-CshProviderDisplayName -Provider $Provider)
    }

    $results = foreach ($session in $Sessions) {
        try {
            Remove-Item -Path $session.FilePath -Force -ErrorAction Stop
            Remove-CshAlias -Index $Index -SessionId $session.SessionId -Provider $provider
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

    Save-CshIndex -Index $Index -Provider $provider
    return @($results)
}

function Rename-CshSession {
    param(
        [Parameter(Mandatory = $true)][object]$Session,
        [Parameter(Mandatory = $true)][hashtable]$Index,
        [string]$Alias,
        [string]$Provider = 'codex'
    )

    Set-CshAlias -Index $Index -SessionId $Session.SessionId -Provider $Provider -Alias $Alias
    Save-CshIndex -Index $Index -Provider $Provider
}

function Invoke-CshCodexResume {
    param(
        [Parameter(Mandatory = $true)][object]$Session,
        [switch]$ShellMode
    )

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

function Resume-CshSession {
    param(
        [Parameter(Mandatory = $true)][object]$Session,
        [switch]$ShellMode,
        [string]$Provider = 'codex'
    )

    $providerName = Resolve-CshProviderName -Provider $Provider
    Assert-CshProviderAvailable -Provider $providerName

    switch ($providerName) {
        'codex' {
            Invoke-CshCodexResume -Session $Session -ShellMode:$ShellMode
        }
        'claude' {
            Invoke-CshClaudeResume -Session $Session -ShellMode:$ShellMode
        }
        default {
            throw "Unsupported provider: $providerName"
        }
    }
}

function Set-CshMarkedBlock {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Block,
        [Parameter(Mandatory = $true)][string]$MarkerStart,
        [Parameter(Mandatory = $true)][string]$MarkerEnd
    )

    Ensure-CshDirectory -Path (Split-Path -Parent $Path)
    $content = if (Test-Path $Path) { Get-Content -Path $Path -Raw } else { '' }
    $pattern = [regex]::Escape($MarkerStart) + '.*?' + [regex]::Escape($MarkerEnd)

    if ($content -match $pattern) {
        $updated = [regex]::Replace($content, $pattern, $Block, [System.Text.RegularExpressions.RegexOptions]::Singleline)
    } elseif ([string]::IsNullOrWhiteSpace($content)) {
        $updated = $Block
    } else {
        $updated = $content.TrimEnd() + [Environment]::NewLine + [Environment]::NewLine + $Block
    }

    Set-Content -Path $Path -Value $updated
}

function Remove-CshMarkedBlock {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$MarkerStart,
        [Parameter(Mandatory = $true)][string]$MarkerEnd
    )

    if (-not (Test-Path $Path)) {
        return $false
    }

    $content = Get-Content -Path $Path -Raw
    $pattern = [regex]::Escape($MarkerStart) + '.*?' + [regex]::Escape($MarkerEnd)
    $updated = [regex]::Replace($content, $pattern, '', [System.Text.RegularExpressions.RegexOptions]::Singleline).Trim()

    if ($updated) {
        Set-Content -Path $Path -Value ($updated + [Environment]::NewLine)
    } else {
        Remove-Item -Path $Path -Force
    }

    return $true
}

function Get-CshPowerShellProfileBlock {
    $modulePath = (Join-Path (Get-CshProjectRoot) 'src/AgentSessionHub.psd1').Replace("'", "''")
    $template = @'
# >>> Codex Session Hub >>>
$cshFzfPath = Join-Path $env:LOCALAPPDATA 'Programs\fzf\bin'
if ((Test-Path $cshFzfPath) -and (($env:Path -split ';') -notcontains $cshFzfPath)) {{
    $env:Path = "$cshFzfPath;$env:Path"
}}
function csx {{
    Import-Module '{0}' -Force
    Invoke-CsxCli -Arguments $args -ShellMode
}}
function clx {{
    Import-Module '{0}' -Force
    Invoke-ClxCli -Arguments $args -ShellMode
}}
Set-Alias cxs csx
# <<< Codex Session Hub <<<
'@

    return ($template -f $modulePath)
}

function Get-CshPosixLauncherContent {
    param([Parameter(Mandatory = $true)][string]$Name)

    $shimPath = (Join-Path (Get-CshProjectRoot) ('bin/{0}.ps1' -f $Name)).Replace('"', '\"')
    $template = @'
#!/usr/bin/env sh
exec pwsh -NoProfile -File "{0}" "$@"
'@

    return ($template -f $shimPath)
}

function Get-CshPosixFunctionBlock {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$BinaryName,
        [string[]]$PassthroughCommands = @()
    )

    $launcherPath = (Join-Path (Get-CshLauncherRoot) $Name).Replace('"', '\"')
    $caseLines = @()
    foreach ($commandName in $PassthroughCommands) {
        $caseLines += ('    {0})' -f $commandName)
        $caseLines += '      shift'
        $caseLines += '      ;;'
    }
    $caseBlock = $caseLines -join [Environment]::NewLine

    $template = @'
{0}() {{
  case "${{1-}}" in
{1}
    doctor|rename|reset|delete|help|install-shell|uninstall-shell|__*)
      "{2}" "$@"
      return $?
      ;;
  esac

  local _csh_result
  _csh_result="$("{2}" __select "$@")" || return $?
  [ -z "$_csh_result" ] && return 0

  local _csh_project="${{_csh_result%%	*}}"
  local _csh_session="${{_csh_result#*	}}"

  if [ -n "$_csh_project" ] && [ -d "$_csh_project" ]; then
    cd "$_csh_project" || return $?
  fi

  "{3}" --resume "$_csh_session"
}}
'@

    return ($template -f $Name, $caseBlock, $launcherPath, $BinaryName)
}

function Get-CshPosixPathBlock {
    $launcherRoot = (Get-CshLauncherRoot).Replace('"', '\"')
    $functions = @(
        Get-CshPosixFunctionBlock -Name 'csx' -BinaryName 'codex' -PassthroughCommands @('browse', 'delete')
        Get-CshPosixFunctionBlock -Name 'clx' -BinaryName 'claude' -PassthroughCommands @('browse')
        @'
cxs() {
  csx "$@"
}
'@
    )
    $template = @'
# >>> Codex Session Hub >>>
export PATH="{0}:$PATH"
{1}
# <<< Codex Session Hub <<<
'@

    return ($template -f $launcherRoot, ($functions -join [Environment]::NewLine + [Environment]::NewLine))
}

function Install-CshPowerShellShellIntegration {
    $profilePath = Get-CshProfilePath
    $markerStart = '# >>> Codex Session Hub >>>'
    $markerEnd = '# <<< Codex Session Hub <<<'
    Set-CshMarkedBlock -Path $profilePath -Block (Get-CshPowerShellProfileBlock) -MarkerStart $markerStart -MarkerEnd $markerEnd
    return $profilePath
}

function Install-CshPosixShellIntegration {
    $launcherRoot = Get-CshLauncherRoot
    Ensure-CshDirectory -Path $launcherRoot

    foreach ($name in @('csx', 'cxs', 'clx')) {
        $launcherPath = Join-Path $launcherRoot $name
        $contentName = if ($name -eq 'cxs') { 'csx' } else { $name }
        Set-Content -Path $launcherPath -Value (Get-CshPosixLauncherContent -Name $contentName)
        try {
            & chmod +x $launcherPath
        } catch {
        }
    }

    $profilePath = Get-CshShellProfilePath
    $markerStart = '# >>> Codex Session Hub >>>'
    $markerEnd = '# <<< Codex Session Hub <<<'
    Set-CshMarkedBlock -Path $profilePath -Block (Get-CshPosixPathBlock) -MarkerStart $markerStart -MarkerEnd $markerEnd

    return [pscustomobject]@{
        LauncherRoot = $launcherRoot
        ProfilePath  = $profilePath
    }
}

function Install-CshShellIntegration {
    if ($IsWindows) {
        $profilePath = Install-CshPowerShellShellIntegration
        Write-Output "Shell integration installed at $profilePath"
        return
    }

    $integration = Install-CshPosixShellIntegration
    Write-Output "Launchers installed at $($integration.LauncherRoot)"
    Write-Output "Shell integration installed at $($integration.ProfilePath)"
    Write-Output "Reload your shell with: source $($integration.ProfilePath)"
}

function Uninstall-CshShellIntegration {
    $markerStart = '# >>> Codex Session Hub >>>'
    $markerEnd = '# <<< Codex Session Hub <<<'

    if ($IsWindows) {
        $profilePath = Get-CshProfilePath
        if (-not (Remove-CshMarkedBlock -Path $profilePath -MarkerStart $markerStart -MarkerEnd $markerEnd)) {
            Write-Output "Profile not found at $profilePath"
            return
        }

        Write-Output "Shell integration removed from $profilePath"
        return
    }

    $profilePath = Get-CshShellProfilePath
    [void](Remove-CshMarkedBlock -Path $profilePath -MarkerStart $markerStart -MarkerEnd $markerEnd)

    foreach ($name in @('csx', 'cxs', 'clx')) {
        $launcherPath = Join-Path (Get-CshLauncherRoot) $name
        if (Test-Path $launcherPath) {
            Remove-Item -Path $launcherPath -Force
        }
    }

    Write-Output "Shell integration removed from $profilePath"
}

function Invoke-CshDoctor {
    param([string]$Provider = 'codex')

    $providerName = Resolve-CshProviderName -Provider $Provider
    $sessionRoot = Get-CshSessionRoot -Provider $providerName
    $configRoot = Get-CshConfigRoot
    $fzfAvailable = Test-CshFzfAvailable -Provider $providerName
    $binaryName = Get-CshProviderBinaryName -Provider $providerName
    $commandAvailable = [bool](Get-CshProviderCommand -Provider $providerName)
    $profileInstalled = $false
    $profilePath = Get-CshShellProfilePath
    $launcherPath = ''
    $launcherName = Get-CshProviderLauncherName -Provider $providerName

    if ($IsWindows) {
        if (Test-Path $profilePath) {
            $rawProfile = Get-Content -Path $profilePath -Raw
            $profileInstalled = $rawProfile -match "# >>> Codex Session Hub >>>" -and $rawProfile -match ("function {0}\s*\{{" -f [regex]::Escape($launcherName))
        }
    } else {
        $launcherPath = Join-Path (Get-CshLauncherRoot) $launcherName
        $profileInstalled = (Test-Path $launcherPath) -and (Test-Path $profilePath) -and ((Get-Content -Path $profilePath -Raw) -match '# >>> Codex Session Hub >>>')
    }

    [pscustomobject]@{
        Provider         = $providerName
        ProviderName     = Get-CshProviderDisplayName -Provider $providerName
        PowerShellVersion = $PSVersionTable.PSVersion.ToString()
        SessionRoot      = $sessionRoot
        SessionRootExists = Test-Path $sessionRoot
        ConfigRoot       = $configRoot
        FzfAvailable     = $fzfAvailable
        BinaryName       = $binaryName
        CommandAvailable = $commandAvailable
        ProfilePath      = $profilePath
        LauncherPath     = $launcherPath
        ProfileInstalled = $profileInstalled
        InstallHelp      = if ($fzfAvailable) { 'fzf detected' } else { "Install fzf, then rerun $launcherName doctor. See README for platform commands." }
    }
}
