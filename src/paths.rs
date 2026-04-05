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
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let bashrc = home.join(".bashrc");
    let bash_profile = home.join(".bash_profile");
    let zprofile = home.join(".zprofile");
    let fish_config = home.join(".config").join("fish").join("config.fish");

    if let Ok(shell) = env::var("SHELL") {
        if shell.ends_with("/zsh") {
            return zprofile;
        }
        if shell.ends_with("/bash") {
            return if cfg!(target_os = "macos") {
                bash_profile
            } else {
                bashrc
            };
        }
        if shell.ends_with("/fish") {
            return fish_config;
        }
    }

    if fish_config.exists() {
        return fish_config;
    }
    if zprofile.exists() {
        return zprofile;
    }
    if cfg!(target_os = "macos") {
        if bash_profile.exists() {
            return bash_profile;
        }
        if bashrc.exists() {
            return bashrc;
        }
    } else {
        if bashrc.exists() {
            return bashrc;
        }
        if bash_profile.exists() {
            return bash_profile;
        }
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

#[cfg(test)]
mod tests {
    use super::detect_posix_profile;
    use std::path::PathBuf;

    #[test]
    fn bash_uses_expected_profile_for_platform() {
        let previous = std::env::var_os("SHELL");
        unsafe {
            std::env::set_var("SHELL", "/bin/bash");
        }

        let detected = detect_posix_profile();
        let expected = if cfg!(target_os = "macos") {
            ".bash_profile"
        } else {
            ".bashrc"
        };

        assert_eq!(detected.file_name(), Some(expected.as_ref()));

        match previous {
            Some(value) => unsafe { std::env::set_var("SHELL", value) },
            None => unsafe { std::env::remove_var("SHELL") },
        }
    }

    #[test]
    fn zsh_uses_zprofile() {
        let previous = std::env::var_os("SHELL");
        unsafe {
            std::env::set_var("SHELL", "/bin/zsh");
        }

        let detected = detect_posix_profile();
        assert_eq!(
            detected,
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".zprofile")
        );

        match previous {
            Some(value) => unsafe { std::env::set_var("SHELL", value) },
            None => unsafe { std::env::remove_var("SHELL") },
        }
    }
}
