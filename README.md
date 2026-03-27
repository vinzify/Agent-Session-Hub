# Agent Session Hub

Agent Session Hub is a native Rust CLI that gives Codex CLI and Claude Code a shared, `fzf`-powered session browser.

## Features

- `csx` for Codex sessions
- `clx` for Claude sessions
- One native binary surfaced as `csx` and `clx`
- Shared browser model across both providers
- Git-aware workspace grouping for repos, branches, and worktrees
- Query filters for `title:`, `repo:`, and `branch:`
- Alias rename and reset
- Codex bulk delete support
- Preview panes and hidden `fzf` helper commands
- Shell integration for bash, zsh, fish, PowerShell, and Windows `cmd`
- Legacy `cxs` alias support

## Requirements

- `fzf` in `PATH`
- At least one supported provider CLI installed:
  - `codex` for `csx`
  - `claude` for `clx`

If you install from a local checkout, you also need a Rust toolchain.

## Install

macOS / Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/vinzify/Agent-Session-Hub/master/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/vinzify/Agent-Session-Hub/master/install.ps1 | iex
```

From a local checkout:

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

The local install path builds the release binary with Cargo, installs `agent-session-hub`, `csx`, `clx`, and `cxs`, and runs `csx install-shell` unless skipped.

## Usage

```sh
csx
csx browse repo:Agent-Session-Hub
csx rename <session-id> --name "My alias"
csx reset <session-id>
csx delete <session-id>
csx doctor
```

```sh
clx
clx browse branch:main
clx rename <session-id> --name "Important chat"
clx reset <session-id>
clx doctor
```

## Shell Integration

The install scripts add shell integration automatically by calling:

```sh
csx install-shell
```

That integration keeps `csx` and `clx` shell-native for resume flows:

- select a session
- change directory in the current shell when possible
- run `codex --resume` or `claude --resume`

The legacy alias `cxs` continues to forward to `csx`.

To remove integration:

```sh
csx uninstall-shell
```

## Architecture

The repo now contains only the Rust implementation:

- `src/app.rs`: command dispatch and provider-mode selection
- `src/session.rs`: session parsing, display shaping, and query filtering
- `src/browser.rs`: `fzf` row generation, preview output, and browser actions
- `src/config.rs`: alias index persistence and legacy index import
- `src/shell.rs`: shell integration block generation
- `src/provider.rs`: provider metadata and runtime behavior
