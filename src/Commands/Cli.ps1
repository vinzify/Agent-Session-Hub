function Resolve-CshSelectedSessions {
    param(
        [Parameter(Mandatory = $true)][object[]]$AllSessions,
        [Parameter(Mandatory = $true)][string[]]$SessionIds
    )

    $resolved = New-Object 'System.Collections.Generic.List[object]'
    $seenSessionKeys = @{}

    foreach ($sessionId in $SessionIds) {
        $provider = ''
        if ($sessionId -match '^S:([^:]+):(.+)$') {
            $provider = Resolve-CshProviderName -Provider $Matches[1]
            $sessionId = $Matches[2]
        } elseif ($sessionId -match '^S:(.+)$') {
            $provider = 'codex'
            $sessionId = $Matches[1]
        } elseif ($sessionId -match '^W:([^:]+):([A-Za-z0-9+/=]+)$') {
            $provider = Resolve-CshProviderName -Provider $Matches[1]
            $workspaceKey = ''
            try {
                $workspaceKey = [System.Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Matches[2]))
            } catch {
                $workspaceKey = ''
            }

            if (-not [string]::IsNullOrWhiteSpace($workspaceKey)) {
                foreach ($session in @($AllSessions | Where-Object { $_.Provider -eq $provider -and $_.GroupKey -eq $workspaceKey })) {
                    $sessionKey = '{0}:{1}' -f $session.Provider, $session.SessionId
                    if (-not $seenSessionKeys.ContainsKey($sessionKey)) {
                        [void]$resolved.Add($session)
                        $seenSessionKeys[$sessionKey] = $true
                    }
                }
            }

            continue
        } elseif ($sessionId -match '^W:([A-Za-z0-9+/=]+)$') {
            $provider = 'codex'
            $workspaceKey = ''
            try {
                $workspaceKey = [System.Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Matches[1]))
            } catch {
                $workspaceKey = ''
            }

            if (-not [string]::IsNullOrWhiteSpace($workspaceKey)) {
                foreach ($session in @($AllSessions | Where-Object { $_.Provider -eq $provider -and $_.GroupKey -eq $workspaceKey })) {
                    $sessionKey = '{0}:{1}' -f $session.Provider, $session.SessionId
                    if (-not $seenSessionKeys.ContainsKey($sessionKey)) {
                        [void]$resolved.Add($session)
                        $seenSessionKeys[$sessionKey] = $true
                    }
                }
            }

            continue
        } elseif ($sessionId -match '^P:') {
            continue
        }

        $session = Find-CshSession -Sessions $AllSessions -SessionId $sessionId -Provider $provider
        if ($session) {
            $sessionKey = '{0}:{1}' -f $session.Provider, $session.SessionId
            if (-not $seenSessionKeys.ContainsKey($sessionKey)) {
                [void]$resolved.Add($session)
                $seenSessionKeys[$sessionKey] = $true
            }
        }
    }

    return $resolved.ToArray()
}

function Get-CshDeleteConfirmationLines {
    param(
        [Parameter(Mandatory = $true)][object[]]$Sessions
    )

    $sessions = @($Sessions)
    $workspaceLabels = @($sessions | ForEach-Object { [string]$_.WorkspaceLabel } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Sort-Object -Unique)
    $workspaceText = if ($workspaceLabels.Count -eq 1) {
        $workspaceLabels[0]
    } elseif ($workspaceLabels.Count -gt 1) {
        '{0} workspaces' -f $workspaceLabels.Count
    } else {
        '{0} sessions' -f $sessions.Count
    }

    $titleLines = @($sessions | Select-Object -First 2 | ForEach-Object {
        '  - #{0} {1}' -f $_.DisplayNumber, (Compress-CshText -Text $_.DisplayTitle -MaxLength 72)
    })

    if ($sessions.Count -gt 2) {
        $titleLines += '  - ... and {0} more' -f ($sessions.Count - 2)
    }

    return @(
        'Delete {0} session{1}' -f $sessions.Count, $(if ($sessions.Count -eq 1) { '' } else { 's' })
        'Workspace: {0}' -f $workspaceText
        'Targets:'
    ) + $titleLines
}

function Confirm-CshDeleteSelection {
    param(
        [Parameter(Mandatory = $true)][object[]]$Sessions
    )

    foreach ($line in @(Get-CshDeleteConfirmationLines -Sessions $Sessions)) {
        Write-Host $line
    }

    $response = Read-Host 'Confirm delete? [y/N]'
    return [string]$response -match '^(y|yes)$'
}

