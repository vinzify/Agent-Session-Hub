# Architecture

Agent Session Hub is now a shared browser core with provider-specific session and runtime adapters.

- `src/Common`: shared path, provider, and formatting helpers
- `src/Metadata`: alias and config persistence, scoped per provider
- `src/Sessions`: session discovery and parsing for both Codex and Claude stores
- `src/Fzf`: browser rows, preview, filters, and `fzf` integration
- `src/Runtime`: provider-specific resume behavior plus shared shell integration
- `src/Commands`: CLI dispatch for `csx` and `clx`
