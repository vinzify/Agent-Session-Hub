# Architecture

Agent Session Hub is a native Rust CLI that preserves the `csx` and `clx` command surface without any PowerShell runtime dependency.

- `src/app.rs`: CLI dispatch, hidden helper commands, browser actions
- `src/provider.rs`: provider metadata and launcher identity
- `src/session.rs`: Codex and Claude session parsing, git context, query filtering
- `src/config.rs`: alias persistence and legacy index migration
- `src/browser.rs`: `fzf` integration, row format, preview rendering
- `src/shell.rs`: shell profile integration for bash, zsh, fish, PowerShell, and Windows `cmd`
- `install.sh` / `install.ps1`: local build or release-binary installation
