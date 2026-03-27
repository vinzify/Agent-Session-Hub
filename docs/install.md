# Install

Agent Session Hub no longer requires PowerShell as the runtime. The application is a native Rust binary.

## One-command install

macOS / Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/vinzify/Agent-Session-Hub/master/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/vinzify/Agent-Session-Hub/master/install.ps1 | iex
```

## Local source install

The local source path builds the binary with Cargo and installs:

- `agent-session-hub`
- `csx`
- `clx`

Example:

```sh
./install.sh
```

```powershell
.\install.ps1
```

## Requirements

- `fzf`
- `codex` and/or `claude`
- Rust only when installing from a local checkout
