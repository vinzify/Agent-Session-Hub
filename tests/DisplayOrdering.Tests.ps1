$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent $here
Import-Module (Join-Path $projectRoot 'src/CodexSessionHub.psd1') -Force
$module = Get-Module CodexSessionHub

Describe 'Display ordering' {
    It 'returns sessions in grouped display order' {
        $sessions = @(
            [pscustomobject]@{ SessionId='2'; Timestamp=[datetimeoffset]'2026-03-01'; TimestampText='2026-03-01 00:00'; LastUpdated=[datetimeoffset]'2026-03-01'; LastUpdatedText='2026-03-01 00:00'; LastUpdatedAge='1d ago'; ProjectKey='b'; ProjectName='B'; ProjectPath='B'; FilePath=''; ProjectExists=$true; Alias=''; Preview=''; DisplayTitle='B1' },
            [pscustomobject]@{ SessionId='1'; Timestamp=[datetimeoffset]'2026-03-02'; TimestampText='2026-03-02 00:00'; LastUpdated=[datetimeoffset]'2026-03-02'; LastUpdatedText='2026-03-02 00:00'; LastUpdatedAge='1h ago'; ProjectKey='a'; ProjectName='A'; ProjectPath='A'; FilePath=''; ProjectExists=$true; Alias=''; Preview=''; DisplayTitle='A1' }
        )

        $bound = $module.NewBoundScriptBlock({
            param($inputSessions)
            Get-CshDisplaySessions -Sessions $inputSessions
        })
        $ordered = @(& $bound -inputSessions $sessions)
        $ordered[0].SessionId | Should Be '1'
        $ordered[1].SessionId | Should Be '2'
        $ordered[0].DisplayNumber | Should Be 1
        $ordered[1].DisplayNumber | Should Be 2
    }

    It 'encodes row identity keys for sessions and projects' {
        $session = [pscustomobject]@{
            SessionId='abc'; DisplayNumber=7; TimestampText='2026-03-02 00:00'; ProjectName='Desktop'; DisplayTitle='Title'; ProjectPath='C:\Users\twinr\Desktop'; Preview='Preview'
        }
        $bound = $module.NewBoundScriptBlock({
            param($boundSession)
            ConvertTo-CshFzfRow -Session $boundSession
        })
        $sessionRow = & $bound $session

        $sessionRow | Should Match '^S:abc\t'
        $bound = $module.NewBoundScriptBlock({ New-CshProjectRowKey -ProjectPath 'C:\Users\twinr\Desktop' })
        (& $bound) | Should Match '^P:'
    }

    It 'builds an fzf query command with a quoted query placeholder' {
        $bound = $module.NewBoundScriptBlock({ Get-CshQueryCommand })
        $command = & $bound

        if ($IsWindows) {
            $command | Should Match 'csx-query\.cmd"$'
        } else {
            $command | Should Match '__query$'
        }
    }

    It 'builds an fzf preview command with session and project placeholders' {
        $bound = $module.NewBoundScriptBlock({ Get-CshPreviewCommand })
        $command = & $bound

        if ($IsWindows) {
            $command | Should Match 'csx-preview\.cmd" \{\}$'
        } else {
            $command | Should Match '__preview \{\}$'
        }
    }
}
