Describe 'Display ordering' {
    BeforeAll {
        . (Join-Path $PSScriptRoot 'TestBootstrap.ps1')
    }

    It 'returns sessions in grouped display order' {
        $sessions = @(
            [pscustomobject]@{ SessionId='2'; Timestamp=[datetimeoffset]'2026-03-01'; TimestampText='2026-03-01 00:00'; LastUpdated=[datetimeoffset]'2026-03-01'; LastUpdatedText='2026-03-01 00:00'; LastUpdatedAge='1d ago'; ProjectKey='b'; ProjectName='B'; ProjectPath='B'; FilePath=''; ProjectExists=$true; Alias=''; Preview=''; DisplayTitle='B1' },
            [pscustomobject]@{ SessionId='1'; Timestamp=[datetimeoffset]'2026-03-02'; TimestampText='2026-03-02 00:00'; LastUpdated=[datetimeoffset]'2026-03-02'; LastUpdatedText='2026-03-02 00:00'; LastUpdatedAge='1h ago'; ProjectKey='a'; ProjectName='A'; ProjectPath='A'; FilePath=''; ProjectExists=$true; Alias=''; Preview=''; DisplayTitle='A1' }
        )

        $ordered = @(Get-CshDisplaySessions -Sessions $sessions)
        $ordered[0].SessionId | Should -Be '1'
        $ordered[1].SessionId | Should -Be '2'
        $ordered[0].DisplayNumber | Should -Be 1
        $ordered[1].DisplayNumber | Should -Be 2
    }

    It 'encodes row identity keys for sessions and projects' {
        $session = [pscustomobject]@{
            SessionId='abc'; DisplayNumber=7; TimestampText='2026-03-02 00:00'; ProjectName='Desktop'; DisplayTitle='Title'; ProjectPath='C:\Users\twinr\Desktop'; Preview='Preview'
        }
        $sessionRow = ConvertTo-CshFzfRow -Session $session

        $sessionRow | Should -Match '^S:abc\t'
        (New-CshProjectRowKey -ProjectPath 'C:\Users\twinr\Desktop') | Should -Match '^P:'
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
}
