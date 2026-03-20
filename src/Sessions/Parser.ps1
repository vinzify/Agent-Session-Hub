function Test-CshMeaningfulUserText {
    param([string]$Text)

    if ([string]::IsNullOrWhiteSpace($Text)) {
        return $false
    }

    $clean = ($Text -replace '\s+', ' ').Trim()
    if ($clean.Length -lt 12) {
        return $false
    }

    $ignorePrefixes = @(
        '<environment_context>'
        '# AGENTS.md'
        'AGENTS.md'
    )

    foreach ($prefix in $ignorePrefixes) {
        if ($clean.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
            return $false
        }
    }

    if ($clean -match '^\s*<\w+') {
        return $false
    }

    return $true
}

function Get-CshPreviewCandidate {
    param([object]$Entry)

    if ($Entry.type -eq 'event_msg' -and $Entry.payload.type -eq 'user_message' -and $Entry.payload.message) {
        return [string]$Entry.payload.message
    }

    if ($Entry.type -eq 'response_item' -and $Entry.payload.type -eq 'message' -and $Entry.payload.role -eq 'user') {
        foreach ($contentItem in $Entry.payload.content) {
            if ($contentItem.type -eq 'input_text' -and $contentItem.text) {
                return [string]$contentItem.text
            }
        }
    }

    return ''
}

