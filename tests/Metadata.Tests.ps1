$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent $here
Import-Module (Join-Path $projectRoot 'src/CodexSessionHub.psd1') -Force
$module = Get-Module CodexSessionHub

Describe 'Index metadata' {
    It 'stores aliases in memory' {
        $index = & $module { New-CshIndex }
        & $module { param($sharedIndex) Set-CshAlias -Index $sharedIndex -SessionId 'abc' -Alias 'hello' } $index
        (& $module { param($sharedIndex) Get-CshAlias -Index $sharedIndex -SessionId 'abc' } $index) | Should Be 'hello'
    }

    It 'clears aliases when set to blank' {
        $index = & $module { New-CshIndex }
        & $module { param($sharedIndex) Set-CshAlias -Index $sharedIndex -SessionId 'abc' -Alias 'hello' } $index
        & $module { param($sharedIndex) Set-CshAlias -Index $sharedIndex -SessionId 'abc' -Alias '' } $index

        (& $module { param($sharedIndex) Get-CshAlias -Index $sharedIndex -SessionId 'abc' } $index) | Should Be ''
    }

    It 'removes alias entries when cleared' {
        $index = & $module { New-CshIndex }
        & $module { param($sharedIndex) Set-CshAlias -Index $sharedIndex -SessionId 'abc' -Alias 'hello' } $index
        & $module { param($sharedIndex) Set-CshAlias -Index $sharedIndex -SessionId 'abc' -Alias '' } $index

        $index.sessions.ContainsKey('abc') | Should Be $false
    }
}
