# Changelog

## 0.3.0

- Added OpenCode provider support with the new `opx` launcher
- Loaded OpenCode sessions from the local SQLite store and restored preview support in the shared browser
- Added OpenCode resume and delete handling plus installer and shell integration updates

## 0.2.5

- Fixed preview-pane row parsing so project and session previews render again in `fzf`
- Added coverage for prefixed rows that include extra tab-delimited columns

## 0.2.4

- Internal release alignment for the preview-pane fix

## 0.2.3

- Kept the selector open after interactive rename, reset, and delete actions
- Moved interactive delete prompts to stderr so shell wrappers do not swallow them

## 0.2.2

- Fixed shell selection handling so `Ctrl-D`, `Ctrl-E`, and `Ctrl-R` do not resume sessions
- Added parser coverage for shortcut-based `fzf` output

## 0.2.1

- Replaced deprecated `fzf` flags with older-version-compatible bindings
- Normalized selected rows in Rust instead of relying on `--accept-nth`

## 0.2.0

- Replaced the PowerShell runtime with a native Rust CLI
- Preserved the `csx` and `clx` command surface with provider-aware shell integration
- Added GitHub Actions CI for Rust and release automation for one-line install artifacts
- Removed the legacy PowerShell module, shims, and Pester test suite
- Updated install flows to consume native GitHub Release archives
- Added Windows `cmd` launchers and retained the `cxs` alias
- Updated docs and contributor guidance for the Rust-native release process

## 0.1.0

- Initial modular `fzf`-based release
- Added global session browsing grouped by project
- Added browser actions for resume, rename, reset title, and delete
- Added direct CLI commands for `rename`, `reset`, `delete`, and `doctor`
- Added preview pane with project and session views
- Added a self-bootstrapping `install.ps1` for user-local installs
- Added a self-contained `uninstall.ps1`
- Documented one-line install and uninstall flows
- Updated CI to `actions/checkout@v5`
