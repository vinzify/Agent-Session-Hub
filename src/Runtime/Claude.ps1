function Invoke-CshClaudeResume {
    param(
        [Parameter(Mandatory = $true)][object]$Session,
        [switch]$ShellMode
    )

    if ($ShellMode -and $Session.ProjectExists) {
        Set-Location $Session.ProjectPath
    } elseif ($ShellMode -and -not $Session.ProjectExists -and -not [string]::IsNullOrWhiteSpace([string]$Session.ProjectPath)) {
        Write-Warning "Project path no longer exists: $($Session.ProjectPath)"
    }

    & claude --resume $Session.SessionId
}
