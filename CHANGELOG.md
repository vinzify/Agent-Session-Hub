# Changelog

## 0.6.1

- Accept lowercase `-v` alongside `-V`, `--version`, and `version`, including shell-wrapper passthrough and provider-qualified commands
- Added end-to-end provider-menu checks with isolated histories and stub CLIs, covering selection, invalid input, cancellation, and query forwarding

## 0.6.0

- Added confirmed, recoverable local CLI history deletion for Claude Code and Antigravity, with offline-writer checks, scoped changes, private compressed backups, and explicit recovery list/restore/purge commands
- Added Claude subagent sidecars, OpenCode parent IDs, and Antigravity subagent metadata to connected agent trees; ordinary Antigravity forks remain independent chats
- Enabled agent/orphan cleanup across all providers and explicit `--chats` opt-in cleanup (90 days by default), protecting newer or undeletable descendants from cascading deletion
- Protected OpenCode families with hidden archived descendants and kept alternate OpenCode stores browse-only to prevent native deletion from targeting a different database
- Indexed cleanup families and cascade protection once, avoiding repeated whole-history scans per agent
- Kept Codex scan grouping provider-specific and clarified transcript/payload sizes versus disk space actually reclaimed
- Added fixture-based deletion/recovery and refusal regressions; documented local-only scope, provider-specific limits, and recovery storage management

## 0.5.1

- Automatically create missing provider shortcuts in installer-managed installations, using the recorded custom launcher directory
- Run the new version's shortcut migration immediately after updates, including through the Windows post-exit helper
- Backfill updates performed by v0.4/v0.5 on the next ordinary hub command, without rerunning an installer or checking the network
- Refuse to overwrite existing files or symlinks; preserve custom Windows wrappers and roll back newly created shortcuts on failure
- Keep source builds, staged version probes, hidden query/preview helpers, shell profiles, sessions, and aliases outside automatic migration
- Added startup, idempotence, collision, permission, custom-root, and update/migration rollback regression tests
- Updated README and installation guidance to replace the manual-shortcut workaround

## 0.5.0

- Added Google Antigravity CLI as a fourth provider: `agx` / `sessionhub antigravity`, resuming with `agy --conversation <id>` from the recorded workspace
- Added read-only SQLite summary previews, search, local aliases, last-activity ordering, and per-conversation DB/WAL/SHM sizes
- Excluded stale IDE-only entries without local CLI databases and remote-only workspaces; left Antigravity deletion and cleanup disabled
- Added Antigravity launchers to POSIX/PowerShell/cmd installers and shell integration, preserving self-update compatibility with existing installations
- Documented provider scope, local data paths, and the shortcut upgrade procedure

## 0.4.0

- Added `version`, `check-update`, and confirmed `update [--yes]` commands through sessionhub/csx/clx/opx
- Added cached daily background update notices for interactive browsing, with an opt-out and quiet offline fallback
- Kept all network checks out of search/preview subprocesses and stdout selection protocols
- Verified official release archives using published SHA-256 checksums and executable versions before replacing managed launcher copies
- Added installation records for custom paths, source-build/symlink/mismatched-launcher guards, rollback, and post-exit Windows replacement
- Published a single SHA256SUMS covering all platform archives/installers; installers now verify archive checksums too
- Refreshed shell management-command routing and documented the one-time upgrade required by older installations
- Required successful source-commit CI before publishing releases and pinned manual release builds to the requested tag

## 0.3.13

- Fixed #7: corrupt primary/legacy alias indexes now fail clearly instead of becoming empty and being overwritten
- Refused unsupported versions and unknown alias fields to preserve future-format data
- Saved alias indexes through same-directory staged files, flush, and atomic replacement; rechecked the destination before saving
- Added corruption, migration, preservation, and failed-replacement regression coverage

## 0.3.12

- Fixed picker compatibility with fzf 0.29, including Ubuntu 22.04's packaged version, by integrating the compatible reload binding from [PR #6](https://github.com/vinzify/Agent-Session-Hub/pull/6) by @dongwook-chan
- Preserved initial session loading for empty and prefilled queries using a single prepared input snapshot, avoiding competing startup streams and broken pipes on early cancellation
- Kept previews aligned with the current search when older fzf versions retain a previous selection during a reload
- Reported fzf startup failures instead of silently treating them as cancellation
- Added real terminal integration checks for fzf 0.29.0 and 0.67.0 in CI, covering startup, search, previews, selection, cancellation, and agent/scan expansion

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
