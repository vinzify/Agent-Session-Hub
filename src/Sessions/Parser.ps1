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

function Get-CshObjectPropertyValue {
    param(
        [object]$InputObject,
        [Parameter(Mandatory = $true)][string]$Name
    )

    if ($null -eq $InputObject) {
        return $null
    }

    $property = $InputObject.PSObject.Properties[$Name]
    if ($null -eq $property) {
        return $null
    }

    return $property.Value
}

function Get-CshCodexPreviewCandidate {
    param([object]$Entry)

    $entryType = [string](Get-CshObjectPropertyValue -InputObject $Entry -Name 'type')
    $payload = Get-CshObjectPropertyValue -InputObject $Entry -Name 'payload'
    if ($entryType -eq 'event_msg' -and [string](Get-CshObjectPropertyValue -InputObject $payload -Name 'type') -eq 'user_message') {
        $message = Get-CshObjectPropertyValue -InputObject $payload -Name 'message'
        if ($message) {
            return [string]$message
        }
    }

    if ($entryType -eq 'response_item' -and [string](Get-CshObjectPropertyValue -InputObject $payload -Name 'type') -eq 'message' -and [string](Get-CshObjectPropertyValue -InputObject $payload -Name 'role') -eq 'user') {
        foreach ($contentItem in @(Get-CshObjectPropertyValue -InputObject $payload -Name 'content')) {
            if ($contentItem.type -eq 'input_text' -and $contentItem.text) {
                return [string]$contentItem.text
            }
        }
    }

    return ''
}

function ConvertFrom-CshClaudeMessageContent {
    param([object]$Content)

    if ($null -eq $Content) {
        return ''
    }

    if ($Content -is [string]) {
        return [string]$Content
    }

    foreach ($item in @($Content)) {
        $itemType = [string](Get-CshObjectPropertyValue -InputObject $item -Name 'type')
        $itemText = Get-CshObjectPropertyValue -InputObject $item -Name 'text'
        if (($itemType -eq 'text' -or $itemType -eq 'input_text') -and $itemText) {
            return [string]$itemText
        }
    }

    return ''
}

function Get-CshClaudePreviewCandidate {
    param([object]$Entry)

    if ([string](Get-CshObjectPropertyValue -InputObject $Entry -Name 'type') -eq 'user') {
        $message = Get-CshObjectPropertyValue -InputObject $Entry -Name 'message'
        if ($message) {
            return ConvertFrom-CshClaudeMessageContent -Content (Get-CshObjectPropertyValue -InputObject $message -Name 'content')
        }
    }

    return ''
}

function Get-CshBranchDisplay {
    param(
        [string]$BranchName,
        [bool]$IsDetachedHead
    )

    if (-not [string]::IsNullOrWhiteSpace($BranchName)) {
        return $BranchName.Trim()
    }

    if ($IsDetachedHead) {
        return 'detached'
    }

    return ''
}

function Get-CshWorkspaceKey {
    param(
        [string]$RepoRoot,
        [string]$BranchName,
        [string]$ProjectPath
    )

    $parts = foreach ($value in @($RepoRoot, $BranchName, $ProjectPath)) {
        if (-not [string]::IsNullOrWhiteSpace($value)) {
            $value.Trim().ToLowerInvariant()
        }
    }

    if (@($parts).Count -eq 0) {
        return ''
    }

    return (@($parts) -join '|')
}

function Get-CshDisplayGroupKey {
    param([Parameter(Mandatory = $true)][object]$Session)

    if (-not [string]::IsNullOrWhiteSpace([string]$Session.WorkspaceKey)) {
        return [string]$Session.WorkspaceKey
    }

    return [string]$Session.ProjectKey
}