function Invoke-CshBrowseCommand {
    param(
        [string]$Query,
        [switch]$ShellMode,
        [switch]$EmitSelection,
        [string]$Provider = 'codex'
    )

    $providerName = Resolve-CshProviderName -Provider $Provider
    $initialQuery = $Query

    while ($true) {
        $index = Get-CshIndex -Provider $providerName
        $sessions = @(Get-CshSessions -Index $index -Provider $providerName)
        $displaySessions = @(Get-CshDisplaySessions -Sessions $sessions)
        $result = Invoke-CshFzfBrowser -Sessions $sessions -InitialQuery $initialQuery -Provider $providerName
        $initialQuery = ''

        if (-not $result) {
            return
        }

        $selectedSessions = @(Resolve-CshSelectedSessions -AllSessions $displaySessions -SessionIds $result.SessionIds)
        if ($selectedSessions.Count -eq 0) {
            continue
        }

        switch ($result.Action) {
            'enter' {
                if ($selectedSessions.Count -gt 1) {
                    throw 'Resume only supports one session at a time. Clear multi-select or choose a single row.'
                }

                if ($EmitSelection) {
                    $target = $selectedSessions[0]
                    Write-Output ("{0}`t{1}" -f $target.ProjectPath, $target.SessionId)
                    return
                }

                Resume-CshSession -Session $selectedSessions[0] -ShellMode:$ShellMode -Provider $providerName
                return
            }
            'ctrl-d' {
                if (-not (Test-CshProviderSupportsDelete -Provider $providerName)) {
                    throw "{0} session delete is not supported." -f (Get-CshProviderDisplayName -Provider $providerName)
                }

                if (-not (Confirm-CshDeleteSelection -Sessions $selectedSessions)) {
                    continue
                }

                [void]@(Remove-CshSessions -Sessions $selectedSessions -Index $index -Provider $providerName)
                continue
            }
            'ctrl-e' {
                $target = $selectedSessions[0]
                $alias = Read-Host ('Rename title for #{0} in {1} (blank resets)' -f $target.DisplayNumber, $target.ProjectName)
                Rename-CshSession -Session $target -Index $index -Alias $alias -Provider $providerName
                continue
            }
            'ctrl-r' {
                $target = $selectedSessions[0]
                Rename-CshSession -Session $target -Index $index -Alias '' -Provider $providerName
                continue
            }
            default {
                throw "Unsupported browser action: $($result.Action)"
            }
        }
    }
}

function Invoke-CshRenameCommand {
    param(
        [Parameter(Mandatory = $true)][string]$SessionId,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Alias,
        [string]$Provider = 'codex'
    )

    $providerName = Resolve-CshProviderName -Provider $Provider
    $index = Get-CshIndex -Provider $providerName
    $sessions = @(Get-CshSessions -Index $index -Provider $providerName)
    $session = Find-CshSession -Sessions $sessions -SessionId $SessionId -Provider $providerName
    if (-not $session) {
        throw "Session not found: $SessionId"
    }

    Rename-CshSession -Session $session -Index $index -Alias $Alias -Provider $providerName
    Write-Output ('Updated alias for {0}' -f $session.SessionId)
}

function Invoke-CshResetCommand {
    param(
        [Parameter(Mandatory = $true)][string]$SessionId,
        [string]$Provider = 'codex'
    )

    $providerName = Resolve-CshProviderName -Provider $Provider
    $index = Get-CshIndex -Provider $providerName
    $sessions = @(Get-CshSessions -Index $index -Provider $providerName)
    $session = Find-CshSession -Sessions $sessions -SessionId $SessionId -Provider $providerName
    if (-not $session) {
        throw "Session not found: $SessionId"
    }

    Rename-CshSession -Session $session -Index $index -Alias '' -Provider $providerName
    Write-Output ('Reset alias for {0}' -f $session.SessionId)
}

function Invoke-CshDeleteCommand {
    param(
        [Parameter(Mandatory = $true)][string[]]$SessionIds,
        [string]$Provider = 'codex'
    )

    $providerName = Resolve-CshProviderName -Provider $Provider
    if (-not (Test-CshProviderSupportsDelete -Provider $providerName)) {
        throw "{0} session delete is not supported." -f (Get-CshProviderDisplayName -Provider $providerName)
    }

    $index = Get-CshIndex -Provider $providerName
    $sessions = @(Get-CshSessions -Index $index -Provider $providerName)
    $targets = @(Resolve-CshSelectedSessions -AllSessions $sessions -SessionIds $SessionIds)
    if ($targets.Count -eq 0) {
        throw 'No matching sessions found.'
    }

    $results = @(Remove-CshSessions -Sessions $targets -Index $index -Provider $providerName)
    foreach ($entry in $results) {
        $prefix = if ($entry.Success) { '[deleted]' } else { '[failed]' }
        Write-Output ('{0} {1} {2}' -f $prefix, $entry.SessionId, $entry.Message)
    }
}

function Show-CshUsage {
    param([string]$Provider = 'codex')

    $launcherName = Get-CshProviderLauncherName -Provider $Provider
    $supportsDelete = Test-CshProviderSupportsDelete -Provider $Provider
    $lines = @(
        "$launcherName [query]"
        "$launcherName browse [query]"
        "$launcherName rename <session-id> --name <alias>"
        "$launcherName reset <session-id>"
    )

    if ($supportsDelete) {
        $lines += "$launcherName delete <session-id...>"
    }

    $lines += @(
        "$launcherName doctor"
        "$launcherName install-shell"
        "$launcherName uninstall-shell"
    )

    $lines | Write-Output
}

