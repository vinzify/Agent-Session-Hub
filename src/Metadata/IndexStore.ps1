function New-CshIndex {
    return @{
        providers = @{}
    }
}

function Get-CshIndexProviderBucket {
    param(
        [Parameter(Mandatory = $true)][hashtable]$Index,
        [string]$Provider = 'codex'
    )

    $providerName = Resolve-CshProviderName -Provider $Provider
    if (-not $Index.ContainsKey('providers') -or -not $Index.providers) {
        $Index.providers = @{}
    }

    if (-not $Index.providers.ContainsKey($providerName) -or -not $Index.providers[$providerName]) {
        $Index.providers[$providerName] = @{
            sessions = @{}
        }
    }

    if (-not $Index.providers[$providerName].ContainsKey('sessions') -or -not $Index.providers[$providerName].sessions) {
        $Index.providers[$providerName].sessions = @{}
    }

    return $Index.providers[$providerName]
}

function Get-CshIndex {
    param([string]$Provider = 'codex')

    $indexPath = Get-CshIndexPath -Provider $Provider
    $resolvedPath = $indexPath
    if (-not (Test-Path $resolvedPath)) {
        $legacyIndexPath = Get-CshLegacyIndexPath -Provider $Provider
        if (Test-Path $legacyIndexPath) {
            $resolvedPath = $legacyIndexPath
        } else {
            return (New-CshIndex)
        }
    }

    if (-not (Test-Path $resolvedPath)) {
        return (New-CshIndex)
    }

    try {
        $raw = Get-Content -Path $resolvedPath -Raw | ConvertFrom-Json -AsHashtable
    } catch {
        return (New-CshIndex)
    }

    if (-not $raw) {
        return (New-CshIndex)
    }

    if ($raw.ContainsKey('sessions') -and $raw.sessions -and (-not $raw.ContainsKey('providers'))) {
        $raw.providers = @{
            codex = @{
                sessions = $raw.sessions
            }
        }
        $raw.Remove('sessions')
    }

    [void](Get-CshIndexProviderBucket -Index $raw -Provider $Provider)
    if ($resolvedPath -ne $indexPath) {
        Save-CshIndex -Index $raw -Provider $Provider
    }
    return $raw
}

function Save-CshIndex {
    param(
        [Parameter(Mandatory = $true)][hashtable]$Index,
        [string]$Provider = 'codex'
    )

    if ($Index.ContainsKey('sessions')) {
        $Index.Remove('sessions')
    }

    [void](Get-CshIndexProviderBucket -Index $Index -Provider $Provider)

    $indexPath = Get-CshIndexPath -Provider $Provider
    Ensure-CshDirectory -Path (Split-Path -Parent $indexPath)
    ($Index | ConvertTo-Json -Depth 8) | Set-Content -Path $indexPath
}

function Get-CshAlias {
    param(
        [Parameter(Mandatory = $true)][hashtable]$Index,
        [Parameter(Mandatory = $true)][string]$SessionId,
        [string]$Provider = 'codex'
    )

    $bucket = Get-CshIndexProviderBucket -Index $Index -Provider $Provider
    if (-not $bucket.sessions.ContainsKey($SessionId)) {
        return ''
    }

    $entry = $bucket.sessions[$SessionId]
    if ($entry -is [hashtable]) {
        return [string]($entry.alias ?? '')
    }

    return ''
}

function Set-CshAlias {
    param(
        [Parameter(Mandatory = $true)][hashtable]$Index,
        [Parameter(Mandatory = $true)][string]$SessionId,
        [string]$Provider = 'codex',
        [string]$Alias
    )

    $bucket = Get-CshIndexProviderBucket -Index $Index -Provider $Provider
    if ([string]::IsNullOrWhiteSpace($Alias)) {
        if ($bucket.sessions.ContainsKey($SessionId)) {
            $bucket.sessions.Remove($SessionId)
        }
        return
    }

    $bucket.sessions[$SessionId] = @{
        alias = $Alias.Trim()
    }
}

function Remove-CshAlias {
    param(
        [Parameter(Mandatory = $true)][hashtable]$Index,
        [Parameter(Mandatory = $true)][string]$SessionId,
        [string]$Provider = 'codex'
    )

    $bucket = Get-CshIndexProviderBucket -Index $Index -Provider $Provider
    if ($bucket.sessions.ContainsKey($SessionId)) {
        $bucket.sessions.Remove($SessionId)
    }
}