function Get-CshWorkspaceLabel {
    param(
        [string]$RepoName,
        [string]$BranchDisplay,
        [string]$ProjectName,
        [string]$ProjectPath,
        [string]$RepoRoot
    )

    if ([string]::IsNullOrWhiteSpace($RepoName)) {
        return $ProjectName
    }

    $label = $RepoName
    if (-not [string]::IsNullOrWhiteSpace($BranchDisplay)) {
        $label = '{0} @ {1}' -f $label, $BranchDisplay
    }

    if (-not [string]::IsNullOrWhiteSpace($ProjectPath) -and -not [string]::IsNullOrWhiteSpace($RepoRoot)) {
        $normalizedProjectPath = Normalize-CshPath $ProjectPath
        $normalizedRepoRoot = Normalize-CshPath $RepoRoot
        if ($normalizedProjectPath -and $normalizedRepoRoot -and ($normalizedProjectPath -ne $normalizedRepoRoot)) {
            $leaf = Get-CshProjectName -ProjectPath $normalizedProjectPath
            if (-not [string]::IsNullOrWhiteSpace($leaf) -and ($leaf -ne $RepoName)) {
                $label = '{0} / {1}' -f $label, $leaf
            }
        }
    }

    return $label
}

function Get-CshGitContext {
    param(
        [string]$Path,
        [hashtable]$Cache
    )

    $normalizedPath = Normalize-CshPath $Path
    $defaultWorkspaceKey = if ([string]::IsNullOrWhiteSpace($normalizedPath)) { '' } else { $normalizedPath.ToLowerInvariant() }
    $emptyContext = [pscustomobject]@{
        RepoRoot       = ''
        RepoName       = ''
        BranchName     = ''
        BranchDisplay  = ''
        IsDetachedHead = $false
        WorkspaceKey   = $defaultWorkspaceKey
    }

    if ([string]::IsNullOrWhiteSpace($normalizedPath)) {
        return $emptyContext
    }

    if ($Cache -and $Cache.ContainsKey($normalizedPath)) {
        return $Cache[$normalizedPath]
    }

    $context = $emptyContext
    $git = Get-Command git -ErrorAction SilentlyContinue
    if ($git -and (Test-Path -LiteralPath $normalizedPath)) {
        try {
            $repoRootOutput = & $git.Source -C $normalizedPath rev-parse --show-toplevel 2>$null
            if ($LASTEXITCODE -eq 0 -and $repoRootOutput) {
                $repoRoot = Normalize-CshPath ([string]($repoRootOutput | Select-Object -First 1))
                $branchOutput = & $git.Source -C $normalizedPath branch --show-current 2>$null
                $branchName = if ($LASTEXITCODE -eq 0 -and $branchOutput) { [string]($branchOutput | Select-Object -First 1) } else { '' }
                $branchName = $branchName.Trim()
                $branchDisplay = Get-CshBranchDisplay -BranchName $branchName -IsDetachedHead ([string]::IsNullOrWhiteSpace($branchName))

                $context = [pscustomobject]@{
                    RepoRoot       = $repoRoot
                    RepoName       = Get-CshProjectName -ProjectPath $repoRoot
                    BranchName     = $branchName
                    BranchDisplay  = $branchDisplay
                    IsDetachedHead = [string]::IsNullOrWhiteSpace($branchName)
                    WorkspaceKey   = Get-CshWorkspaceKey -RepoRoot $repoRoot -BranchName $branchDisplay -ProjectPath $normalizedPath
                }
            }
        } catch {
            $context = $emptyContext
        }
    }

    if ($Cache) {
        $Cache[$normalizedPath] = $context
    }

    return $context
}

