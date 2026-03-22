function Get-CshProviderCatalog {
    return @{
        codex = @{
            Name               = 'codex'
            DisplayName        = 'Codex'
            BinaryName         = 'codex'
            LauncherName       = 'csx'
            LegacyAliases      = @('cxs')
            SessionRootEnv     = 'CODEX_SESSION_HUB_SESSION_ROOT'
            DefaultSessionRoot = (Join-Path $HOME '.codex\sessions')
            IndexFileName      = 'index.json'
            SupportsDelete     = $true
        }
        claude = @{
            Name               = 'claude'
            DisplayName        = 'Claude'
            BinaryName         = 'claude'
            LauncherName       = 'clx'
            LegacyAliases      = @()
            SessionRootEnv     = 'CODEX_SESSION_HUB_CLAUDE_SESSION_ROOT'
            DefaultSessionRoot = (Join-Path $HOME '.claude\projects')
            IndexFileName      = 'claude-index.json'
            SupportsDelete     = $false
        }
    }
}

function Resolve-CshProviderName {
    param([string]$Provider = 'codex')

    $normalized = if ([string]::IsNullOrWhiteSpace($Provider)) { 'codex' } else { $Provider.Trim().ToLowerInvariant() }
    $catalog = Get-CshProviderCatalog
    if (-not $catalog.ContainsKey($normalized)) {
        throw "Unsupported provider: $Provider"
    }

    return $normalized
}

function Get-CshProvider {
    param([string]$Provider = 'codex')

    $providerName = Resolve-CshProviderName -Provider $Provider
    return [pscustomobject](Get-CshProviderCatalog)[$providerName]
}

function Get-CshProviderDisplayName {
    param([string]$Provider = 'codex')

    return [string](Get-CshProvider -Provider $Provider).DisplayName
}

function Get-CshProviderBinaryName {
    param([string]$Provider = 'codex')

    return [string](Get-CshProvider -Provider $Provider).BinaryName
}

function Get-CshProviderLauncherName {
    param([string]$Provider = 'codex')

    return [string](Get-CshProvider -Provider $Provider).LauncherName
}

function Test-CshProviderSupportsDelete {
    param([string]$Provider = 'codex')

    return [bool](Get-CshProvider -Provider $Provider).SupportsDelete
}
