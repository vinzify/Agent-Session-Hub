function Test-CshFzfAvailable {
    param([string]$Provider = 'codex')

    return [bool](Get-Command fzf -ErrorAction SilentlyContinue)
}

function Assert-CshFzfAvailable {
    param([string]$Provider = 'codex')

    if (Test-CshFzfAvailable -Provider $Provider) {
        return
    }

    $launcherName = Get-CshProviderLauncherName -Provider $Provider
    throw "fzf is required but was not found in PATH. Run '$launcherName doctor' for install help."
}

function New-CshSessionRowKey {
    param(
        [Parameter(Mandatory = $true)][string]$Provider,
        [Parameter(Mandatory = $true)][string]$SessionId
    )

    return 'S:{0}:{1}' -f (Resolve-CshProviderName -Provider $Provider), $SessionId
}

function ConvertTo-CshFzfRow {
    param([Parameter(Mandatory = $true)][object]$Session)

    $rowKey = New-CshSessionRowKey -Provider $Session.Provider -SessionId $Session.SessionId
    $fields = @(
        $rowKey
        $Session.DisplayNumber
        $Session.TimestampText
        (Compress-CshText -Text $Session.WorkspaceLabel -MaxLength 28)
        (Compress-CshText -Text $Session.DisplayTitle -MaxLength 90)
        $Session.ProjectPath
        $Session.Preview
    )

    return ($fields | ForEach-Object {
        ($_ -replace "`t", ' ') -replace '"', "'"
    }) -join "`t"
}

function ConvertFrom-CshFzfRow {
    param([Parameter(Mandatory = $true)][string]$Row)

    $parts = $Row -split "`t", 7
    $provider = ''
    $workspaceKey = ''
    $sessionId = ''
    if ($parts.Length -ge 1) {
        if ($parts[0] -match '^S:([^:]+):(.+)$') {
            $provider = Resolve-CshProviderName -Provider $Matches[1]
            $sessionId = $Matches[2]
        } elseif ($parts[0] -match '^W:([^:]+):([A-Za-z0-9+/=]+)$') {
            $provider = Resolve-CshProviderName -Provider $Matches[1]
            try {
                $workspaceKey = [System.Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Matches[2]))
            } catch {
                $workspaceKey = ''
            }
        } elseif ($parts[0] -match '^S:(.+)$') {
            # Backward-compatible row parsing for older tests/snapshots.
            $provider = 'codex'
            $sessionId = $Matches[1]
        } elseif ($parts[0] -match '^W:([A-Za-z0-9+/=]+)$') {
            $provider = 'codex'
            try {
                $workspaceKey = [System.Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Matches[1]))
            } catch {
                $workspaceKey = ''
            }
        }
    }

    return [pscustomobject]@{
        RowKey        = if ($parts.Length -ge 1) { $parts[0] } else { '' }
        Provider      = $provider
        SessionId     = $sessionId
        WorkspaceKey  = $workspaceKey
        DisplayNumber = if ($parts.Length -ge 2) { $parts[1] } else { '' }
        Timestamp     = if ($parts.Length -ge 3) { $parts[2] } else { '' }
        ProjectName   = if ($parts.Length -ge 4) { $parts[3] } else { '' }
        Title         = if ($parts.Length -ge 5) { $parts[4] } else { '' }
        ProjectPath   = if ($parts.Length -ge 6) { $parts[5] } else { '' }
        Preview       = if ($parts.Length -ge 7) { $parts[6] } else { '' }
    }
}

function New-CshWorkspaceRowKey {
    param(
        [Parameter(Mandatory = $true)][string]$WorkspaceKey,
        [string]$Provider = 'codex'
    )

    $bytes = [System.Text.Encoding]::UTF8.GetBytes($WorkspaceKey)
    return 'W:{0}:{1}' -f (Resolve-CshProviderName -Provider $Provider), [Convert]::ToBase64String($bytes)
}

function Get-CshPreviewCommand {
    param([string]$Provider = 'codex')

    if ($IsWindows) {
        $shimPath = Get-CshPreviewShimPath -Provider $Provider
        return ('"{0}" {{}}' -f $shimPath)
    }

    $shimPath = Get-CshShimPath -Provider $Provider
    return ('pwsh -NoProfile -File "{0}" __preview {{}}' -f $shimPath)
}

