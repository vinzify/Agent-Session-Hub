function Test-CshFzfAvailable {
    return [bool](Get-Command fzf -ErrorAction SilentlyContinue)
}

function Assert-CshFzfAvailable {
    if (Test-CshFzfAvailable) {
        return
    }

    throw "fzf is required but was not found in PATH. Run 'csx doctor' for install help."
}

function ConvertTo-CshFzfRow {
    param([Parameter(Mandatory = $true)][object]$Session)

    $rowKey = 'S:{0}' -f $Session.SessionId
    $fields = @(
        $rowKey
        $Session.DisplayNumber
        $Session.TimestampText
        (Compress-CshText -Text $Session.ProjectName -MaxLength 14)
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
    return [pscustomobject]@{
        RowKey      = if ($parts.Length -ge 1) { $parts[0] } else { '' }
        SessionId   = if (($parts.Length -ge 1) -and ($parts[0] -match '^S:(.+)$')) { $Matches[1] } else { '' }
        DisplayNumber = if ($parts.Length -ge 2) { $parts[1] } else { '' }
        Timestamp   = if ($parts.Length -ge 3) { $parts[2] } else { '' }
        ProjectName = if ($parts.Length -ge 4) { $parts[3] } else { '' }
        Title       = if ($parts.Length -ge 5) { $parts[4] } else { '' }
        ProjectPath = if ($parts.Length -ge 6) { $parts[5] } else { '' }
        Preview     = if ($parts.Length -ge 7) { $parts[6] } else { '' }
    }
}

function New-CshProjectRowKey {
    param([Parameter(Mandatory = $true)][string]$ProjectPath)

    $bytes = [System.Text.Encoding]::UTF8.GetBytes($ProjectPath)
    return 'P:{0}' -f [Convert]::ToBase64String($bytes)
}

function Get-CshPreviewCommand {
    if ($IsWindows) {
        $shimPath = Get-CshPreviewShimPath
        return ('"{0}" {{}}' -f $shimPath)
    }

    $shimPath = Get-CshShimPath
    return ('pwsh -NoProfile -File "{0}" __preview {{}}' -f $shimPath)
}

function Get-CshQueryCommand {
    if ($IsWindows) {
        $shimPath = Get-CshQueryShimPath
        return ('"{0}"' -f $shimPath)
    }

    $shimPath = Get-CshShimPath
    return ('pwsh -NoProfile -File "{0}" __query' -f $shimPath)
}

function ConvertTo-CshFzfRows {
    param([object[]]$Sessions)

    $Sessions = @($Sessions)
    if ($Sessions.Count -eq 0) {
        return @()
    }

    $rows = New-Object 'System.Collections.Generic.List[string]'
    $groups = $Sessions | Group-Object ProjectKey
    $orderedProjects = foreach ($group in $groups) {
        $items = @($group.Group | Sort-Object @{ Expression = 'DisplayNumber'; Descending = $false })
        [pscustomobject]@{
            ProjectName = $items[0].ProjectName
            ProjectPath = $items[0].ProjectPath
            Items       = $items
        }
    }

    foreach ($project in $orderedProjects) {
        $headerKey = New-CshProjectRowKey -ProjectPath $project.ProjectPath
        $headerFields = @(
            $headerKey
            ''
            ''
            ('[{0}] {1}' -f $project.Items.Count, $project.ProjectName)
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
        [string]$InitialQuery
    )

    Assert-CshFzfAvailable

    $displaySessions = @(Get-CshDisplaySessions -Sessions $Sessions)
    if ($displaySessions.Count -eq 0) {
        return $null
    }

    $previewCommand = Get-CshPreviewCommand
    $queryCommand = Get-CshQueryCommand

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
        "start:reload-sync($queryCommand),change:reload-sync($queryCommand {q})+first,enter:print(enter)+accept,ctrl-d:print(ctrl-d)+accept,ctrl-e:print(ctrl-e)+accept,ctrl-r:print(ctrl-r)+accept"
        '--header'
        'Text=folder | Number=# | title:term | Enter resume | Tab select | Ctrl-E rename | Ctrl-R reset | Ctrl-D delete'
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
    $recentLines = @($ProjectSessions | Select-Object -First 3 | ForEach-Object {
        '  {0}  {1}' -f $_.LastUpdatedAge.PadRight(7), (Compress-CshText -Text $_.DisplayTitle -MaxLength 52)
    })

    return @(
        (Format-CshAsciiBanner -Kind 'project' -Primary $latest.ProjectName -Secondary ('{0} sessions' -f $ProjectSessions.Count))
        ('Path:    {0}' -f $latest.ProjectPath)
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
        (Format-CshAsciiBanner -Kind 'session' -Primary ('#{0} {1}' -f $Session.DisplayNumber, $Session.ProjectName) -Secondary $Session.LastUpdatedAge)
        ('Title:   {0}' -f $Session.DisplayTitle)
        ('Project: {0}' -f $Session.ProjectPath)
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
        [AllowEmptyString()][string]$ProjectPath
    )

    $index = Get-CshIndex
    $sessions = @(Get-CshSessions -Index $index)
    $displaySessions = @(Get-CshDisplaySessions -Sessions $sessions)
    $projectSessions = @()

    if (-not [string]::IsNullOrWhiteSpace($ProjectPath)) {
        $projectSessions = @($displaySessions | Where-Object { $_.ProjectPath -eq $ProjectPath } | Sort-Object @{ Expression = 'Timestamp'; Descending = $true })
    }

    if (-not [string]::IsNullOrWhiteSpace($SessionId)) {
        $session = Find-CshSession -Sessions $displaySessions -SessionId $SessionId
        if ($session) {
            if ($projectSessions.Count -eq 0) {
                $projectSessions = @($displaySessions | Where-Object { $_.ProjectPath -eq $session.ProjectPath } | Sort-Object @{ Expression = 'Timestamp'; Descending = $true })
            }

            $lines = @(Get-CshSessionPreviewLines -Session $session -ProjectSessionCount $projectSessions.Count)
            $lines -join [Environment]::NewLine | Write-Output
            return
        }
    }

    if ([string]::IsNullOrWhiteSpace($ProjectPath)) {
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
    param([string]$Query)

    $index = Get-CshIndex
    $sessions = @(Get-CshSessions -Index $index)
    $filteredSessions = @(Get-CshFilteredDisplaySessions -Sessions $sessions -Query $Query)
    if ($filteredSessions.Count -eq 0) {
        return
    }

    $rows = @(ConvertTo-CshFzfRows -Sessions $filteredSessions)
    if ($rows.Count -gt 0) {
        $rows | Write-Output
    }
}
