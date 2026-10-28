# Changelog

## 0.3.11

- Added an incremental metadata cache for Codex and Claude histories, avoiding repeated parsing of unchanged session logs during search and preview
- Refreshed aliases, project existence, relative ages, and Git state independently of cached metadata; invalidated changed logs and pruned deleted entries automatically
- Ordered workspaces and conversations by last activity and showed last-updated dates in picker rows
- Fixed cleanup and help command routing in bash, zsh, fish, PowerShell, and Windows cmd shell integration
- Fixed Unicode title truncation, unnecessary provider prompts for explicit `sessionhub` commands, and quoted executable paths containing apostrophes
- Rechecked cleanup eligibility after confirmation and rejected out-of-range retention ages
- Improved scan-run identification and respected custom Claude roots when reading history
- Redesigned the public README with real terminal captures, provider capabilities, search examples, and cleanup guidance

## 0.3.10

- Collapsed generated Codex Security sessions into one expandable row per scan run
- Kept generated scan chats out of normal conversation numbering
- Added `csx cleanup` with category and retention filters, a non-destructive dry run, interactive group selection, and exact confirmation before deletion
- Protected ordinary root chats and retained parent chats when cleaning linked agents
- Documented scan grouping, storage cleanup defaults, examples, and safety guarantees for public users

## 0.3.9

- Grouped Codex subagent sessions beneath their connected conversations instead of listing them as ordinary chats
- Added `Ctrl-A` expansion for agent trees, agent names and paths in search, and connected-chat previews
- Added separate chat, agent, and combined storage totals
- Made root conversation deletion include its connected agent session files after an explicit combined confirmation
- Suppressed harmless broken-pipe failures when `fzf` closes a session-list reload early

## 0.3.8

- Added per-session storage sizes to browser rows and previews, plus combined workspace totals
- Added approximate per-session payload sizes for OpenCode sessions stored in its shared SQLite database
- Updated the POSIX installer to replace active launcher binaries atomically instead of failing with `Text file busy`

## 0.3.7

- Fixed `fzf` session loading to avoid a broken-pipe race when the initial reload closes stdin
- Fixed project-header selection to resume the latest session instead of treating the project as an invalid multi-selection

## 0.3.6

- Fixed Windows `fzf` preview and query reload commands to quote the installer binary correctly
- Fixed provider resume launches to execute the resolved provider binary path directly on Windows

## 0.3.5

- Fixed PowerShell shell integration to add the Agent Session Hub launcher directory to `PATH`, so `sessionhub` resolves after reloading the profile

## 0.3.4

- Fixed Windows installation when the default shim directory and install directory resolve to the same `bin` path

## 0.3.3

- Fixed PowerShell one-line installation so `irm .../install.ps1 | iex` works under `Invoke-Expression`
- Fixed Linux shell profile detection during shell integration setup

## 0.3.2

- Removed the deprecated `cxs` alias to keep the public command surface focused on `sessionhub`, `csx`, `clx`, and `opx`
- Cleaned installers, shell integration, and docs to match the simplified launcher set

## 0.3.1

- Added the generic `sessionhub` entrypoint for provider discovery and cross-provider help
- Installed `sessionhub` alongside `csx`, `clx`, and `opx` in the public installers
- Routed `fzf` preview and reload helpers through explicit provider flags so the generic entrypoint works inside the picker

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
- Added Windows `cmd` launchers for the Rust-native CLI
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