function Get-CshQueryCommand {
    param([string]$Provider = 'codex')

    if ($IsWindows) {
        $shimPath = Get-CshQueryShimPath -Provider $Provider
        return ('"{0}"' -f $shimPath)
    }

    $shimPath = Get-CshShimPath -Provider $Provider
    return ('pwsh -NoProfile -File "{0}" __query' -f $shimPath)
}

function Get-CshBrowserHeader {
    param([string]$Provider = 'codex')

    $keysLine = if (Test-CshProviderSupportsDelete -Provider $Provider) {
        'Keys: Enter open | Tab mark | Ctrl-E rename | Ctrl-R reset | Ctrl-D delete(confirm)'
    } else {
        'Keys: Enter open | Tab mark | Ctrl-E rename | Ctrl-R reset'
    }

    return @(
        'Find: text folder/repo | # number | title:term | repo:term | branch:term'
        $keysLine
    ) -join "`n"
}

function ConvertTo-CshFzfRows {
    param([object[]]$Sessions)

    $Sessions = @($Sessions)
    if ($Sessions.Count -eq 0) {
        return @()
    }

    $rows = New-Object 'System.Collections.Generic.List[string]'
    $groups = $Sessions | Group-Object { '{0}|{1}' -f $_.Provider, $_.GroupKey }
    $orderedProjects = foreach ($group in $groups) {
        $items = @($group.Group | Sort-Object @{ Expression = 'DisplayNumber'; Descending = $false })
        [pscustomobject]@{
            Provider       = $items[0].Provider
            GroupKey       = if (-not [string]::IsNullOrWhiteSpace([string]$items[0].GroupKey)) { $items[0].GroupKey } else { $items[0].ProjectKey }
            WorkspaceLabel = if (-not [string]::IsNullOrWhiteSpace([string]$items[0].WorkspaceLabel)) { $items[0].WorkspaceLabel } else { $items[0].ProjectName }
            ProjectPath    = $items[0].ProjectPath
            Items          = $items
        }
    }

    foreach ($project in $orderedProjects) {
        $headerKey = New-CshWorkspaceRowKey -WorkspaceKey $project.GroupKey -Provider $project.Provider
        $headerFields = @(
            $headerKey
            ''
            ''
            ('[{0}] {1}' -f $project.Items.Count, $project.WorkspaceLabel)
            (Compress-CshText -Text $project.ProjectPath -MaxLength 100)
            $project.ProjectPath
            ''
        )
        [void]$rows.Add(($headerFields | ForEach-Object {
            ($_ -replace "`t", ' ') -replace '"', "'"
        }) -join "`t")

        foreach ($session in $project.Items) {
            [void]$rows.Add((ConvertTo-CshFzfRow -Session $session))
        }
    }

    return $rows.ToArray()
}

