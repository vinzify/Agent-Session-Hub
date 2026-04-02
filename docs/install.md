# Install

Agent Session Hub no longer requires PowerShell as the runtime. The application is a native Rust binary.

## One-command install

macOS / Linux:

```sh
curl -fsSL https://github.com/vinzify/Agent-Session-Hub/releases/latest/download/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://github.com/vinzify/Agent-Session-Hub/releases/latest/download/install.ps1 | iex
```

## Local source install

The local source path builds the binary with Cargo and installs:

- `agent-session-hub`
- `csx`
- `clx`
- `opx`

Example:

```sh
./install.sh
```

```powershell
.\install.ps1
```

## Requirements

- `fzf`
- `codex`, `claude`, and/or `opencode`
- Rust only when installing from a local checkout
