param([Parameter(Mandatory=$true)][string]$PlanPath,
      [Parameter(Mandatory=$true)][int]$AshParentId)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
function Get-AshHash([string]$FilePath) {
    $stream = [System.IO.File]::OpenRead($FilePath)
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try { return [System.BitConverter]::ToString($algorithm.ComputeHash($stream)).Replace('-', '').ToLowerInvariant() }
    finally { $stream.Dispose(); $algorithm.Dispose() }
}
$plan = Get-Content -LiteralPath $PlanPath -Raw -Encoding UTF8 | ConvertFrom-Json
$resolvedStage = [System.IO.Path]::GetFullPath([System.IO.Path]::GetDirectoryName($PlanPath))
if ($resolvedStage -ne [System.IO.Path]::GetFullPath($plan.stage) -or
    [System.IO.Path]::GetFileName($resolvedStage) -notlike 'agent-session-hub-update-*') {
    throw 'Invalid update staging directory.'
}
$staged = @()
$committed = @()
$succeeded = $false
try {
    Wait-Process -Id $AshParentId -ErrorAction SilentlyContinue
    foreach ($item in $plan.files) {
        if ((Get-AshHash $item.destination) -ne $item.sha256) {
            throw "Launcher changed during update: $($item.destination). Rerun the installer."
        }
        $entry = [pscustomobject]@{
            destination = $item.destination
            temp = $item.destination + '.update-' + [guid]::NewGuid().ToString('N')
            backup = $item.destination + '.backup-' + [guid]::NewGuid().ToString('N')
        }
        [System.IO.File]::Copy($item.source, $entry.temp)
        $staged += $entry
    }
    foreach ($entry in $staged) {
        [System.IO.File]::Replace($entry.temp, $entry.destination, $entry.backup)
        $committed += $entry
    }
    $succeeded = $true
    Write-Host 'Agent Session Hub updated. All launchers are ready; sessions/settings were untouched.'
} catch {
    $originalError = $_
    [array]::Reverse($committed)
    foreach ($entry in $committed) {
        try { [System.IO.File]::Replace($entry.backup, $entry.destination, $null) }
        catch { Write-Warning "Recovery backup retained at $($entry.backup): $_" }
    }
    Write-Error "Update failed; original launchers restored where possible. Close other hub processes and retry. $originalError" -ErrorAction Continue
    exit 1
} finally {
    foreach ($entry in $staged) {
        if ([System.IO.File]::Exists($entry.temp)) { [System.IO.File]::Delete($entry.temp) }
        # Failed rollback backups must remain available for manual recovery.
        if ($succeeded -and [System.IO.File]::Exists($entry.backup)) { [System.IO.File]::Delete($entry.backup) }
    }
    [System.IO.Directory]::Delete($resolvedStage, $true)
}
