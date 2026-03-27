use crate::provider::ProviderKind;
use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub fn config_root() -> PathBuf {
    if let Ok(value) = env::var("CODEX_SESSION_HUB_CONFIG_ROOT") {
        if !value.trim().is_empty() {
            return PathBuf::from(value);
        }
    }

    if cfg!(windows) {
        if let Some(dir) = dirs::config_dir() {
            return dir.join("AgentSessionHub");
        }
    }

    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("agent-session-hub")
}

pub fn legacy_config_root() -> PathBuf {
    if cfg!(windows) {
        return dirs::config_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
            .join("CodexSessionHub");
    }

    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("codex-session-hub")
}

pub fn provider_session_root(provider: ProviderKind) -> PathBuf {
    if let Ok(value) = env::var(provider.session_root_env()) {
        if !value.trim().is_empty() {
            return PathBuf::from(value);
        }
    }
    provider.default_session_root()
}

pub fn index_path(provider: ProviderKind) -> PathBuf {
    config_root().join(provider.index_file_name())
}

pub fn legacy_index_path(provider: ProviderKind) -> PathBuf {
    legacy_config_root().join(provider.index_file_name())
}

pub fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    Ok(())
}

pub fn install_root() -> PathBuf {
    if cfg!(windows) {
        return dirs::data_local_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
            .join("AgentSessionHub");
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
        .join("share")
        .join("agent-session-hub")
}

pub fn launcher_root() -> PathBuf {
    if cfg!(windows) {
        return install_root().join("bin");
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
        .join("bin")
}

pub fn detect_posix_profile() -> PathBuf {
    if let Ok(shell) = env::var("SHELL") {
        if shell.ends_with("/zsh") {
            return dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".zprofile");
        }
        if shell.ends_with("/bash") {
            return dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".bash_profile");
        }
        if shell.ends_with("/fish") {
            return dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
                .join("fish")
                .join("config.fish");
        }
    }

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let fish = home.join(".config").join("fish").join("config.fish");
    let zsh = home.join(".zprofile");
    let bash = home.join(".bash_profile");
    if fish.exists() {
        return fish;
    }
    if zsh.exists() {
        return zsh;
    }
    if bash.exists() {
        return bash;
    }
    home.join(".profile")
}

pub fn powershell_profile_path() -> PathBuf {
    if let Ok(value) = env::var("PROFILE") {
        if !value.trim().is_empty() {
            return PathBuf::from(value);
        }
    }

    if cfg!(windows) {
        return dirs::document_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
            .join("PowerShell")
            .join("Microsoft.PowerShell_profile.ps1");
    }

    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("powershell")
        .join("Microsoft.PowerShell_profile.ps1")
}

pub fn current_exe() -> Result<PathBuf> {
    std::env::current_exe().context("resolve current executable")
}

pub fn normalize_path(path: &str) -> String {
    let trimmed = path.trim().replace("\\\\?\\", "");
    if trimmed.is_empty() {
        return String::new();
    }
    let candidate = PathBuf::from(&trimmed);
    if let Ok(resolved) = candidate.canonicalize() {
        return resolved
            .to_string_lossy()
            .trim_end_matches(['\\', '/'])
            .to_string();
    }
    candidate
        .to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .to_string()
}
