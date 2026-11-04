param([Parameter(Mandatory=$true)][string]$PlanPath,
      [Parameter(Mandatory=$true)][int]$AshParentId)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
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
        if ((Get-FileHash -LiteralPath $item.destination -Algorithm SHA256).Hash.ToLowerInvariant() -ne $item.sha256) {
            throw "Launcher changed during update: $($item.destination). Rerun the installer."
        }
        $entry = [pscustomobject]@{
            destination = $item.destination
            temp = $item.destination + '.update-' + [guid]::NewGuid().ToString('N')
            backup = $item.destination + '.backup-' + [guid]::NewGuid().ToString('N')
        }
        Copy-Item -LiteralPath $item.source -Destination $entry.temp
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
        if (Test-Path -LiteralPath $entry.temp) { Remove-Item -LiteralPath $entry.temp -Force }
        # Failed rollback backups must remain available for manual recovery.
        if ($succeeded -and (Test-Path -LiteralPath $entry.backup)) { Remove-Item -LiteralPath $entry.backup -Force }
    }
    Remove-Item -LiteralPath $resolvedStage -Recurse -Force
}
