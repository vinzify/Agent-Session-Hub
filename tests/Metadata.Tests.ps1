$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent $here
Import-Module (Join-Path $projectRoot 'src/CodexSessionHub.psd1') -Force
$module = Get-Module CodexSessionHub

Describe 'Index metadata' {
    It 'stores aliases in memory' {
        $bound = $module.NewBoundScriptBlock({ New-CshIndex })
        $index = & $bound
        $bound = $module.NewBoundScriptBlock({
            param($sharedIndex)
            Set-CshAlias -Index $sharedIndex -SessionId 'abc' -Alias 'hello'
        })
        & $bound $index
        $bound = $module.NewBoundScriptBlock({
            param($sharedIndex)
            Get-CshAlias -Index $sharedIndex -SessionId 'abc'
        })
        (& $bound $index) | Should Be 'hello'
    }

    It 'clears aliases when set to blank' {
        $bound = $module.NewBoundScriptBlock({ New-CshIndex })
        $index = & $bound
        $bound = $module.NewBoundScriptBlock({
            param($sharedIndex)
            Set-CshAlias -Index $sharedIndex -SessionId 'abc' -Alias 'hello'
            Set-CshAlias -Index $sharedIndex -SessionId 'abc' -Alias ''
        })
        & $bound $index

        $bound = $module.NewBoundScriptBlock({
            param($sharedIndex)
            Get-CshAlias -Index $sharedIndex -SessionId 'abc'
        })
        (& $bound $index) | Should Be ''
    }

    It 'removes alias entries when cleared' {
        $bound = $module.NewBoundScriptBlock({ New-CshIndex })
        $index = & $bound
        $bound = $module.NewBoundScriptBlock({
            param($sharedIndex)
            Set-CshAlias -Index $sharedIndex -SessionId 'abc' -Alias 'hello'
            Set-CshAlias -Index $sharedIndex -SessionId 'abc' -Alias ''
        })
        & $bound $index

        $index.sessions.ContainsKey('abc') | Should Be $false
    }
}
