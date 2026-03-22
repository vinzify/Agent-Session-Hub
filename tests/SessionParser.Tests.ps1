Describe 'Normalize-CshPath' {
    BeforeAll {
        . (Join-Path $PSScriptRoot 'TestBootstrap.ps1')
    }

    It 'removes the Windows long path prefix' {
        (Normalize-CshPath '\\?\D:\code\example') | Should -Be 'D:\code\example'
    }

    It 'normalizes Windows drive paths to backslashes' {
        (Normalize-CshPath 'D:/code/example') | Should -Be 'D:\code\example'
    }
}

Describe 'Get-CshFilteredDisplaySessions' {
    BeforeAll {
        . (Join-Path $PSScriptRoot 'TestBootstrap.ps1')
    }

    It 'treats quote-only queries as empty' {
        $sessions = @(
            [pscustomobject]@{
                SessionId='1'; Timestamp=[datetimeoffset]'2026-03-02'; TimestampText='2026-03-02 00:00'
                LastUpdated=[datetimeoffset]'2026-03-02'; LastUpdatedText='2026-03-02 00:00'; LastUpdatedAge='1h ago'
                ProjectKey='desktop'; ProjectName='Desktop'; ProjectPath='C:\Users\twinr\Desktop'; FilePath=''
                ProjectExists=$true; Alias=''; Preview='Preview'; DisplayTitle='Title'
            }
        )

        $filtered = @(Get-CshFilteredDisplaySessions -Sessions $sessions -Query '""""')
        $filtered.Count | Should -Be 1
        $filtered[0].SessionId | Should -Be '1'
    }

    It 'filters sessions by branch query' {
        $sessions = @(
            [pscustomobject]@{
                SessionId='1'; Timestamp=[datetimeoffset]'2026-03-02'; TimestampText='2026-03-02 00:00'
                LastUpdated=[datetimeoffset]'2026-03-02'; LastUpdatedText='2026-03-02 00:00'; LastUpdatedAge='1h ago'
                ProjectKey='desktop'; ProjectName='Desktop'; ProjectPath='C:\Users\twinr\Desktop'; FilePath=''
                ProjectExists=$true; Alias=''; Preview='Preview'; DisplayTitle='Title'; RepoName='platform'; BranchDisplay='feature/session-hub'
            },
            [pscustomobject]@{
                SessionId='2'; Timestamp=[datetimeoffset]'2026-03-01'; TimestampText='2026-03-01 00:00'
                LastUpdated=[datetimeoffset]'2026-03-01'; LastUpdatedText='2026-03-01 00:00'; LastUpdatedAge='1d ago'
                ProjectKey='api'; ProjectName='Api'; ProjectPath='D:\code\api'; FilePath=''
                ProjectExists=$true; Alias=''; Preview='Preview'; DisplayTitle='Title'; RepoName='payments'; BranchDisplay='main'
            }
        )

        $filtered = @(Get-CshFilteredDisplaySessions -Sessions $sessions -Query 'branch:feature')
        $filtered.Count | Should -Be 1
        $filtered[0].SessionId | Should -Be '1'
    }

    It 'filters sessions by repo query' {
        $sessions = @(
            [pscustomobject]@{
                SessionId='1'; Timestamp=[datetimeoffset]'2026-03-02'; TimestampText='2026-03-02 00:00'
                LastUpdated=[datetimeoffset]'2026-03-02'; LastUpdatedText='2026-03-02 00:00'; LastUpdatedAge='1h ago'
                ProjectKey='desktop'; ProjectName='src'; ProjectPath='C:\code\platform\src'; FilePath=''
                ProjectExists=$true; Alias=''; Preview='Preview'; DisplayTitle='Title'; RepoName='platform'
            },
            [pscustomobject]@{
                SessionId='2'; Timestamp=[datetimeoffset]'2026-03-01'; TimestampText='2026-03-01 00:00'
                LastUpdated=[datetimeoffset]'2026-03-01'; LastUpdatedText='2026-03-01 00:00'; LastUpdatedAge='1d ago'
                ProjectKey='api'; ProjectName='worker'; ProjectPath='D:\code\payments\worker'; FilePath=''
                ProjectExists=$true; Alias=''; Preview='Preview'; DisplayTitle='Title'; RepoName='payments'
            }
        )

        $filtered = @(Get-CshFilteredDisplaySessions -Sessions $sessions -Query 'repo:platform')
        $filtered.Count | Should -Be 1
        $filtered[0].SessionId | Should -Be '1'
    }

    It 'matches repo names in plain text search' {
        $sessions = @(
            [pscustomobject]@{
                SessionId='1'; Timestamp=[datetimeoffset]'2026-03-02'; TimestampText='2026-03-02 00:00'
                LastUpdated=[datetimeoffset]'2026-03-02'; LastUpdatedText='2026-03-02 00:00'; LastUpdatedAge='1h ago'
                ProjectKey='desktop'; ProjectName='src'; ProjectPath='C:\code\platform\src'; FilePath=''
                ProjectExists=$true; Alias=''; Preview='Preview'; DisplayTitle='Title'; RepoName='platform'
            },
            [pscustomobject]@{
                SessionId='2'; Timestamp=[datetimeoffset]'2026-03-01'; TimestampText='2026-03-01 00:00'
                LastUpdated=[datetimeoffset]'2026-03-01'; LastUpdatedText='2026-03-01 00:00'; LastUpdatedAge='1d ago'
                ProjectKey='api'; ProjectName='worker'; ProjectPath='D:\code\payments\worker'; FilePath=''
                ProjectExists=$true; Alias=''; Preview='Preview'; DisplayTitle='Title'; RepoName='payments'
            }
        )

        $filtered = @(Get-CshFilteredDisplaySessions -Sessions $sessions -Query 'payments')
        $filtered.Count | Should -Be 1
        $filtered[0].SessionId | Should -Be '2'
    }
}

