Describe 'Claude provider' {
    BeforeAll {
        . (Join-Path $PSScriptRoot 'TestBootstrap.ps1')
    }

    It 'uses the Claude projects directory as the default session root' {
        (Get-CshDefaultSessionRoot -Provider 'claude') | Should -Be (Join-Path $HOME '.claude\projects')
    }

    It 'parses Claude project transcripts into normalized sessions' {
        $tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('csh-claude-' + [guid]::NewGuid().ToString('N'))
        $projectDir = Join-Path $tempRoot 'projects'
        $encodedProjectDir = Join-Path $projectDir 'C--Users-twinr-code-demo'
        $sessionFile = Join-Path $encodedProjectDir '12345678-1234-1234-1234-1234567890ab.jsonl'
        $historyPath = Join-Path $tempRoot 'history.jsonl'

        try {
            New-Item -ItemType Directory -Path $encodedProjectDir -Force | Out-Null
            $entries = @(
                '{"type":"user","message":{"role":"user","content":"resume"},"timestamp":"2026-03-22T10:00:00Z","cwd":"D:\\code\\demo","sessionId":"12345678-1234-1234-1234-1234567890ab","gitBranch":"feature/claude"}'
                '{"type":"assistant","timestamp":"2026-03-22T10:00:01Z","cwd":"D:\\code\\demo","sessionId":"12345678-1234-1234-1234-1234567890ab","gitBranch":"feature/claude","slug":"calm-otter"}'
                '{"type":"user","message":{"role":"user","content":"Investigate the release checklist for this repo"},"timestamp":"2026-03-22T10:00:02Z","cwd":"D:\\code\\demo","sessionId":"12345678-1234-1234-1234-1234567890ab","gitBranch":"feature/claude"}'
            )
            Set-Content -Path $sessionFile -Value $entries -Encoding utf8

            $history = @(
                '{"display":"Investigate the release checklist for this repo","timestamp":1774173602000,"project":"D:\\code\\demo","sessionId":"12345678-1234-1234-1234-1234567890ab"}'
            )
            Set-Content -Path $historyPath -Value $history -Encoding utf8

            $historyIndex = @{}
            $historyIndex['12345678-1234-1234-1234-1234567890ab'] = [pscustomobject]@{
                display   = 'Investigate the release checklist for this repo'
                timestamp = 1774173602000
                project   = 'D:\code\demo'
                sessionId = '12345678-1234-1234-1234-1234567890ab'
            }

            $session = Read-CshClaudeSessionFile -File (Get-Item $sessionFile) -Index (New-CshIndex) -GitContextCache @{} -HistoryIndex $historyIndex
            $session.Provider | Should -Be 'claude'
            $session.ProviderLabel | Should -Be 'Claude'
            $session.SupportsDelete | Should -BeFalse
            $session.SessionId | Should -Be '12345678-1234-1234-1234-1234567890ab'
            $session.ProjectPath | Should -Be 'D:\code\demo'
            $session.BranchDisplay | Should -Be 'feature/claude'
            $session.DisplayTitle | Should -Match 'Investigate the release checklist'
        } finally {
            if (Test-Path $tempRoot) {
                Remove-Item -Path $tempRoot -Recurse -Force
            }
        }
    }

    It 'disables delete support for Claude usage help' {
        $usage = @(Show-CshUsage -Provider 'claude')

        ($usage -join "`n") | Should -Not -Match 'delete'
        ($usage -join "`n") | Should -Match '^clx \[query\]'
    }
}
