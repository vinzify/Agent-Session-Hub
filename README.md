<div align="center">
  <img src="banner.png" width="800" alt="Agent Session Hub Banner" />
  <h1>Agent Session Hub</h1>
  <p><strong>An <code>fzf</code>-powered session browser for Codex CLI and Claude Code. It uses PowerShell 7+ as the runtime and can be launched from PowerShell, zsh, and bash terminals.</strong></p>
  
  <p>
    <a href="https://learn.microsoft.com/en-us/powershell/"><img src="https://img.shields.io/badge/Built_with-PowerShell_7-blue?style=flat-square" alt="Built with PowerShell 7"></a>
    <a href="https://github.com/junegunn/fzf"><img src="https://img.shields.io/badge/Powered_by-fzf-orange?style=flat-square" alt="Powered by fzf"></a>
    <a href="https://github.com/vinzify/Codex-Session-Hub/blob/master/LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square" alt="License: MIT"></a>
  </p>
</div>

Agent Session Hub gives Codex CLI and Claude Code a global session browser. Instead of opening a project first and then trying to remember the right resume command, you can launch one command, search every session on your machine, preview context, and jump back into the right folder immediately.

The GitHub repository may still be named `Codex-Session-Hub`, but the runtime, module, config, and install folder names now use Agent Session Hub branding.

## Why It Exists

- Browse Codex sessions across all projects from one command
- Browse Claude sessions from the same shared browser model with a separate short command
- Resume directly into the correct project directory
- Distinguish worktrees and feature branches inside the same repo
- Rename sessions with persistent aliases
- Multi-select and delete supported sessions in bulk
- Preview project and session context before resuming
- Works on Windows, macOS, and Linux
- Uses PowerShell 7+ as the runtime engine
- Can be launched from PowerShell, zsh, and bash

## Features

- Global browser for Codex sessions across all repos on your machine with `csx`
- Global browser for Claude project transcripts with `clx`
- Workspace-aware grouping using repo + branch + working directory identity
- Visible repo and branch context in the list and preview for git-backed sessions
- Plain-text search by folder or repo name, plus `title:`, `repo:`, and `branch:` filters
- Safe bulk delete from providers that support delete, currently Codex
- Persistent aliases for sessions so important threads are easy to find later
- Shell integration for PowerShell, zsh, and bash
- Separate provider-aware aliases, previews, and resume behavior for Codex and Claude

## Quick Start

Windows PowerShell 7+:

1. Install:

```powershell
irm https://raw.githubusercontent.com/vinzify/Codex-Session-Hub/master/install.ps1 | iex
```

2. Reload your PowerShell profile:

```powershell
. $PROFILE
```

3. Launch Agent Session Hub:

```powershell
csx
```

Launch the Claude browser:

```powershell
clx
```

macOS / Linux terminal:

1. Install:

```sh
curl -fsSL https://raw.githubusercontent.com/vinzify/Codex-Session-Hub/master/install.sh | sh
```

2. Reload your shell:

```sh
source ~/.zprofile
```

If you use bash instead of zsh, reload `~/.bash_profile` or `~/.profile` instead.

3. Launch Agent Session Hub:

```sh
csx
```

Launch the Claude browser:

```sh
clx
```

## Requirements