function Read-CshSessionFile {
    param(
        [Parameter(Mandatory = $true)][System.IO.FileInfo]$File,
        [Parameter(Mandatory = $true)][hashtable]$Index
    )

    $meta = $null
    $preview = ''
    $fallbackPreview = ''
    $stream = $null
    $reader = $null

    try {
        $stream = [System.IO.File]::Open($File.FullName, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
        $reader = [System.IO.StreamReader]::new($stream)

        while (-not $reader.EndOfStream) {
            $line = $reader.ReadLine()
            if ([string]::IsNullOrWhiteSpace($line)) {
                continue
            }

            try {
                $entry = $line | ConvertFrom-Json -Depth 20
            } catch {
                continue
            }

            if (-not $meta -and $entry.type -eq 'session_meta') {
                $meta = $entry.payload
            }

            $candidate = Get-CshPreviewCandidate -Entry $entry
            if (-not [string]::IsNullOrWhiteSpace($candidate)) {
                if ([string]::IsNullOrWhiteSpace($fallbackPreview)) {
                    $fallbackPreview = $candidate
                }

                if ((-not $preview) -and (Test-CshMeaningfulUserText -Text $candidate)) {
                    $preview = $candidate
                }
            }

            if ($meta -and $preview) {
                break
            }
        }
    } finally {
        if ($reader) {
            $reader.Dispose()
        }
        if ($stream) {
            $stream.Dispose()
        }
    }

    if (-not $meta -or -not $meta.id) {
        return $null
    }

    $timestamp = $null
    if ($meta.timestamp) {
        try {
            $timestamp = [datetimeoffset]::Parse([string]$meta.timestamp)
        } catch {
            $timestamp = $null
        }
    }

    if (-not $timestamp) {
        $timestamp = [datetimeoffset]$File.LastWriteTimeUtc
    }

    if (-not $preview) {
        $preview = $fallbackPreview
    }

    $projectPath = Normalize-CshPath ([string]$meta.cwd)
    $alias = Get-CshAlias -Index $Index -SessionId ([string]$meta.id)
    $previewText = Compress-CshText -Text $preview -MaxLength 160
    $displayTitle = if ($alias) { $alias } elseif ($previewText) { $previewText } else { 'Session {0}' -f $meta.id }

    return [pscustomobject]@{
        SessionId         = [string]$meta.id
        Timestamp         = $timestamp
        TimestampText     = Format-CshTimestamp -Timestamp $timestamp
        LastUpdated       = [datetimeoffset]$File.LastWriteTimeUtc
        LastUpdatedText   = Format-CshTimestamp -Timestamp ([datetimeoffset]$File.LastWriteTimeUtc)
        LastUpdatedAge    = Format-CshRelativeAge -Timestamp ([datetimeoffset]$File.LastWriteTimeUtc)
        ProjectPath       = $projectPath
        ProjectKey        = $projectPath.ToLowerInvariant()
        ProjectName       = Get-CshProjectName -ProjectPath $projectPath
        FilePath          = $File.FullName
        ProjectExists     = [bool](Test-Path $projectPath)
        Alias             = $alias
        Preview           = $previewText
        DisplayTitle      = $displayTitle
    }
}

function Get-CshSessions {
    param([hashtable]$Index = $(Get-CshIndex))

    $sessionRoot = Get-CshSessionRoot
    if (-not (Test-Path $sessionRoot)) {
        return @()
    }

    $files = Get-ChildItem -Path $sessionRoot -Recurse -File -Filter '*.jsonl' | Sort-Object LastWriteTime -Descending
    $sessions = foreach ($file in $files) {
        $session = Read-CshSessionFile -File $file -Index $Index
        if ($session) {
            $session
        }
    }

    return @($sessions | Sort-Object @{ Expression = 'Timestamp'; Descending = $true }, @{ Expression = 'ProjectPath'; Descending = $false })
}

function Get-CshDisplaySessions {
    param([Parameter(Mandatory = $true)][object[]]$Sessions)

    $groups = $Sessions | Group-Object ProjectKey
    $orderedProjects = foreach ($group in $groups) {
        $items = @($group.Group | Sort-Object @{ Expression = 'Timestamp'; Descending = $true })
        [pscustomobject]@{
            ProjectName = $items[0].ProjectName
            ProjectPath = $items[0].ProjectPath
            LatestTime  = $items[0].Timestamp
            Items       = $items
        }
    }

    $display = New-Object 'System.Collections.Generic.List[object]'
    foreach ($project in ($orderedProjects | Sort-Object @{ Expression = 'LatestTime'; Descending = $true }, @{ Expression = 'ProjectPath'; Descending = $false })) {
        foreach ($session in $project.Items) {
            $displayNumber = $display.Count + 1
            [void]$display.Add([pscustomobject]@{
                SessionId       = $session.SessionId
                DisplayNumber   = $displayNumber
                Timestamp       = $session.Timestamp
                TimestampText   = $session.TimestampText
                LastUpdated     = $session.LastUpdated
                LastUpdatedText = $session.LastUpdatedText
                LastUpdatedAge  = $session.LastUpdatedAge
                ProjectPath     = $session.ProjectPath
                ProjectKey      = $session.ProjectKey
                ProjectName     = $session.ProjectName
                FilePath        = $session.FilePath
                ProjectExists   = $session.ProjectExists
                Alias           = $session.Alias
                Preview         = $session.Preview
                DisplayTitle    = $session.DisplayTitle
            })
        }
    }

    return $display.ToArray()
}

function Get-CshFilteredDisplaySessions {
    param(
        [Parameter(Mandatory = $true)][object[]]$Sessions,
        [string]$Query
    )

    $displaySessions = @(Get-CshDisplaySessions -Sessions $Sessions)
    $normalizedQuery = if ($null -eq $Query) { '' } else { [string]$Query }
    $normalizedQuery = $normalizedQuery.Trim()
    if ($normalizedQuery -match '^[\s"]*$') {
        $normalizedQuery = ''
    }

    if ([string]::IsNullOrWhiteSpace($normalizedQuery)) {
        return $displaySessions
    }

    $trimmedQuery = $normalizedQuery
    if ($trimmedQuery -match '^\d+$') {
        return @($displaySessions | Where-Object {
            [string]$_.DisplayNumber -like "$trimmedQuery*"
        })
    }

    $searchTitles = $false
    $textQuery = $trimmedQuery
    if ($trimmedQuery -match '^(t:|title:)\s*(.+)$') {
        $searchTitles = $true
        $textQuery = $Matches[2].Trim()
    }

    if ([string]::IsNullOrWhiteSpace($textQuery)) {
        return $displaySessions
    }

    $lowerQuery = $textQuery.ToLowerInvariant()
    if ($searchTitles) {
        return @($displaySessions | Where-Object {
            $_.DisplayTitle.ToLowerInvariant().Contains($lowerQuery)
        })
    }

    return @($displaySessions | Where-Object {
        $_.ProjectName.ToLowerInvariant().Contains($lowerQuery)
    })
}

function Find-CshSession {
    param(
        [Parameter(Mandatory = $true)][object[]]$Sessions,
        [Parameter(Mandatory = $true)][string]$SessionId
    )

    $exact = @($Sessions | Where-Object { $_.SessionId -eq $SessionId })
    if ($exact.Count -eq 1) {
        return $exact[0]
    }

    $prefix = @($Sessions | Where-Object { $_.SessionId -like "$SessionId*" })
    if ($prefix.Count -eq 1) {
        return $prefix[0]
    }

    return $null
}
