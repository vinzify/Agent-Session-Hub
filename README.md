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
- `cxs` as a legacy alias that forwards to `csx`

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
```

## What Makes It Different

| Capability | Provider-native history | Agent Session Hub |
| --- | --- | --- |
| One picker across multiple AI CLIs | No | Yes |
| Fuzzy search with `fzf` | Varies | Yes |
| Preview pane before resume | Rare | Yes |
| Resume from the matching project directory | Varies | Yes |
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

If you used the old PowerShell-era releases, `cxs` is just the old shortcut for Codex and still forwards to `csx`.

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
- `src/config.rs`: alias persistence and legacy index import
- `src/shell.rs`: shell integration for bash, zsh, fish, PowerShell, and Windows `cmd`
- `src/provider.rs`: provider metadata and runtime behavior

## Launching It

For ready-to-post launch copy and demo checklist ideas, see [docs/launch.md](./docs/launch.md).

## Contributing

Start with [CONTRIBUTING.md](./CONTRIBUTING.md).
