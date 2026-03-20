function New-CshIndex {
    return @{
        sessions = @{}
    }
}

function Get-CshIndex {
    $indexPath = Get-CshIndexPath
    if (-not (Test-Path $indexPath)) {
        return (New-CshIndex)
    }

    try {
        $raw = Get-Content -Path $indexPath -Raw | ConvertFrom-Json -AsHashtable
    } catch {
        return (New-CshIndex)
    }

    if (-not $raw) {
        return (New-CshIndex)
    }

    if (-not $raw.ContainsKey('sessions') -or -not $raw.sessions) {
        $raw.sessions = @{}
    }

    return $raw
}

function Save-CshIndex {
    param([Parameter(Mandatory = $true)][hashtable]$Index)

    $indexPath = Get-CshIndexPath
    Ensure-CshDirectory -Path (Split-Path -Parent $indexPath)
    ($Index | ConvertTo-Json -Depth 8) | Set-Content -Path $indexPath
}

function Get-CshAlias {
    param(
        [Parameter(Mandatory = $true)][hashtable]$Index,
        [Parameter(Mandatory = $true)][string]$SessionId
    )

    if (-not $Index.sessions.ContainsKey($SessionId)) {
        return ''
    }

    $entry = $Index.sessions[$SessionId]
    if ($entry -is [hashtable]) {
        return [string]($entry.alias ?? '')
    }

    return ''
}

function Set-CshAlias {
    param(
        [Parameter(Mandatory = $true)][hashtable]$Index,
        [Parameter(Mandatory = $true)][string]$SessionId,
        [string]$Alias
    )

    if ([string]::IsNullOrWhiteSpace($Alias)) {
        if ($Index.sessions.ContainsKey($SessionId)) {
            $Index.sessions.Remove($SessionId)
        }
        return
    }

    $Index.sessions[$SessionId] = @{
        alias = $Alias.Trim()
    }
}

function Remove-CshAlias {
    param(
        [Parameter(Mandatory = $true)][hashtable]$Index,
        [Parameter(Mandatory = $true)][string]$SessionId
    )

    if ($Index.sessions.ContainsKey($SessionId)) {
        $Index.sessions.Remove($SessionId)
    }
}
