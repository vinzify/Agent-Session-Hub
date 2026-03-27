# Changelog

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
