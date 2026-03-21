Describe 'shell integration helpers' {
    BeforeAll {
        . (Join-Path $PSScriptRoot 'TestBootstrap.ps1')
    }

    It 'renders a POSIX launcher that invokes pwsh' -Skip:$IsWindows {
        $content = Get-CshPosixLauncherContent -Name 'csx'

        $content | Should -Match '#!/usr/bin/env sh'
        $content | Should -Match 'exec pwsh -NoProfile -File ".*bin[/\\]csx\.ps1" "\$@"'
    }

    It 'renders a POSIX PATH export block' -Skip:$IsWindows {
        $content = Get-CshPosixPathBlock

        $content | Should -Match '# >>> Codex Session Hub >>>'
        $content | Should -Match 'export PATH=".+:\$PATH"'
        $content | Should -Match 'csx\(\)'
        $content | Should -Match 'browse\)'
        $content | Should -Match '__select'
    }

    It 'chooses the zsh profile on non-Windows zsh shells' -Skip:$IsWindows {
        $originalShell = [string]$env:SHELL
        try {
            $env:SHELL = '/bin/zsh'
            (Get-CshShellProfilePath) | Should -Be (Join-Path $HOME '.zprofile')
        }
        finally {
            $env:SHELL = $originalShell
        }
    }
}