Describe 'Format-CshAsciiBanner' {
    BeforeAll {
        . (Join-Path $PSScriptRoot 'TestBootstrap.ps1')
    }

    It 'renders a compact three-line banner' {
        $banner = Format-CshAsciiBanner -Kind 'project' -Primary 'Desktop' -Secondary '10 sessions'
        $lines = @($banner -split [Environment]::NewLine)

        $lines.Count | Should -Be 3
        $lines[1] | Should -Match 'PROJECT'
        $lines[1] | Should -Match 'Desktop'
    }
}

Describe 'Read-CshSessionFile git metadata' {
    BeforeAll {
        . (Join-Path $PSScriptRoot 'TestBootstrap.ps1')
    }

    It 'leaves git metadata empty outside a repository' {
        $tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('csh-nongit-' + [guid]::NewGuid().ToString('N'))
        $sessionDir = Join-Path $tempRoot 'session'
        $sessionFile = Join-Path $tempRoot 'session.jsonl'

        try {
            New-Item -ItemType Directory -Path $sessionDir -Force | Out-Null
            $meta = @{
                timestamp = '2026-03-02T00:00:00Z'
                type = 'session_meta'
                payload = @{
                    id = 'abc'
                    timestamp = '2026-03-02T00:00:00Z'
                    cwd = $sessionDir
                }
            } | ConvertTo-Json -Compress -Depth 5
            Set-Content -Path $sessionFile -Value $meta -Encoding utf8

            $session = Read-CshSessionFile -File (Get-Item $sessionFile) -Index (New-CshIndex) -GitContextCache @{}
            $session.RepoRoot | Should -Be ''
            $session.BranchDisplay | Should -Be ''
            $session.WorkspaceKey | Should -Be ($session.ProjectPath.ToLowerInvariant())
        } finally {
            if (Test-Path $tempRoot) {
                Remove-Item -Path $tempRoot -Recurse -Force
            }
        }
    }

    It 'reads repository root and branch from the session cwd' -Skip:(-not [bool](Get-Command git -ErrorAction SilentlyContinue)) {
        $tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('csh-git-' + [guid]::NewGuid().ToString('N'))
        $repoRoot = Join-Path $tempRoot 'repo'
        $workDir = Join-Path $repoRoot 'src'
        $sessionFile = Join-Path $tempRoot 'session.jsonl'
        $git = (Get-Command git -ErrorAction Stop).Source

        try {
            New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null
            & $git init --initial-branch=main $repoRoot 2>$null | Out-Null
            New-Item -ItemType Directory -Path $workDir -Force | Out-Null
            & $git -C $repoRoot checkout -b feature/session-hub 2>$null | Out-Null

            $meta = @{
                timestamp = '2026-03-02T00:00:00Z'
                type = 'session_meta'
                payload = @{
                    id = 'git-session'
                    timestamp = '2026-03-02T00:00:00Z'
                    cwd = $workDir
                }
            } | ConvertTo-Json -Compress -Depth 5
            Set-Content -Path $sessionFile -Value $meta -Encoding utf8

            $session = Read-CshSessionFile -File (Get-Item $sessionFile) -Index (New-CshIndex) -GitContextCache @{}
            $expectedRepoRoot = Normalize-CshPath $repoRoot
            $actualRepoRoot = [string]$session.RepoRoot
            if (-not $IsWindows) {
                $expectedRepoRoot = $expectedRepoRoot -replace '^/private', ''
                $actualRepoRoot = $actualRepoRoot -replace '^/private', ''
            }

            $actualRepoRoot | Should -Be $expectedRepoRoot
            $session.ProjectPath | Should -Be (Normalize-CshPath $workDir)
            $session.BranchDisplay | Should -Be 'feature/session-hub'
            $expectedWorkspaceKey = Get-CshWorkspaceKey -RepoRoot (Normalize-CshPath $repoRoot) -BranchName 'feature/session-hub' -ProjectPath (Normalize-CshPath $workDir)
            $actualWorkspaceKey = [string]$session.WorkspaceKey
            if (-not $IsWindows) {
                $expectedWorkspaceKey = $expectedWorkspaceKey -replace '^/private', ''
                $actualWorkspaceKey = $actualWorkspaceKey -replace '^/private', ''
            }

            $actualWorkspaceKey | Should -Be $expectedWorkspaceKey
        } finally {
            if (Test-Path $tempRoot) {
                Remove-Item -Path $tempRoot -Recurse -Force
            }
        }
    }
}
