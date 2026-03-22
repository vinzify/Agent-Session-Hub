Describe 'install.ps1 bootstrap mode' {
    It 'supports fileless scriptblock execution' {
        $projectRoot = Split-Path -Parent $PSScriptRoot
        $installScriptPath = Join-Path $projectRoot 'install.ps1'
        $installRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('csh-install-test-' + [guid]::NewGuid().ToString('N'))

        try {
            $scriptContent = Get-Content -Path $installScriptPath -Raw
            $originalSourceRoot = [string]$env:CODEX_SESSION_HUB_SOURCE_ROOT
            $env:CODEX_SESSION_HUB_SOURCE_ROOT = $projectRoot
            & ([scriptblock]::Create($scriptContent)) -InstallRoot $installRoot -SkipShellIntegration *> $null

            (Test-Path (Join-Path $installRoot 'src/AgentSessionHub.psd1')) | Should -BeTrue
            (Test-Path (Join-Path $installRoot 'bin/csx.ps1')) | Should -BeTrue
            (Test-Path (Join-Path $installRoot 'bin/clx.ps1')) | Should -BeTrue
            (Test-Path (Join-Path $installRoot 'README.md')) | Should -BeTrue
            (Test-Path (Join-Path $installRoot 'install.sh')) | Should -BeTrue
            (Test-Path (Join-Path $installRoot 'uninstall.sh')) | Should -BeTrue
        }
        finally {
            $env:CODEX_SESSION_HUB_SOURCE_ROOT = $originalSourceRoot
            if (Test-Path $installRoot) {
                Remove-Item -LiteralPath $installRoot -Recurse -Force -ErrorAction SilentlyContinue
            }
        }
    }

    It 'delegates shell integration to the installed module' {
        $projectRoot = Split-Path -Parent $PSScriptRoot
        $installScriptPath = Join-Path $projectRoot 'install.ps1'
        $scriptContent = Get-Content -Path $installScriptPath -Raw

        $scriptContent | Should -Match 'Import-Module \$modulePath -Force'
        $scriptContent | Should -Match "Invoke-CsxCli -Arguments @\('install-shell'\)"
    }
}
