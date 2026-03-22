# Install Guide

The GitHub repository may still be `Codex-Session-Hub`, but the installed product now uses Agent Session Hub naming with two entrypoints:
- `csx` for Codex sessions
- `clx` for Claude sessions

PowerShell 7+ is the runtime requirement, but you can launch the tool from PowerShell, zsh, or bash once `pwsh` is available.

## Recommended

### Windows PowerShell 7+

Once the GitHub repository is public:

```powershell
irm https://raw.githubusercontent.com/vinzify/Codex-Session-Hub/master/install.ps1 | iex
```

Then reload your shell:

```powershell
. $PROFILE
csx doctor
clx doctor
```

### macOS / Linux terminal

```sh
curl -fsSL https://raw.githubusercontent.com/vinzify/Codex-Session-Hub/master/install.sh | sh
```

Then reload your shell profile:

```sh
source ~/.zprofile
```

If you use bash, reload `~/.bash_profile` or `~/.profile` instead.

## Requirements

1. Install PowerShell 7 as `pwsh`.
2. Install `fzf`.
3. Make sure `codex` is available in `PATH` if you want `csx`.
4. Make sure `claude` is available in `PATH` if you want `clx`.

## From Source

1. Clone the repo.
2. On Windows PowerShell, run `pwsh -File .\install.ps1`.
3. On macOS/Linux, run `./install.sh`.
4. Reload your shell profile.
5. Run `csx doctor` and `clx doctor`.
