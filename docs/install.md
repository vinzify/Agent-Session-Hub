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

For installations older than 0.4.0, run the one-command installer once and reload
the profile as directed. This installs the update-capable binary, records the
actual installation/launcher directories, and refreshes shell routing.

After that:

```sh
sessionhub version
sessionhub check-update
sessionhub update
# Explicitly approve an unattended update:
sessionhub update --yes
```

These management commands also work through `csx`, `clx`, and `opx`. Update checks
use GitHub's latest stable release; prereleases and downgrades are not installed.
The updater downloads a platform archive and matching `SHA256SUMS` from the same
release tag over HTTPS, verifies its SHA-256 and executable version, and only then
replaces managed launchers. Checksums detect damaged/mismatched downloads; they
are not an independent publisher signature. Trust remains in the official GitHub
repository and HTTPS distribution.

Interactive browsing starts a bounded background check once per day. Cached
notices appear on startup; a completed new check can show after leaving the
picker, or on the next launch. Offline failures are quiet and never block browsing.
Disable automatic checks with `AGENT_SESSION_HUB_NO_UPDATE_CHECK=1`.

The updater refuses unrecorded/source builds, symlink launchers, or installations
whose launcher copies no longer match. Repair those with the installer rather
than forcing an overwrite. Custom installer roots are supported through the
recorded locations. No `sudo`, session deletion, or shell-profile rewriting occurs
during self-update. The canonical executable and all copied binary launchers are
updated together with recovery on error; existing script wrappers remain in place.

On Windows, a temporary PowerShell helper waits for this command to exit before
replacing locked executables. Other running hub processes may still prevent an
update; the helper reports failure and attempts rollback. Reopen your terminal
after the initial installer upgrade so existing shell functions know the new commands.

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
