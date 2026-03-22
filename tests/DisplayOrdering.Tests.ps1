Describe 'Display ordering' {
    BeforeAll {
        . (Join-Path $PSScriptRoot 'TestBootstrap.ps1')
    }

    It 'returns sessions in grouped display order' {
        $sessions = @(
            [pscustomobject]@{ SessionId='2'; Timestamp=[datetimeoffset]'2026-03-01'; TimestampText='2026-03-01 00:00'; LastUpdated=[datetimeoffset]'2026-03-01'; LastUpdatedText='2026-03-01 00:00'; LastUpdatedAge='1d ago'; ProjectKey='b'; ProjectName='B'; ProjectPath='B'; GroupKey='repo-b|main|b'; WorkspaceKey='repo-b|main|b'; WorkspaceLabel='repo-b @ main'; FilePath=''; ProjectExists=$true; Alias=''; Preview=''; DisplayTitle='B1' },
            [pscustomobject]@{ SessionId='1'; Timestamp=[datetimeoffset]'2026-03-02'; TimestampText='2026-03-02 00:00'; LastUpdated=[datetimeoffset]'2026-03-02'; LastUpdatedText='2026-03-02 00:00'; LastUpdatedAge='1h ago'; ProjectKey='a'; ProjectName='A'; ProjectPath='A'; GroupKey='repo-a|feature|a'; WorkspaceKey='repo-a|feature|a'; WorkspaceLabel='repo-a @ feature'; FilePath=''; ProjectExists=$true; Alias=''; Preview=''; DisplayTitle='A1' }
        )

        $ordered = @(Get-CshDisplaySessions -Sessions $sessions)
        $ordered[0].SessionId | Should -Be '1'
        $ordered[1].SessionId | Should -Be '2'
        $ordered[0].DisplayNumber | Should -Be 1
        $ordered[1].DisplayNumber | Should -Be 2
        $ordered[0].WorkspaceLabel | Should -Be 'repo-a @ feature'
    }

    It 'encodes row identity keys for sessions and workspaces' {
        $session = [pscustomobject]@{
            SessionId='abc'; DisplayNumber=7; TimestampText='2026-03-02 00:00'; ProjectName='Desktop'; WorkspaceLabel='repo @ feature'; DisplayTitle='Title'; ProjectPath='C:\Users\twinr\Desktop'; Preview='Preview'
        }
        $sessionRow = ConvertTo-CshFzfRow -Session $session

        $sessionRow | Should -Match '^S:abc\t'
        (New-CshWorkspaceRowKey -WorkspaceKey 'repo|feature|desktop') | Should -Match '^W:'
    }

    It 'creates separate workspace headers for different worktree keys' {
        $sessions = @(
            [pscustomobject]@{
                SessionId='1'; DisplayNumber=1; Timestamp=[datetimeoffset]'2026-03-02'; TimestampText='2026-03-02 00:00'
                LastUpdated=[datetimeoffset]'2026-03-02'; LastUpdatedText='2026-03-02 00:00'; LastUpdatedAge='1h ago'
                ProjectName='src'; WorkspaceLabel='platform @ feature-a / src'; ProjectPath='D:\code\platform-a\src'; GroupKey='platform|feature-a|d:\code\platform-a\src'; ProjectKey='d:\code\platform-a\src'
                DisplayTitle='One'; Preview='Preview'
            },
            [pscustomobject]@{
                SessionId='2'; DisplayNumber=2; Timestamp=[datetimeoffset]'2026-03-01'; TimestampText='2026-03-01 00:00'
                LastUpdated=[datetimeoffset]'2026-03-01'; LastUpdatedText='2026-03-01 00:00'; LastUpdatedAge='1d ago'
                ProjectName='src'; WorkspaceLabel='platform @ feature-b / src'; ProjectPath='D:\code\platform-b\src'; GroupKey='platform|feature-b|d:\code\platform-b\src'; ProjectKey='d:\code\platform-b\src'
                DisplayTitle='Two'; Preview='Preview'
            }
        )

        $rows = @(ConvertTo-CshFzfRows -Sessions $sessions)
        (@($rows | Where-Object { $_ -match '^W:' })).Count | Should -Be 2
        $rows[0] | Should -Match 'platform @ feature-a / src'
    }

    It 'builds an fzf query command with a quoted query placeholder' {
        $command = Get-CshQueryCommand

        if ($IsWindows) {
            $command | Should -Match 'csx-query\.cmd"$'
        } else {
            $command | Should -Match '__query$'
        }
    }

    It 'builds an fzf preview command with session and project placeholders' {
        $command = Get-CshPreviewCommand

        if ($IsWindows) {
            $command | Should -Match 'csx-preview\.cmd" \{\}$'
        } else {
            $command | Should -Match '__preview \{\}$'
        }
    }

    It 'builds a compact two-line browser header' {
        $header = Get-CshBrowserHeader
        $lines = @($header -split "`n")

        $lines.Count | Should -Be 2
        $lines[0] | Should -Match 'branch:term'
        $lines[1] | Should -Match 'Ctrl-D delete'
        $lines[1] | Should -Match 'confirm'
    }

    It 'resolves a workspace row to every session in that workspace' {
        $workspaceKey = 'platform|feature-a|d:\code\platform-a\src'
        $workspaceRow = New-CshWorkspaceRowKey -WorkspaceKey $workspaceKey
        $sessions = @(
            [pscustomobject]@{ SessionId='1'; GroupKey=$workspaceKey; WorkspaceLabel='platform @ feature-a / src' },
            [pscustomobject]@{ SessionId='2'; GroupKey=$workspaceKey; WorkspaceLabel='platform @ feature-a / src' },
            [pscustomobject]@{ SessionId='3'; GroupKey='platform|feature-b|d:\code\platform-b\src'; WorkspaceLabel='platform @ feature-b / src' }
        )

        $resolved = @(Resolve-CshSelectedSessions -AllSessions $sessions -SessionIds @($workspaceRow))
        $resolved.Count | Should -Be 2
        @($resolved | Select-Object -ExpandProperty SessionId) | Should -Be @('1', '2')
    }

    It 'builds delete confirmation lines for workspace deletes' {
        $sessions = @(
            [pscustomobject]@{ SessionId='1'; DisplayNumber=5; WorkspaceLabel='platform @ feature-a / src'; DisplayTitle='First title' },
            [pscustomobject]@{ SessionId='2'; DisplayNumber=6; WorkspaceLabel='platform @ feature-a / src'; DisplayTitle='Second title' },
            [pscustomobject]@{ SessionId='3'; DisplayNumber=7; WorkspaceLabel='platform @ feature-a / src'; DisplayTitle='Third title' }
        )

        $lines = @(Get-CshDeleteConfirmationLines -Sessions $sessions)
        $lines[0] | Should -Be 'Delete 3 sessions'
        $lines[1] | Should -Be 'Workspace: platform @ feature-a / src'
        $lines[3] | Should -Match '#5 First title'
        $lines[4] | Should -Match '#6 Second title'
        $lines[5] | Should -Be '  - ... and 1 more'
    }
}
