# Codex Session Hub

`Codex Session Hub` is a PowerShell 7 CLI for browsing, resuming, renaming, resetting, and deleting Codex CLI sessions with an `fzf`-powered terminal interface.

## Features

- Browse all Codex sessions across projects from one command
- Resume a session in the correct project directory
- Rename sessions with persistent aliases
- Reset renamed sessions back to the generated title
- Multi-select sessions in `fzf` and bulk delete them
- Preview project summaries and session details in a side pane
- Works with PowerShell 7 on Windows, macOS, and Linux

## Requirements

- PowerShell 7+
- Codex CLI available in `PATH`
- `fzf` available in `PATH`

## Install

1. Clone the repo.
2. Install `fzf`.
3. Run:

```powershell
pwsh -File .\install.ps1
```

4. Reload your shell:

```powershell
. $PROFILE
```

## Install fzf

### Windows

- `winget install junegunn.fzf`
- or `choco install fzf`
- or `scoop install fzf`

### macOS

- `brew install fzf`

### Linux

- install from your distro package manager, for example `apt`, `dnf`, `pacman`, or `zypper`

## Usage

```powershell
csx
csx browse
csx browse desktop
csx rename <session-id> --name "My alias"
csx reset <session-id>
csx delete <session-id>
csx doctor
```

## Search

- Text query: filters by folder name
- Number query: filters by session number prefix
- `title:<term>` or `t:<term>`: searches session titles

## Browser Keys

- `Enter`: resume focused session
- `Tab` / `Shift-Tab`: multi-select
- `Ctrl-D`: delete selected sessions
- `Ctrl-E`: rename alias for focused session
- `Ctrl-R`: reset focused session title
- `Esc` / `Ctrl-C`: exit

## Configuration

Optional environment variables:

- `CODEX_SESSION_HUB_SESSION_ROOT`
- `CODEX_SESSION_HUB_CONFIG_ROOT`
- `CODEX_SESSION_HUB_FZF_OPTS`

## Development

Tests use Pester:

```powershell
Invoke-Pester -Path .\tests
```

## License

MIT
