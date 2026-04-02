# Contributing

## Local setup

1. Install Rust from https://rustup.rs/
2. Install `fzf`
3. Install `codex`, `claude`, and/or `opencode`
4. Run `cargo test`

## Local install

```sh
./install.sh
```

Or on Windows:

```powershell
.\install.ps1
```

## Notes

- The public commands are `csx`, `clx`, and `opx`
- The legacy alias `cxs` still maps to `csx`
- The Rust runtime lives under `src/*.rs`
- `cargo test` is the test entrypoint

## Releases

Public one-line installs pull from GitHub Releases.

To publish a release:

1. Create and push a semver tag like `v0.1.0`
2. Let `.github/workflows/release.yml` build and upload the release archives
3. Verify the release contains:
   - `install.sh`
   - `install.ps1`
   - `uninstall.sh`
   - `uninstall.ps1`
   - `agent-session-hub-x86_64-unknown-linux-gnu.tar.gz`
   - `agent-session-hub-x86_64-apple-darwin.tar.gz`
   - `agent-session-hub-aarch64-apple-darwin.tar.gz`
   - `agent-session-hub-x86_64-pc-windows-msvc.zip`