function Invoke-CshCli {
    param(
        [string[]]$Arguments,
        [switch]$ShellMode,
        [string]$Provider = 'codex'
    )

    $providerName = Resolve-CshProviderName -Provider $Provider
    $argsList = @($Arguments)
    if ($argsList.Count -eq 0) {
        Invoke-CshBrowseCommand -ShellMode:$ShellMode -Provider $providerName
        return
    }

    $command = $argsList[0]
    $rest = @($argsList | Select-Object -Skip 1)

    switch ($command) {
        '__preview' {
            $sessionId = ''
            $workspaceKey = ''
            $projectPath = ''

            if ($rest.Count -ge 1) {
                $rawValue = [string]$rest[0]
                if ($rawValue -match '^S:([^:]+):(.+)$') {
                    $sessionId = $Matches[2]
                } elseif ($rawValue -match '^S:(.+)$') {
                    $sessionId = $Matches[1]
                } elseif ($rawValue -match '^W:([^:]+):([A-Za-z0-9+/=]+)$') {
                    try {
                        $workspaceKey = [System.Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Matches[2]))
                    } catch {
                        $workspaceKey = ''
                    }
                } elseif ($rawValue -match '^W:([A-Za-z0-9+/=]+)$') {
                    try {
                        $workspaceKey = [System.Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Matches[1]))
                    } catch {
                        $workspaceKey = ''
                    }
                } elseif ($rawValue.Contains("`t")) {
                    $row = ConvertFrom-CshFzfRow -Row $rawValue
                    $sessionId = $row.SessionId
                    $workspaceKey = $row.WorkspaceKey
                    $projectPath = $row.ProjectPath
                } else {
                    $sessionId = $rawValue
                    if ($rest.Count -ge 2) {
                        $projectPath = $rest[1]
                    }

                    if ([string]::IsNullOrWhiteSpace($projectPath) -and -not [string]::IsNullOrWhiteSpace($sessionId) -and ($sessionId.Contains('\') -or $sessionId.Contains(':'))) {
                        $projectPath = $sessionId
                        $sessionId = ''
                    }
                }
            }

            Write-CshPreview -SessionId $sessionId -WorkspaceKey $workspaceKey -ProjectPath $projectPath -Provider $providerName
        }
        '__query' {
            $query = if ($rest.Count -gt 0) { $rest -join ' ' } elseif ($env:FZF_QUERY) { $env:FZF_QUERY } else { '' }
            Write-CshQueryRows -Query $query -Provider $providerName
        }
        '__select' {
            $query = if ($rest.Count -gt 0) { $rest -join ' ' } else { '' }
            Invoke-CshBrowseCommand -Query $query -EmitSelection -Provider $providerName
        }
        'browse' {
            $query = if ($rest.Count -gt 0) { $rest -join ' ' } else { '' }
            Invoke-CshBrowseCommand -Query $query -ShellMode:$ShellMode -Provider $providerName
        }
        'rename' {
            if ($rest.Count -lt 1) {
                throw 'rename requires a session id.'
            }

            $sessionId = $rest[0]
            $nameIndex = [Array]::IndexOf($rest, '--name')
            if ($nameIndex -lt 0 -or ($nameIndex + 1) -ge $rest.Count) {
                throw "rename requires --name <alias>."
            }

            Invoke-CshRenameCommand -SessionId $sessionId -Alias $rest[$nameIndex + 1] -Provider $providerName
        }
        'reset' {
            if ($rest.Count -lt 1) {
                throw 'reset requires a session id.'
            }

            Invoke-CshResetCommand -SessionId $rest[0] -Provider $providerName
        }
        'delete' {
            if ($rest.Count -lt 1) {
                throw 'delete requires at least one session id.'
            }

            Invoke-CshDeleteCommand -SessionIds $rest -Provider $providerName
        }
        'doctor' {
            Invoke-CshDoctor -Provider $providerName | Format-List
        }
        'install-shell' {
            Install-CshShellIntegration
        }
        'uninstall-shell' {
            Uninstall-CshShellIntegration
        }
        'help' {
            Show-CshUsage -Provider $providerName
        }
        default {
            Invoke-CshBrowseCommand -Query ($argsList -join ' ') -ShellMode:$ShellMode -Provider $providerName
        }
    }
}

function Invoke-CsxCli {
    param(
        [string[]]$Arguments,
        [switch]$ShellMode
    )

    Invoke-CshCli -Arguments $Arguments -ShellMode:$ShellMode -Provider 'codex'
}

function Invoke-ClxCli {
    param(
        [string[]]$Arguments,
        [switch]$ShellMode
    )

    Invoke-CshCli -Arguments $Arguments -ShellMode:$ShellMode -Provider 'claude'
}