Before installing, make sure you have:
- [PowerShell 7+](https://learn.microsoft.com/en-us/powershell/scripting/install/installing-powershell) available as `pwsh`
- Codex CLI in `PATH` if you want to use `csx`
- Claude Code in `PATH` if you want to use `clx`
- [`fzf`](https://github.com/junegunn/fzf) in `PATH`

Notes:
- PowerShell 7+ is the runtime requirement.
- Your interactive shell can still be PowerShell, zsh, or bash.
- On macOS and Linux, the shell launchers call `pwsh` under the hood.

Install `fzf`:
- Windows: `winget install junegunn.fzf`, `choco install fzf`, or `scoop install fzf`
- macOS: `brew install fzf`
- Linux: install with your distro package manager

## Install

Recommended on Windows PowerShell:

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
clx doctor
```

Recommended on macOS / Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/vinzify/Codex-Session-Hub/master/install.sh | sh
```

Then reload your shell profile:

```sh
source ~/.zprofile
```

If you use bash, reload `~/.bash_profile` or `~/.profile` instead.

Then verify the command is available:

```sh
csx doctor
clx doctor
```

Default install locations:
- Windows: `%LOCALAPPDATA%\AgentSessionHub`
- macOS / Linux: `~/.local/share/agent-session-hub`

From source:

```powershell
git clone https://github.com/vinzify/Codex-Session-Hub.git
cd Codex-Session-Hub
pwsh -File .\install.ps1
. $PROFILE
```

```sh
git clone https://github.com/vinzify/Codex-Session-Hub.git
cd Codex-Session-Hub
./install.sh
source ~/.zprofile
```

Uninstall:

```powershell
irm https://raw.githubusercontent.com/vinzify/Codex-Session-Hub/master/uninstall.ps1 | iex
```

```sh
curl -fsSL https://raw.githubusercontent.com/vinzify/Codex-Session-Hub/master/uninstall.sh | sh
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

clx
clx browse
clx rename <session-id> --name "My Claude alias"
clx reset <session-id>
clx doctor
```

Quick examples:

```powershell
csx
csx browse repo:Codex-Session-Hub
csx browse branch:feature/session-hub
csx browse title:installer
csx rename 019d12c0 --name "Release prep"
csx delete 019d12c0

clx
clx browse repo:payments
clx browse branch:feature/triage
clx rename a526782b --name "Claude project follow-up"
```

Browser controls:

| Key Binding | Action |
| --- | --- |
| `Enter` | Resume the focused session |
| `Tab` / `Shift-Tab` | Multi-select sessions |
| `Ctrl-D` | Delete all selected Codex sessions |
| `Ctrl-E` | Rename/Alias the focused session |
| `Ctrl-R` | Reset the focused session's title |
| `Esc` / `Ctrl-C` | Exit the browser |

Browser behavior:
- `csx` browses Codex sessions from the local Codex session store.
- `clx` browses Claude project transcripts and resumes them with `claude --resume <session-id>`.
- Sessions are grouped by workspace context, not only by raw folder path.
- Git-backed workspaces show repo and branch context in both the list and preview.
- Different worktrees in the same repo are separated using repo + branch + working directory identity behind the scenes.
- In `csx`, selecting a workspace header and pressing `Ctrl-D` deletes every session in that workspace after confirmation.
- `clx` intentionally disables delete in v1. Claude resume is supported, but deleting Claude transcript files is not exposed yet.
- Codex delete confirmation shows the workspace label, total session count, and a preview of the first session titles before anything is removed.
- The browser header is intentionally compact and split across two lines so search syntax and key bindings stay readable.

Provider model:
- `csx` is the Codex browser and keeps the current Codex-specific delete flow.
- `clx` is the Claude browser and uses Claude transcript files as the browse source.
- Both commands share the same list, preview, alias, search, and workspace-grouping model.
- Aliases are stored per provider, so the same session ID can exist in both ecosystems without collisions.

Search filters:
- Text query: folder or repo name
- Number query: session number prefix
- `title:<term>` or `t:<term>`: session title or alias
- `repo:<term>` or `r:<term>`: git repo name
- `branch:<term>` or `b:<term>`: git branch name

Preview details:
- Session preview shows the provider, title, working directory, repo root, branch, timestamps, and a short prompt excerpt.
- Workspace preview shows the provider, workspace label, path, repo context, branch summary, and recent sessions in that group.

## Configuration

Optional environment variables:

| Variable | Description |
| --- | --- |
| `CODEX_SESSION_HUB_SESSION_ROOT` | Override the default Codex session root used by `csx`. |
| `CODEX_SESSION_HUB_CLAUDE_SESSION_ROOT` | Override the default Claude projects root used by `clx`. |
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
