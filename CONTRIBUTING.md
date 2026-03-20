# Contributing

## Setup

1. Install PowerShell 7.
2. Install `fzf`.
3. Clone the repository.
4. Run `pwsh -File .\install.ps1`.

## Standards

- Keep files focused and modular.
- Add tests for session parsing, ordering, and metadata changes.
- Avoid changing Codex session files except for explicit delete operations.

## Tests

```powershell
Invoke-Pester -Path .\tests
```
