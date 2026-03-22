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
        (Get-CshIndexProviderBucket -Index $index -Provider 'codex').sessions.ContainsKey('abc') | Should -Be $false
    }

    It 'scopes aliases per provider' {
        $index = New-CshIndex
        Set-CshAlias -Index $index -SessionId 'shared-id' -Alias 'codex alias' -Provider 'codex'
        Set-CshAlias -Index $index -SessionId 'shared-id' -Alias 'claude alias' -Provider 'claude'

        (Get-CshAlias -Index $index -SessionId 'shared-id' -Provider 'codex') | Should -Be 'codex alias'
        (Get-CshAlias -Index $index -SessionId 'shared-id' -Provider 'claude') | Should -Be 'claude alias'
    }
}