function Invoke-CshFzfBrowser {
    param(
        [Parameter(Mandatory = $true)][object[]]$Sessions,
        [string]$InitialQuery,
        [string]$Provider = 'codex'
    )

    $providerName = Resolve-CshProviderName -Provider $Provider
    Assert-CshFzfAvailable -Provider $providerName

    $displaySessions = @(Get-CshDisplaySessions -Sessions $Sessions)
    if ($displaySessions.Count -eq 0) {
        return $null
    }

    $previewCommand = Get-CshPreviewCommand -Provider $providerName
    $queryCommand = Get-CshQueryCommand -Provider $providerName
    $bindings = @(
        "start:reload-sync($queryCommand),change:reload-sync($queryCommand {q})+first,enter:print(enter)+accept,ctrl-e:print(ctrl-e)+accept,ctrl-r:print(ctrl-r)+accept"
    )
    if (Test-CshProviderSupportsDelete -Provider $providerName) {
        $bindings[0] = $bindings[0] + ',ctrl-d:print(ctrl-d)+accept'
    }

    $fzfArgs = @(
        '--ansi'
        '--multi'
        '--disabled'
        '--layout=reverse'
        '--height=100%'
        '--border'
        '--delimiter'
        "`t"
        '--accept-nth'
        '1'
        '--with-nth'
        '2,3,4,5'
        '--nth'
        '2'
        '--preview'
        $previewCommand
        '--preview-window'
        'right:40%:wrap'
        '--bind'
        $bindings[0]
        '--header'
        (Get-CshBrowserHeader -Provider $providerName)
    )

    if ($InitialQuery) {
        $fzfArgs += @('--query', $InitialQuery)
    }

    if ($env:CODEX_SESSION_HUB_FZF_OPTS) {
        $fzfArgs += ($env:CODEX_SESSION_HUB_FZF_OPTS -split '\s+')
    }

    $output = @() | & fzf @fzfArgs
    if (-not $output) {
        return $null
    }

    $lines = @($output)
    $action = $lines[0]
    if (-not $action) {
        $action = 'enter'
    }

    $selectedRows = @($lines | Select-Object -Skip 1)
    if ($selectedRows.Count -eq 0 -and $lines.Count -eq 1 -and $action -notin @('enter', 'ctrl-d', 'ctrl-e', 'ctrl-r')) {
        $action = 'enter'
        $selectedRows = @($lines[0])
    }

    $sessionIds = @($selectedRows | ForEach-Object {
        if (-not [string]::IsNullOrWhiteSpace($_)) {
            $_.Trim()
        }
    } | Where-Object { $_ })

    return [pscustomobject]@{
        Action     = $action
        SessionIds = $sessionIds
    }
}

function Get-CshProjectPreviewLines {
    param([Parameter(Mandatory = $true)][object[]]$ProjectSessions)

    $latest = $ProjectSessions[0]
    $sessionNumbers = @($ProjectSessions | Select-Object -ExpandProperty DisplayNumber)
    $rangeText = if ($sessionNumbers.Count -gt 0) { ('#{0} -> #{1}' -f ($sessionNumbers | Measure-Object -Minimum).Minimum, ($sessionNumbers | Measure-Object -Maximum).Maximum) } else { '-' }
    $branchNames = @($ProjectSessions | ForEach-Object { [string]$_.BranchDisplay } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Sort-Object -Unique)
    $branchSummary = ''
    if ($branchNames.Count -eq 1) {
        $branchSummary = $branchNames[0]
    } elseif ($branchNames.Count -gt 1) {
        $visibleBranches = @($branchNames | Select-Object -First 3)
        $branchSummary = $visibleBranches -join ', '
        if ($branchNames.Count -gt $visibleBranches.Count) {
            $branchSummary = '{0} +{1} more' -f $branchSummary, ($branchNames.Count - $visibleBranches.Count)
        }
    }
    $recentLines = @($ProjectSessions | Select-Object -First 3 | ForEach-Object {
        '  {0}  {1}' -f $_.LastUpdatedAge.PadRight(7), (Compress-CshText -Text $_.DisplayTitle -MaxLength 52)
    })

    return @(
        (Format-CshAsciiBanner -Kind 'workspace' -Primary $latest.WorkspaceLabel -Secondary ('{0} sessions' -f $ProjectSessions.Count))
        ('Provider:{0}{1}' -f (' '), $latest.ProviderLabel)
        ('Path:    {0}' -f $latest.ProjectPath)
        $(if (-not [string]::IsNullOrWhiteSpace([string]$latest.RepoRoot)) { 'Repo:    {0}' -f $latest.RepoRoot })
        $(if ($branchSummary) { 'Branch:  {0}' -f $branchSummary })
        ('Exists:  {0}' -f $latest.ProjectExists)
        ('Latest:  {0} ({1})' -f $latest.LastUpdatedAge, $latest.LastUpdatedText)
        ('Started: {0}' -f $latest.TimestampText)
        ('Range:   {0}' -f $rangeText)
        ''
        'Recent'
        '------'
    ) + $recentLines
}

