# Architecture

Agent Session Hub is a native Rust CLI with provider launchers `csx`, `clx`, `opx`, `agx`, plus the generic `sessionhub` entrypoint. Browsing does not require PowerShell; Windows installation and post-exit updating use PowerShell helpers.

- `src/app.rs`: CLI dispatch, hidden helper commands, browser actions
- `src/provider.rs`: provider metadata and launcher identity
- `src/session.rs`: Codex and Claude JSONL parsing plus OpenCode SQLite parsing, git context, query filtering
- `src/antigravity.rs`: read-only Antigravity CLI summary-index adapter and per-conversation storage accounting
- `src/config.rs`: alias persistence and legacy index migration
- `src/atomic.rs`: same-directory staged writes with flush and replacement
- `src/update.rs`: stable-release checks, bounded verified downloads, managed-install detection, and launcher transactions
- `src/update-windows.ps1`: post-exit replacement/rollback for Windows executable locks
- `src/cache.rs`: disposable, incremental Codex and Claude metadata cache
- `src/browser.rs`: `fzf` integration, row format, preview rendering
- `src/shell.rs`: shell profile integration for bash, zsh, fish, PowerShell, and Windows `cmd`
- `install.sh` / `install.ps1`: local build or release-binary installation

## Session loading

Codex and Claude histories are discovered from local JSONL files. The loader
compares each file's size and modification time with its cache entry. Unchanged
files reuse parsed metadata; new or changed files are reparsed. Entries for files
that disappeared are pruned. A file modified during parsing is not cached.

Aliases, relative ages, project existence, and live Git context are rebuilt from
current data on each load. Recorded provider branch information takes precedence
over the project's current branch when available. Claude's adjacent
`history.jsonl` is also checked; a history change invalidates its metadata cache.
OpenCode continues to load directly from SQLite.

Antigravity CLI reads `conversation_summaries.db` with SQLite's read-only flag
and a bounded busy timeout, including committed WAL updates from a running CLI.
It uses only summary fields, not opaque protobuf step payloads or subprocess
calls. Missing per-conversation databases and non-file workspaces are excluded;
invalid IDs cannot become filesystem paths or CLI options. File URIs are decoded
with native platform path handling. Last activity accepts both Go/SQLite and
RFC3339 timestamps, falling back to the database modification time when absent.
The index has no reliable creation timestamp, so previews omit the Started field.
Storage totals include each conversation's DB/WAL/SHM files, not shared data.
No additional hub metadata cache is used for this provider. Aliases remain in
the separate `antigravity-index.json` hub configuration file.

Cache files use a versioned schema and are scoped to the source root. Writes use
an atomic temporary-file replacement; invalid, obsolete, or unwritable caches
fall back to parsing the original history. On Unix, new cache directories use
owner-only permissions and files are created privately. The cache stores paths,
identifiers, timestamps, short text previews, and agent lineage, not full
conversation transcripts.

Workspace and conversation ordering uses last activity with deterministic
tie-breaks. The browser indexes all sessions by group once per render so it does
not rescan the entire history for every workspace.

## Picker compatibility

The browser targets fzf 0.29 and newer. Its initial rows are supplied from one
prepared, rewound temporary file; subsequent searches use `change:reload(...)`.
This avoids relying on newer `start`/`reload-sync` bindings, and an early exit
cannot interrupt a pipe writer. Initial queries and expanded agent/scan state are
included in the prepared snapshot.

Preview commands also receive the current query so an older fzf version's stale
selection cannot keep an unrelated workspace in the preview during a reload.
Startup errors are reported, while no-match and user-cancel exit codes remain
normal exits.

CI exercises the actual picker in a pseudo-terminal with upstream fzf 0.29.0 and
0.67.0 executables. To run those checks locally on Linux or macOS:

```sh
cargo build --locked
uv run --with pexpect==4.9.0 --with pyte==0.8.2 python scripts/test-fzf-compat.py \
  --binary target/debug/agent-session-hub --fzf /path/to/fzf
```