function New-CshSessionRecord {
    param(
        [Parameter(Mandatory = $true)][string]$Provider,
        [Parameter(Mandatory = $true)][string]$SessionId,
        [Parameter(Mandatory = $true)][datetimeoffset]$Timestamp,
        [Parameter(Mandatory = $true)][datetimeoffset]$LastUpdated,
        [string]$ProjectPath,
        [string]$Preview,
        [string]$Alias,
        [string]$FilePath,
        [string]$RecordedBranchName,
        [bool]$RecordedDetachedHead,
        [string]$Slug,
        [hashtable]$GitContextCache
    )

    $providerName = Resolve-CshProviderName -Provider $Provider
    $normalizedProjectPath = Normalize-CshPath $ProjectPath
    $projectName = Get-CshProjectName -ProjectPath $normalizedProjectPath
    $gitContext = Get-CshGitContext -Path $normalizedProjectPath -Cache $GitContextCache
    $shouldUseRecordedBranch = (-not [string]::IsNullOrWhiteSpace($RecordedBranchName)) -or $RecordedDetachedHead

    if ($shouldUseRecordedBranch) {
        $branchName = if ($RecordedDetachedHead) { '' } else { $RecordedBranchName.Trim() }
        $branchDisplay = Get-CshBranchDisplay -BranchName $branchName -IsDetachedHead $RecordedDetachedHead
        $gitContext = [pscustomobject]@{
            RepoRoot       = $gitContext.RepoRoot
            RepoName       = $gitContext.RepoName
            BranchName     = $branchName
            BranchDisplay  = $branchDisplay
            IsDetachedHead = $RecordedDetachedHead
            WorkspaceKey   = Get-CshWorkspaceKey -RepoRoot $gitContext.RepoRoot -BranchName $branchDisplay -ProjectPath $normalizedProjectPath
        }
    }

    $previewText = Compress-CshText -Text $Preview -MaxLength 160
    $displayTitle = if ($Alias) {
        $Alias
    } elseif ($previewText) {
        $previewText
    } elseif (-not [string]::IsNullOrWhiteSpace($Slug)) {
        $Slug
    } else {
        '{0} session {1}' -f (Get-CshProviderDisplayName -Provider $providerName), $SessionId
    }

    return [pscustomobject]@{
        Provider          = $providerName
        ProviderLabel     = Get-CshProviderDisplayName -Provider $providerName
        SupportsDelete    = Test-CshProviderSupportsDelete -Provider $providerName
        SessionId         = $SessionId
        Timestamp         = $Timestamp
        TimestampText     = Format-CshTimestamp -Timestamp $Timestamp
        LastUpdated       = $LastUpdated
        LastUpdatedText   = Format-CshTimestamp -Timestamp $LastUpdated
        LastUpdatedAge    = Format-CshRelativeAge -Timestamp $LastUpdated
        ProjectPath       = $normalizedProjectPath
        ProjectKey        = $normalizedProjectPath.ToLowerInvariant()
        ProjectName       = $projectName
        RepoRoot          = $gitContext.RepoRoot
        RepoName          = $gitContext.RepoName
        BranchName        = $gitContext.BranchName
        BranchDisplay     = $gitContext.BranchDisplay
        IsDetachedHead    = $gitContext.IsDetachedHead
        WorkspaceKey      = $gitContext.WorkspaceKey
        WorkspaceLabel    = Get-CshWorkspaceLabel -RepoName $gitContext.RepoName -BranchDisplay $gitContext.BranchDisplay -ProjectName $projectName -ProjectPath $normalizedProjectPath -RepoRoot $gitContext.RepoRoot
        FilePath          = $FilePath
        ProjectExists     = [bool]([string]::IsNullOrWhiteSpace($normalizedProjectPath) -eq $false -and (Test-Path $normalizedProjectPath))
        Alias             = $Alias
        Preview           = $previewText
        DisplayTitle      = $displayTitle
        Slug              = $Slug
    }
}

