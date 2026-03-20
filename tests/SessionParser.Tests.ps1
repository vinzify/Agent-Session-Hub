$here = Split-Path -Parent $MyInvocation.MyCommand.Path

Describe 'Normalize-CshPath' {
    BeforeAll {
        . (Join-Path $here 'TestBootstrap.ps1')
    }

    It 'removes the Windows long path prefix' {
        (Normalize-CshPath '\\?\D:\code\example') | Should Be 'D:\code\example'
    }
}

Describe 'Get-CshFilteredDisplaySessions' {
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
        $filtered.Count | Should Be 1
        $filtered[0].SessionId | Should Be '1'
    }
}

Describe 'Format-CshAsciiBanner' {
    It 'renders a compact three-line banner' {
        $banner = Format-CshAsciiBanner -Kind 'project' -Primary 'Desktop' -Secondary '10 sessions'
        $lines = @($banner -split [Environment]::NewLine)

        $lines.Count | Should Be 3
        $lines[1] | Should Match 'PROJECT'
        $lines[1] | Should Match 'Desktop'
    }
}
