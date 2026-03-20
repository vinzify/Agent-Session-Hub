Describe 'Index metadata' {
    BeforeAll {
        . (Join-Path $PSScriptRoot 'TestBootstrap.ps1')
    }

    It 'stores aliases in memory' {
        $index = New-CshIndex
        Set-CshAlias -Index $index -SessionId 'abc' -Alias 'hello'
        (Get-CshAlias -Index $index -SessionId 'abc') | Should -Be 'hello'
    }

    It 'clears aliases when set to blank' {
        $index = New-CshIndex
        Set-CshAlias -Index $index -SessionId 'abc' -Alias 'hello'
        Set-CshAlias -Index $index -SessionId 'abc' -Alias ''
        (Get-CshAlias -Index $index -SessionId 'abc') | Should -Be ''
    }

    It 'removes alias entries when cleared' {
        $index = New-CshIndex
        Set-CshAlias -Index $index -SessionId 'abc' -Alias 'hello'
        Set-CshAlias -Index $index -SessionId 'abc' -Alias ''
        $index.sessions.ContainsKey('abc') | Should -Be $false
    }
}
