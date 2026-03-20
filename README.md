<div align="center">
  <img src="banner.png" width="800" alt="Codex Session Hub Banner" />
  <h1>Codex Session Hub</h1>
  <p><strong>An <code>fzf</code>-powered PowerShell 7 CLI for browsing, resuming, renaming, and deleting Codex CLI sessions across projects.</strong></p>
  
  <p>
    <a href="https://learn.microsoft.com/en-us/powershell/"><img src="https://img.shields.io/badge/Built_with-PowerShell_7-blue?style=flat-square" alt="Built with PowerShell 7"></a>
    <a href="https://github.com/junegunn/fzf"><img src="https://img.shields.io/badge/Powered_by-fzf-orange?style=flat-square" alt="Powered by fzf"></a>
    <a href="https://github.com/vinzify/Codex-Session-Hub/blob/master/LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square" alt="License: MIT"></a>
  </p>
</div>

Codex Session Hub gives Codex CLI a global session browser. Instead of opening a project first and then running `codex resume`, you can launch one command, search every session on your machine, preview context, and jump back into the right folder immediately.

## Why It Exists

- Browse Codex sessions across all projects from one command
- Resume directly into the correct project directory
- Rename sessions with persistent aliases
- Multi-select and delete sessions in bulk
- Preview project and session context before resuming
- Works on Windows, macOS, and Linux with PowerShell 7

## Quick Start

1. Install:

```powershell
irm https://raw.githubusercontent.com/vinzify/Codex-Session-Hub/master/install.ps1 | iex
```

2. Reload your PowerShell profile:

```powershell
. $PROFILE
```

3. Launch Codex Session Hub:

```powershell
csx
```

## Requirements

Before installing, make sure you have:
- [PowerShell 7+](https://learn.microsoft.com/en-us/powershell/scripting/install/installing-powershell)
- Codex CLI in `PATH`
- [`fzf`](https://github.com/junegunn/fzf) in `PATH`

Install `fzf`:
- Windows: `winget install junegunn.fzf`, `choco install fzf`, or `scoop install fzf`
- macOS: `brew install fzf`
- Linux: install with your distro package manager

## Install

Recommended:

```powershell
irm https://raw.githubusercontent.com/vinzify/Codex-Session-Hub/master/install.ps1 | iex
```

Then reload your shell:

```powershell
. $PROFILE
```

Then verify the command is available:

```powershell
csx doctor
```

Default install locations:
- Windows: `%LOCALAPPDATA%\CodexSessionHub`
- macOS / Linux: `~/.local/share/codex-session-hub`

From source:

```powershell
git clone https://github.com/vinzify/Codex-Session-Hub.git
cd Codex-Session-Hub
pwsh -File .\install.ps1
. $PROFILE
```

Uninstall:

```powershell
irm https://raw.githubusercontent.com/vinzify/Codex-Session-Hub/master/uninstall.ps1 | iex
```

## Usage

Main commands:

```powershell
csx
csx browse
csx browse desktop
csx rename <session-id> --name "My friendly alias"
csx reset <session-id>
csx delete <session-id>
csx doctor
```

Browser controls:

| Key Binding | Action |
| --- | --- |
| `Enter` | Resume the focused session |
| `Tab` / `Shift-Tab` | Multi-select sessions |
| `Ctrl-D` | Delete all selected sessions |
| `Ctrl-E` | Rename/Alias the focused session |
| `Ctrl-R` | Reset the focused session's title |
| `Esc` / `Ctrl-C` | Exit the browser |

Search filters:
- Text query: folder or project name
- Number query: session number prefix
- `title:<term>` or `t:<term>`: session title or alias

## Configuration

Optional environment variables:

| Variable | Description |
| --- | --- |
| `CODEX_SESSION_HUB_SESSION_ROOT` | Override the default location where sessions are stored. |
| `CODEX_SESSION_HUB_CONFIG_ROOT` | Override the default location for configuration files. |
| `CODEX_SESSION_HUB_FZF_OPTS` | Add custom options to the `fzf` command. |

## Development

Tests are written using [Pester](https://pester.dev/). To run the test suite:

```powershell
Import-Module Pester -MinimumVersion 5.0 -Force
Invoke-Pester -Path .\tests
```

## License

[MIT](LICENSE)
