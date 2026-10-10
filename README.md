<p align="center">
  <img src="./banner.png" alt="Agent Session Hub banner">
</p>

# Agent Session Hub

Jump back into any Codex CLI, Claude Code, or OpenCode session from one picker.

Agent Session Hub is a native Rust CLI that gives AI coding tools a shared, `fzf`-powered session switcher with previews, aliases, and resume-in-project behavior.

## What It Solves

- One session picker across `codex`, `claude`, and `opencode`
- Resume from the right project directory instead of hunting through history
- Preview sessions before reopening them
- Keep Codex agents collapsed beneath their connected conversations, with separate storage totals
- Collapse generated Codex Security runs and clean old scan or agent data safely
- Rename, reset, and delete sessions without leaving the terminal
- Keep fast daily commands: `csx`, `clx`, and `opx`

## Install

macOS / Linux:

```sh
curl -fsSL https://github.com/vinzify/Agent-Session-Hub/releases/latest/download/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://github.com/vinzify/Agent-Session-Hub/releases/latest/download/install.ps1 | iex
```

The public installer downloads the matching release binary for your platform and installs:

- `sessionhub`
- `csx`
- `clx`
- `opx`

## Use It In 10 Seconds

```sh
sessionhub
sessionhub providers
sessionhub help
```

`sessionhub` is the discovery entrypoint. Once it becomes muscle memory, use the direct launchers:

```sh
csx
clx
opx
```

Examples:

```sh
csx browse repo:Agent-Session-Hub
clx browse branch:main
opx browse title:landing
csx rename <session-id> --name "Fix auth bug"
opx delete <session-id>
csx cleanup --dry-run
```

Inside the picker, press `Ctrl-A` on a Codex conversation to expand or collapse its
connected agent tree. Searching for an agent name or path reveals that agent with
its parent conversation. Deleting a conversation also deletes its connected agent
session files after the confirmation lists both counts; deleting an individual
expanded agent affects only that agent.

Generated Codex Security sessions are collapsed into one row per scan run. Press
`Ctrl-A` on a security-scan row to reveal its generated chats, then use `Ctrl-A`
again on a chat to reveal its agents.

## Storage Cleanup

Codex scans and subagents can accumulate large session logs. Preview everything
that currently qualifies for cleanup without changing any files:

```sh
csx cleanup --dry-run
```

Run the interactive cleaner with `csx cleanup`. Mark groups with `Tab`, press
`Enter` to review them, and type the exact word `DELETE` to confirm. `Esc` cancels
without deleting anything.

```sh
# Interactive cleanup using the safe default retention periods
csx cleanup

# Limit the candidates to generated Codex Security scan runs
csx cleanup --security-scans

# Preview linked agents older than 90 days
csx cleanup --dry-run --agents --older-than 90d

# Clean agents whose parent chat no longer exists
csx cleanup --orphans --older-than 7d
```

The default retention periods are 7 days for generated security scans, 30 days
for agents linked to retained chats, and 7 days for orphan agents. An age can use
hours, days, or weeks (`12h`, `7d`, `4w`); a bare number means days.

Cleanup has these safeguards:

- Normal root chats are never offered as cleanup candidates.
- Cleaning linked agents preserves their parent chat.
- A security-scan group includes only generated scan chats and agents under that
  scan's temporary run directory.
- A scan run is offered only after its newest session passes the retention age.
- Nothing is removed during `--dry-run`, and interactive deletion requires the
  exact `DELETE` confirmation.

## What Makes It Different

| Capability | Provider-native history | Agent Session Hub |
| --- | --- | --- |
| One picker across multiple AI CLIs | No | Yes |
| Fuzzy search with `fzf` | Varies | Yes |
| Preview pane before resume | Rare | Yes |
| Resume from the matching project directory | Varies | Yes |
| Repo and branch-aware grouping | No | Yes |
| Collapsed Codex agent hierarchy and storage totals | No | Yes |
| Collapsed security-scan runs and guarded storage cleanup | No | Yes |
| Rename and reset session aliases | Rare | Yes |
| Delete from the picker | Varies | Yes |

## Supported Providers

- Codex CLI via `csx`
- Claude Code via `clx`
- OpenCode via `opx`

Requirements:

- `fzf` in `PATH`
- At least one of `codex`, `claude`, or `opencode`
- Rust only when installing from a local checkout

## Shell Behavior

The installers automatically run:

```sh
csx install-shell
```

That shell integration keeps the direct launchers shell-native for resume flows:

- select a session
- change into the matching project directory when possible
- reopen the session with `codex`, `claude`, or `opencode`

To remove shell integration:

```sh
csx uninstall-shell
```

## Local Install

```sh
git clone https://github.com/vinzify/Agent-Session-Hub.git
cd Agent-Session-Hub
./install.sh
```

```powershell
git clone https://github.com/vinzify/Agent-Session-Hub.git
cd Agent-Session-Hub
.\install.ps1
```

## Repo Layout

- `src/app.rs`: CLI dispatch and provider-mode selection
- `src/session.rs`: Codex JSONL, Claude JSONL, and OpenCode SQLite parsing plus filtering
- `src/browser.rs`: `fzf` row generation, preview output, and picker actions
- `src/cleanup.rs`: retention policies and guarded Codex session cleanup
- `src/config.rs`: alias persistence and legacy index import
- `src/shell.rs`: shell integration for bash, zsh, fish, PowerShell, and Windows `cmd`
- `src/provider.rs`: provider metadata and runtime behavior

## Contributing

Start with [CONTRIBUTING.md](./CONTRIBUTING.md).