function Get-CshSessionPreviewLines {
    param(
        [Parameter(Mandatory = $true)][object]$Session,
        [int]$ProjectSessionCount = 0
    )

    $projectCountText = if ($ProjectSessionCount -gt 0) { '{0} sessions' -f $ProjectSessionCount } else { '' }

    return @(
        (Format-CshAsciiBanner -Kind 'session' -Primary ('#{0} {1}' -f $Session.DisplayNumber, $Session.WorkspaceLabel) -Secondary $Session.LastUpdatedAge)
        ('Provider:{0}{1}' -f (' '), $Session.ProviderLabel)
        ('Title:   {0}' -f $Session.DisplayTitle)
        ('Project: {0}' -f $Session.ProjectPath)
        $(if (-not [string]::IsNullOrWhiteSpace([string]$Session.RepoRoot)) { 'Repo:    {0}' -f $Session.RepoRoot })
        $(if (-not [string]::IsNullOrWhiteSpace([string]$Session.BranchDisplay)) { 'Branch:  {0}' -f $Session.BranchDisplay })
        ('Exists:  {0}' -f $Session.ProjectExists)
        $(if ($projectCountText) { 'Group:   {0}' -f $projectCountText })
        ('Started: {0}' -f $Session.TimestampText)
        ('Updated: {0} ({1})' -f $Session.LastUpdatedAge, $Session.LastUpdatedText)
        ('Session: {0}' -f $Session.SessionId)
        ''
        'Preview'
        '-------'
        $(if ($Session.Preview) { $Session.Preview } else { '<no meaningful preview>' })
    )
}

function Write-CshPreview {
    param(
        [AllowEmptyString()][string]$SessionId,
        [AllowEmptyString()][string]$WorkspaceKey,
        [AllowEmptyString()][string]$ProjectPath,
        [string]$Provider = 'codex'
    )

    $providerName = Resolve-CshProviderName -Provider $Provider
    $index = Get-CshIndex -Provider $providerName
    $sessions = @(Get-CshSessions -Index $index -Provider $providerName)
    $displaySessions = @(Get-CshDisplaySessions -Sessions $sessions)
    $projectSessions = @()

    if (-not [string]::IsNullOrWhiteSpace($WorkspaceKey)) {
        $projectSessions = @($displaySessions | Where-Object { $_.Provider -eq $providerName -and $_.GroupKey -eq $WorkspaceKey } | Sort-Object @{ Expression = 'Timestamp'; Descending = $true })
    } elseif (-not [string]::IsNullOrWhiteSpace($ProjectPath)) {
        $projectSessions = @($displaySessions | Where-Object { $_.Provider -eq $providerName -and $_.ProjectPath -eq $ProjectPath } | Sort-Object @{ Expression = 'Timestamp'; Descending = $true })
    }

    if (-not [string]::IsNullOrWhiteSpace($SessionId)) {
        $session = Find-CshSession -Sessions $displaySessions -SessionId $SessionId -Provider $providerName
        if ($session) {
            if ($projectSessions.Count -eq 0) {
                $projectSessions = @($displaySessions | Where-Object { $_.Provider -eq $providerName -and $_.GroupKey -eq $session.GroupKey } | Sort-Object @{ Expression = 'Timestamp'; Descending = $true })
            }

            $lines = @(Get-CshSessionPreviewLines -Session $session -ProjectSessionCount $projectSessions.Count)
            $lines -join [Environment]::NewLine | Write-Output
            return
        }
    }

    if ([string]::IsNullOrWhiteSpace($WorkspaceKey) -and [string]::IsNullOrWhiteSpace($ProjectPath)) {
        Write-Output ''
        return
    }

    if ($projectSessions.Count -eq 0) {
        Write-Output ''
        return
    }

    $lines = @(Get-CshProjectPreviewLines -ProjectSessions $projectSessions)

    $lines -join [Environment]::NewLine | Write-Output
}

function Write-CshQueryRows {
    param(
        [string]$Query,
        [string]$Provider = 'codex'
    )

    $providerName = Resolve-CshProviderName -Provider $Provider
    $index = Get-CshIndex -Provider $providerName
    $sessions = @(Get-CshSessions -Index $index -Provider $providerName)
    $filteredSessions = @(Get-CshFilteredDisplaySessions -Sessions $sessions -Query $Query)
    if ($filteredSessions.Count -eq 0) {
        return
    }

    $rows = @(ConvertTo-CshFzfRows -Sessions $filteredSessions)
    if ($rows.Count -gt 0) {
        $rows | Write-Output
    }
}