function Read-CshCodexSessionFile {
    param(
        [Parameter(Mandatory = $true)][System.IO.FileInfo]$File,
        [Parameter(Mandatory = $true)][hashtable]$Index,
        [hashtable]$GitContextCache
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

            if (-not $meta -and [string](Get-CshObjectPropertyValue -InputObject $entry -Name 'type') -eq 'session_meta') {
                $meta = Get-CshObjectPropertyValue -InputObject $entry -Name 'payload'
            }

            $candidate = Get-CshCodexPreviewCandidate -Entry $entry
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

    if (-not $meta) {
        return $null
    }

    $metaId = [string](Get-CshObjectPropertyValue -InputObject $meta -Name 'id')
    if ([string]::IsNullOrWhiteSpace($metaId)) {
        return $null
    }

    $timestamp = $null
    $metaTimestamp = Get-CshObjectPropertyValue -InputObject $meta -Name 'timestamp'
    if ($metaTimestamp) {
        try {
            $timestamp = [datetimeoffset]::Parse([string]$metaTimestamp)
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

    return New-CshSessionRecord `
        -Provider 'codex' `
        -SessionId $metaId `
        -Timestamp $timestamp `
        -LastUpdated ([datetimeoffset]$File.LastWriteTimeUtc) `
        -ProjectPath ([string](Get-CshObjectPropertyValue -InputObject $meta -Name 'cwd')) `
        -Preview $preview `
        -Alias (Get-CshAlias -Index $Index -SessionId $metaId -Provider 'codex') `
        -FilePath $File.FullName `
        -GitContextCache $GitContextCache
}

function Get-CshClaudeHistoryPath {
    return (Join-Path $HOME '.claude\history.jsonl')
}

function Get-CshClaudeHistoryIndex {
    $historyPath = Get-CshClaudeHistoryPath
    if (-not (Test-Path -LiteralPath $historyPath)) {
        return @{}
    }

    $index = @{}
    $stream = $null
    $reader = $null

    try {
        $stream = [System.IO.File]::Open($historyPath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
        $reader = [System.IO.StreamReader]::new($stream)

        while (-not $reader.EndOfStream) {
            $line = $reader.ReadLine()
            if ([string]::IsNullOrWhiteSpace($line)) {
                continue
            }

            try {
                $entry = $line | ConvertFrom-Json -Depth 10
            } catch {
                continue
            }

            $historySessionId = [string](Get-CshObjectPropertyValue -InputObject $entry -Name 'sessionId')
            if ([string]::IsNullOrWhiteSpace($historySessionId)) {
                continue
            }

            $index[$historySessionId] = $entry
        }
    } finally {
        if ($reader) {
            $reader.Dispose()
        }
        if ($stream) {
            $stream.Dispose()
        }
    }

    return $index
}

function Read-CshClaudeSessionFile {
    param(
        [Parameter(Mandatory = $true)][System.IO.FileInfo]$File,
        [Parameter(Mandatory = $true)][hashtable]$Index,
        [hashtable]$GitContextCache,
        [hashtable]$HistoryIndex
    )

    $sessionId = ''
    $projectPath = ''
    $preview = ''
    $fallbackPreview = ''
    $timestamp = $null
    $lastUpdated = $null
    $recordedBranchName = ''
    $recordedDetachedHead = $false
    $slug = ''
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
                $entry = $line | ConvertFrom-Json -Depth 30
            } catch {
                continue
            }

            $entrySessionId = [string](Get-CshObjectPropertyValue -InputObject $entry -Name 'sessionId')
            if (-not $sessionId -and -not [string]::IsNullOrWhiteSpace($entrySessionId)) {
                $sessionId = $entrySessionId
            }

            $entryCwd = Get-CshObjectPropertyValue -InputObject $entry -Name 'cwd'
            if (-not $projectPath -and $entryCwd) {
                $projectPath = [string]$entryCwd
            }

            $entrySlug = Get-CshObjectPropertyValue -InputObject $entry -Name 'slug'
            if (-not $slug -and $entrySlug) {
                $slug = [string]$entrySlug
            }

            $entryGitBranch = Get-CshObjectPropertyValue -InputObject $entry -Name 'gitBranch'
            if ($entryGitBranch) {
                $rawBranch = ([string]$entryGitBranch).Trim()
                if ($rawBranch -eq 'HEAD') {
                    $recordedDetachedHead = $true
                    $recordedBranchName = ''
                } elseif (-not [string]::IsNullOrWhiteSpace($rawBranch)) {
                    $recordedBranchName = $rawBranch
                    $recordedDetachedHead = $false
                }
            }

            $entryTimestampRaw = Get-CshObjectPropertyValue -InputObject $entry -Name 'timestamp'
            if ($entryTimestampRaw) {
                try {
                    $entryTimestamp = [datetimeoffset]::Parse([string]$entryTimestampRaw)
                    if (-not $timestamp -or $entryTimestamp -lt $timestamp) {
                        $timestamp = $entryTimestamp
                    }
                    if (-not $lastUpdated -or $entryTimestamp -gt $lastUpdated) {
                        $lastUpdated = $entryTimestamp
                    }
                } catch {
                }
            }

            $candidate = Get-CshClaudePreviewCandidate -Entry $entry
            if (-not [string]::IsNullOrWhiteSpace($candidate)) {
                if ([string]::IsNullOrWhiteSpace($fallbackPreview)) {
                    $fallbackPreview = $candidate
                }

                if ((-not $preview) -and (Test-CshMeaningfulUserText -Text $candidate)) {
                    $preview = $candidate
                }
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

    if (-not $sessionId) {
        $sessionId = [System.IO.Path]::GetFileNameWithoutExtension($File.Name)
    }

    $historyEntry = $null
    if ($HistoryIndex -and $HistoryIndex.ContainsKey($sessionId)) {
        $historyEntry = $HistoryIndex[$sessionId]
    }

    if (-not $preview -and $historyEntry -and (Test-CshMeaningfulUserText -Text ([string]$historyEntry.display))) {
        $preview = [string]$historyEntry.display
    }

    if (-not $preview) {
        $preview = $fallbackPreview
    }

    $historyProject = Get-CshObjectPropertyValue -InputObject $historyEntry -Name 'project'
    if (-not $projectPath -and $historyProject) {
        $projectPath = [string]$historyProject
    }

    $historyTimestamp = Get-CshObjectPropertyValue -InputObject $historyEntry -Name 'timestamp'
    if (-not $timestamp -and $historyTimestamp) {
        try {
            $timestamp = [datetimeoffset]::FromUnixTimeMilliseconds([int64]$historyTimestamp)
        } catch {
            $timestamp = $null
        }
    }

    if (-not $timestamp) {
        $timestamp = [datetimeoffset]$File.LastWriteTimeUtc
    }

    if (-not $lastUpdated) {
        $lastUpdated = [datetimeoffset]$File.LastWriteTimeUtc
    }

    return New-CshSessionRecord `
        -Provider 'claude' `
        -SessionId $sessionId `
        -Timestamp $timestamp `
        -LastUpdated $lastUpdated `
        -ProjectPath $projectPath `
        -Preview $preview `
        -Alias (Get-CshAlias -Index $Index -SessionId $sessionId -Provider 'claude') `
        -FilePath $File.FullName `
        -RecordedBranchName $recordedBranchName `
        -RecordedDetachedHead $recordedDetachedHead `
        -Slug $slug `
        -GitContextCache $GitContextCache
}

function Read-CshSessionFile {
    param(
        [Parameter(Mandatory = $true)][System.IO.FileInfo]$File,
        [Parameter(Mandatory = $true)][hashtable]$Index,
        [hashtable]$GitContextCache,
        [string]$Provider = 'codex',
        [hashtable]$HistoryIndex
    )

    switch (Resolve-CshProviderName -Provider $Provider) {
        'codex' {
            return Read-CshCodexSessionFile -File $File -Index $Index -GitContextCache $GitContextCache
        }
        'claude' {
            return Read-CshClaudeSessionFile -File $File -Index $Index -GitContextCache $GitContextCache -HistoryIndex $HistoryIndex
        }
        default {
            throw "Unsupported provider: $Provider"
        }
    }
}

function Get-CshSessions {
    param(
        [hashtable]$Index = $(Get-CshIndex),
        [string]$Provider = 'codex'
    )

    $providerName = Resolve-CshProviderName -Provider $Provider
    $sessionRoot = Get-CshSessionRoot -Provider $providerName
    if (-not (Test-Path -LiteralPath $sessionRoot)) {
        return @()
    }

    $files = Get-ChildItem -Path $sessionRoot -Recurse -File -Filter '*.jsonl' | Sort-Object LastWriteTime -Descending
    $gitContextCache = @{}
    $historyIndex = if ($providerName -eq 'claude') { Get-CshClaudeHistoryIndex } else { @{} }
    $sessions = foreach ($file in $files) {
        $session = Read-CshSessionFile -File $file -Index $Index -GitContextCache $gitContextCache -Provider $providerName -HistoryIndex $historyIndex
        if ($session) {
            $session
        }
    }

    return @($sessions | Sort-Object @{ Expression = 'Timestamp'; Descending = $true }, @{ Expression = 'ProjectPath'; Descending = $false })
}

function Get-CshDisplaySessions {
    param([Parameter(Mandatory = $true)][object[]]$Sessions)

    $groups = $Sessions | Group-Object { '{0}|{1}' -f $_.Provider, (Get-CshDisplayGroupKey -Session $_) }
    $orderedProjects = foreach ($group in $groups) {
        $items = @($group.Group | Sort-Object @{ Expression = 'Timestamp'; Descending = $true })
        [pscustomobject]@{
            Provider       = $items[0].Provider
            GroupKey       = Get-CshDisplayGroupKey -Session $items[0]
            WorkspaceLabel = if (-not [string]::IsNullOrWhiteSpace([string]$items[0].WorkspaceLabel)) { $items[0].WorkspaceLabel } else { $items[0].ProjectName }
            ProjectPath    = $items[0].ProjectPath
            LatestTime     = $items[0].Timestamp
            Items          = $items
        }
    }

    $display = New-Object 'System.Collections.Generic.List[object]'
    foreach ($project in ($orderedProjects | Sort-Object @{ Expression = 'LatestTime'; Descending = $true }, @{ Expression = 'ProjectPath'; Descending = $false })) {
        foreach ($session in $project.Items) {
            $displayNumber = $display.Count + 1
            [void]$display.Add([pscustomobject]@{
                Provider        = $session.Provider
                ProviderLabel   = $session.ProviderLabel
                SupportsDelete  = $session.SupportsDelete
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
                GroupKey        = $project.GroupKey
                RepoRoot        = $session.RepoRoot
                RepoName        = $session.RepoName
                BranchName      = $session.BranchName
                BranchDisplay   = $session.BranchDisplay
                IsDetachedHead  = $session.IsDetachedHead
                WorkspaceKey    = $session.WorkspaceKey
                WorkspaceLabel  = if (-not [string]::IsNullOrWhiteSpace([string]$session.WorkspaceLabel)) { $session.WorkspaceLabel } else { $project.WorkspaceLabel }
                FilePath        = $session.FilePath
                ProjectExists   = $session.ProjectExists
                Alias           = $session.Alias
                Preview         = $session.Preview
                DisplayTitle    = $session.DisplayTitle
                Slug            = $session.Slug
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
    $searchRepos = $false
    $searchBranches = $false
    $textQuery = $trimmedQuery
    if ($trimmedQuery -match '^(t:|title:)\s*(.+)$') {
        $searchTitles = $true
        $textQuery = $Matches[2].Trim()
    } elseif ($trimmedQuery -match '^(r:|repo:)\s*(.+)$') {
        $searchRepos = $true
        $textQuery = $Matches[2].Trim()
    } elseif ($trimmedQuery -match '^(b:|branch:)\s*(.+)$') {
        $searchBranches = $true
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

    if ($searchRepos) {
        return @($displaySessions | Where-Object {
            ([string]$_.RepoName).ToLowerInvariant().Contains($lowerQuery)
        })
    }

    if ($searchBranches) {
        return @($displaySessions | Where-Object {
            ([string]$_.BranchDisplay).ToLowerInvariant().Contains($lowerQuery)
        })
    }

    return @($displaySessions | Where-Object {
        $_.ProjectName.ToLowerInvariant().Contains($lowerQuery) -or
        ([string]$_.RepoName).ToLowerInvariant().Contains($lowerQuery)
    })
}

function Find-CshSession {
    param(
        [Parameter(Mandatory = $true)][object[]]$Sessions,
        [Parameter(Mandatory = $true)][string]$SessionId,
        [string]$Provider = ''
    )

    $providerName = if ([string]::IsNullOrWhiteSpace($Provider)) { '' } else { Resolve-CshProviderName -Provider $Provider }
    $exact = @($Sessions | Where-Object {
        $_.SessionId -eq $SessionId -and ([string]::IsNullOrWhiteSpace($providerName) -or $_.Provider -eq $providerName)
    })
    if ($exact.Count -eq 1) {
        return $exact[0]
    }

    $prefix = @($Sessions | Where-Object {
        $_.SessionId -like "$SessionId*" -and ([string]::IsNullOrWhiteSpace($providerName) -or $_.Provider -eq $providerName)
    })
    if ($prefix.Count -eq 1) {
        return $prefix[0]
    }

    return $null
}
