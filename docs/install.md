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
- `agx`

Example:

```sh
./install.sh
```

```powershell
.\install.ps1
```

## Requirements

- `fzf` 0.29 or newer (including Ubuntu 22.04's packaged version)
- At least one of `codex`, `claude`, `opencode`, or Antigravity CLI (`agy`)
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

These management commands also work through `csx`, `clx`, `opx`, and `agx`. Update checks
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
The newly installed version creates any missing provider shortcuts, preserving
custom paths and refusing to overwrite existing files or symlinks. Windows runs
that step through the post-exit helper. Failed migration rolls back new shortcuts;
the update transaction also attempts to restore the original binaries on failure.

On Windows, a temporary PowerShell helper waits for this command to exit before
replacing locked executables. Other running hub processes may still prevent an
update; the helper reports failure and attempts rollback. Reopen your terminal
after the initial installer upgrade so existing shell functions know the new commands.

To refresh shell integration separately:

```sh
csx install-shell
```

### Adding the Antigravity shortcut to an older install

With v0.5.1 or newer, missing shortcuts such as `agx` are added automatically in
the recorded launcher directory. No installer rerun or shell reload is required
to use the new command.

One-time bootstrap from v0.4.x or v0.5.0:

```sh
sessionhub update
sessionhub providers
agx
```

The old updater cannot invoke new post-update logic. The first ordinary command
run by the updated binary repairs missing shortcuts locally; future updates run
that migration immediately. This also works when the current version is already
installed, without downloading it again. Hidden query/preview commands and staged
`--version` probes do not trigger repair. Unrecorded/source builds are not modified.

Shell profiles are deliberately left alone. To add the optional `agx` shell
function (for changing the parent shell's working directory), run
`sessionhub install-shell` and reload your shell. The binary shortcut works without it.

Antigravity support requires the `agy` CLI and its SQLite conversation store,
normally `~/.gemini/antigravity-cli`. It does not read the older IDE's private
conversation format directly. See [provider details](../README.md#google-antigravity-cli).

## Uninstall

macOS / Linux:

```sh
curl -fsSL https://github.com/vinzify/Agent-Session-Hub/releases/latest/download/uninstall.sh | sh
```

Windows PowerShell:

```powershell
irm https://github.com/vinzify/Agent-Session-Hub/releases/latest/download/uninstall.ps1 | iex
```
