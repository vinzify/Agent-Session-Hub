<div align="center">
  <img src="banner.png" width="800" alt="Codex Session Hub Banner" />
  <h1>Codex Session Hub</h1>
  <p><strong>A blazing fast, <code>fzf</code>-powered PowerShell 7 CLI for browsing and managing Codex CLI sessions.</strong></p>
  
  <p>
    <a href="https://learn.microsoft.com/en-us/powershell/"><img src="https://img.shields.io/badge/Built_with-PowerShell_7-blue?style=flat-square" alt="Built with PowerShell 7"></a>
    <a href="https://github.com/junegunn/fzf"><img src="https://img.shields.io/badge/Powered_by-fzf-orange?style=flat-square" alt="Powered by fzf"></a>
    <a href="https://github.com/vinzify/Codex-Session-Hub/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square" alt="License: MIT"></a>
  </p>
</div>

Codex Session Hub transforms the way you manage and resume your Codex CLI sessions. It provides an `fzf`-powered interface right in your terminal, allowing you to instantly browse and resume sessions across all your projects without ever needing to manually change directories.

## ✨ Features

* **Universal Browse:** Access your Codex sessions across all projects from one powerful command.
* **Smart Resume:** Select a session and instantly resume it in the correct project directory.
* **Persistent Aliases (Rename):** Give your auto-generated sessions meaningful names that stick, so you know exactly what you were working on.
* **Bulk Operations:** Multi-select sessions using `fzf` and bulk delete them without breaking a sweat.
* **Rich Context Previews:** View project summaries and detailed session information in a sleek side pane while browsing.
* **Cross-Platform Compatibility:** Runs flawlessly on PowerShell 7+ whether you are on Windows, macOS, or Linux.

## 🚀 Installation

### Requirements

Before getting started, make sure you have:
* **[PowerShell 7+](https://learn.microsoft.com/en-us/powershell/scripting/install/installing-powershell)** installed.
* **Codex CLI** available in your `PATH`.
* **[fzf](https://github.com/junegunn/fzf)** installed and available in your `PATH`.

#### Getting `fzf`
* **Windows:** `winget install junegunn.fzf`, `choco install fzf`, or `scoop install fzf`
* **macOS:** `brew install fzf`
* **Linux:** Install via `apt`, `dnf`, `pacman`, or `zypper` depending on your distro.

### Setup

1. **Clone** the repository to your preferred location:
   ```powershell
   git clone https://github.com/vinzify/Codex-Session-Hub.git
   cd Codex-Session-Hub
   ```
2. **Install** the CLI by running the install script:
   ```powershell
   pwsh -File .\install.ps1
   ```
3. **Reload** your shell profile:
   ```powershell
   . $PROFILE
   ```
4. You're ready to go! Run `csx` in your terminal.

*(To uninstall, simply run the `.\uninstall.ps1` script from the project folder).*

## 💻 Usage & Commands

The main command for Codex Session Hub is `csx`.

```powershell
# Open the fzf browser for all sessions
csx
csx browse

# Open the browser filtered by a specific project name
csx browse desktop

# Rename a specific session to a friendly alias
csx rename <session-id> --name "My friendly alias"

# Revert a session's title back to the auto-generated one
csx reset <session-id>

# Delete a session by its ID
csx delete <session-id>

# Run a system check to verify dependencies and paths
csx doctor
```

### ⌨️ Browser Controls (`csx` / `csx browse`)

Once inside the `fzf` interface, you have access to powerful shortcuts:

| Key Binding | Action |
| --- | --- |
| `Enter` | Resume the focused session |
| `Tab` / `Shift-Tab` | Multi-select sessions |
| `Ctrl-D` | Delete all selected sessions |
| `Ctrl-E` | Rename/Alias the focused session |
| `Ctrl-R` | Reset the focused session's title |
| `Esc` / `Ctrl-C` | Exit the browser |

### 🔍 Search Filtering

While in the browser, you can filter your sessions with special syntax:
* **Text query:** Filters by folder/project name (default)
* **Number query:** Filters by session number prefix
* **`title:<term>`** or **`t:<term>`**: Filters specifically by the session title/alias

## ⚙️ Configuration

Codex Session Hub supports customization through optional environment variables:

| Variable | Description |
| --- | --- |
| `CODEX_SESSION_HUB_SESSION_ROOT` | Override the default location where sessions are stored. |
| `CODEX_SESSION_HUB_CONFIG_ROOT` | Override the default location for configuration files. |
| `CODEX_SESSION_HUB_FZF_OPTS` | Add custom options to the `fzf` command. |

## 🛠️ Development

Tests are written using [Pester](https://pester.dev/). To run the test suite:

```powershell
# Run all tests
Invoke-Pester -Path .\tests
```

---
---
**License:** [MIT](LICENSE)

---
💖 **Support the Project**

If Codex Session Hub saves you time and you want to support its continued development, consider sending an ETH donation:
`0xe7043f731a2f36679a676938e021c6B67F80b9A1`
