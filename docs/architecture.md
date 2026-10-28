# Architecture

Agent Session Hub is a native Rust CLI that preserves the provider launchers `csx`, `clx`, `opx`, plus the generic `sessionhub` entrypoint without any PowerShell runtime dependency.

- `src/app.rs`: CLI dispatch, hidden helper commands, browser actions
- `src/provider.rs`: provider metadata and launcher identity
- `src/session.rs`: Codex and Claude JSONL parsing plus OpenCode SQLite parsing, git context, query filtering
- `src/config.rs`: alias persistence and legacy index migration
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

Cache files use a versioned schema and are scoped to the source root. Writes use
an atomic temporary-file replacement; invalid, obsolete, or unwritable caches
fall back to parsing the original history. On Unix, new cache directories use
owner-only permissions and files are created privately. The cache stores paths,
identifiers, timestamps, short text previews, and agent lineage, not full
conversation transcripts.

Workspace and conversation ordering uses last activity with deterministic
tie-breaks. The browser indexes all sessions by group once per render so it does
not rescan the entire history for every workspace.

## Configuration

| Environment variable | Purpose |
| --- | --- |
| `CODEX_SESSION_HUB_CONFIG_ROOT` | Override the directory containing alias indexes |
| `CODEX_SESSION_HUB_CACHE_ROOT` | Override the disposable metadata cache directory |
| `CODEX_SESSION_HUB_DISABLE_CACHE=1` | Bypass cache reads and writes |
| `CODEX_SESSION_HUB_SESSION_ROOT` | Override the Codex sessions directory |
| `CODEX_SESSION_HUB_CLAUDE_SESSION_ROOT` | Override Claude's projects directory; its parent is used for `history.jsonl` |
| `CODEX_SESSION_HUB_OPENCODE_SESSION_ROOT` | Override the directory containing `opencode.db` |

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
