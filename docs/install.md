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
- `sessionhub`
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

- `fzf` 0.29 or newer (including Ubuntu 22.04's packaged version)
- `codex`, `claude`, and/or `opencode`
- Rust only when installing from a local checkout

## Release platforms

Prebuilt archives are available for Linux x86-64, macOS Intel and Apple Silicon,
and Windows x86-64. Other targets require a source build and compatible system
dependencies.

## Update and refresh shell integration

Run the one-command installer again to update the executable and shell
integration. Reload the profile as directed by the installer so new management
commands are available through existing shell functions.

To refresh shell integration separately:

```sh
csx install-shell
```

## Uninstall

macOS / Linux:

```sh
curl -fsSL https://github.com/vinzify/Agent-Session-Hub/releases/latest/download/uninstall.sh | sh
```

Windows PowerShell:

```powershell
irm https://github.com/vinzify/Agent-Session-Hub/releases/latest/download/uninstall.ps1 | iex
```