## Configuration

| Environment variable | Purpose |
| --- | --- |
| `CODEX_SESSION_HUB_CONFIG_ROOT` | Override the directory containing alias indexes |
| `CODEX_SESSION_HUB_CACHE_ROOT` | Override the disposable metadata cache directory |
| `CODEX_SESSION_HUB_DISABLE_CACHE=1` | Bypass cache reads and writes |
| `CODEX_SESSION_HUB_SESSION_ROOT` | Override the Codex sessions directory |
| `CODEX_SESSION_HUB_CLAUDE_SESSION_ROOT` | Override Claude's projects directory; its parent is used for `history.jsonl` |
| `CODEX_SESSION_HUB_OPENCODE_SESSION_ROOT` | Override the directory containing `opencode.db` |
| `CODEX_SESSION_HUB_ANTIGRAVITY_SESSION_ROOT` | Override Antigravity CLI's store containing `conversation_summaries.db` and `conversations/` |
| `AGENT_SESSION_HUB_NO_UPDATE_CHECK=1` | Disable background release checks (explicit checks still work) |

## Alias integrity and updates

Alias indexes are durable user data: parse/read errors, unsupported versions, and
unknown fields are rejected, not treated as empty. Both primary and legacy loaders
preserve the original on failure. Saves revalidate any existing destination, stage
beside it, preserve file permissions, flush, and atomically replace it. Valid legacy
migration leaves the original legacy index intact.

The updater only recognizes the installer-managed layout recorded in
`installation.json` and the launcher directory's `.agent-session-hub-install.json`.
It checks that the current executable belongs to that layout and that all binary
copies still match before updating. Archives/checksums are pinned to the official
GitHub release tag; response and extraction sizes are capped. Only the root
executable is read from the archive; archive paths cannot choose installation targets.
The downloaded executable's reported version must match the selected stable release.

New provider binary copies are optional while validating a legacy installation.
Once present, they are validated and updated with the original launchers,
including refusal of mismatched files or symlinks. Migration derives missing
shortcuts from the installed binary's provider registry, validates the installation,
then stages private files and uses no-clobber persistence. Existing Windows cmd
wrappers are retained; missing ones use the shared wrapper generator. A failed
creation rolls back only unchanged files created in that migration.

After replacement, the updater runs the newly installed canonical binary's
`__migrate-shortcuts` command, so future provider names come from the new version.
The Unix replacement transaction includes that step; Windows runs it after all
copies are replaced and before discarding recovery backups. An ordinary startup
also backfills missing shortcuts for upgrades performed by v0.4/v0.5, whose updater
cannot call the new hook. Startup avoids content hashing when no shortcuts are
missing, makes no network requests, and skips hidden query/preview commands.
Unrecorded source builds and staged version probes never find an installation
record and cannot migrate another installation. No shell profiles are modified.

Background checks only originate from interactive browse/selection entrypoints.
They cache an attempted check for 24 hours, use a short network timeout, and never
write to the TSV stdout protocol. Search/preview subprocesses do not perform them.
No downloaded code is executed by notification checks. Self-update is explicit
and requires confirmation (or `--yes`). Session/alias files and shell profiles are
outside the update write set. Windows replacement waits for the updater to exit;
native Windows CI exercises successful replacement and rollback under a file lock.

The default cache location is the OS cache directory under `agent-session-hub`
(for example, `~/.cache/agent-session-hub` on Linux). It can be removed at any
time and is recreated automatically. `csx doctor` reports the cache location and
whether caching is enabled.

## Documentation screenshots

The README images show real `fzf` sessions with fictional data and a custom
terminal palette. To regenerate them on Linux, install `fzf`, `uv`, and a
monospace font, then run:

```sh
cargo build --release --locked
uv run --with pillow --with pexpect --with pyte python scripts/capture-demo.py
```

The capture script uses a temporary session root and cancels both pickers. It
does not open or modify your real session history.
