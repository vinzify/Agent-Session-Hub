@{
    RootModule        = 'AgentSessionHub.psm1'
    ModuleVersion     = '0.1.0'
    GUID              = 'f8d51637-23dc-45e1-aee4-ebfbc412b7d8'
    Author            = 'vinzify'
    CompanyName       = 'Open Source'
    Copyright         = '(c) vinzify. All rights reserved.'
    Description       = 'Agent Session Hub: browse and resume Codex and Claude CLI sessions with an fzf-powered terminal interface.'
    PowerShellVersion = '7.0'
    FunctionsToExport = @('Invoke-CsxCli', 'Invoke-ClxCli')
    CmdletsToExport   = @()
    VariablesToExport = @()
    AliasesToExport   = @()
}
