use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ProviderKind {
    Codex,
    Claude,
    Opencode,
    Antigravity,
}

impl ProviderKind {
    pub const fn all() -> [Self; 4] {
        [Self::Codex, Self::Claude, Self::Opencode, Self::Antigravity]
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "codex" => Some(Self::Codex),
            "claude" => Some(Self::Claude),
            "opencode" => Some(Self::Opencode),
            "antigravity" => Some(Self::Antigravity),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Opencode => "opencode",
            Self::Antigravity => "antigravity",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude",
            Self::Opencode => "OpenCode",
            Self::Antigravity => "Antigravity CLI",
        }
    }

    pub fn launcher_name(self) -> &'static str {
        match self {
            Self::Codex => "csx",
            Self::Claude => "clx",
            Self::Opencode => "opx",
            Self::Antigravity => "agx",
        }
    }

    pub fn binary_name(self) -> &'static str {
        match self {
            Self::Antigravity => "agy",
            _ => self.name(),
        }
    }

    pub fn supports_delete(self) -> bool {
        true
    }

    pub fn session_root_env(self) -> &'static str {
        match self {
            Self::Codex => "CODEX_SESSION_HUB_SESSION_ROOT",
            Self::Claude => "CODEX_SESSION_HUB_CLAUDE_SESSION_ROOT",
            Self::Opencode => "CODEX_SESSION_HUB_OPENCODE_SESSION_ROOT",
            Self::Antigravity => "CODEX_SESSION_HUB_ANTIGRAVITY_SESSION_ROOT",
        }
    }

    pub fn index_file_name(self) -> &'static str {
        match self {
            Self::Codex => "index.json",
            Self::Claude => "claude-index.json",
            Self::Opencode => "opencode-index.json",
            Self::Antigravity => "antigravity-index.json",
        }
    }

    pub fn default_session_root(self) -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        match self {
            Self::Codex => home.join(".codex").join("sessions"),
            Self::Claude => home.join(".claude").join("projects"),
            Self::Opencode => dirs::data_local_dir()
                .unwrap_or_else(|| home.join(".local").join("share"))
                .join("opencode"),
            Self::Antigravity => home.join(".gemini").join("antigravity-cli"),
        }
    }

    pub fn alias_from_argv0(argv0: &str) -> Self {
        let normalized = argv0.replace('\\', "/");
        let file_name = normalized.rsplit('/').next().unwrap_or(argv0);
        let lower = file_name.trim_end_matches(".exe").to_ascii_lowercase();
        match lower.as_str() {
            "clx" => Self::Claude,
            "opx" => Self::Opencode,
            "agx" => Self::Antigravity,
            _ => Self::Codex,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn antigravity_has_separate_provider_launcher_executable_and_alias_storage() {
        let provider = ProviderKind::Antigravity;
        assert!(ProviderKind::all().contains(&provider));
        assert_eq!(ProviderKind::parse("Antigravity"), Some(provider));
        assert_eq!(ProviderKind::alias_from_argv0("/custom/bin/agx"), provider);
        assert_eq!(ProviderKind::alias_from_argv0(r"C:\bin\agx.exe"), provider);
        assert_eq!(provider.binary_name(), "agy");
        assert_eq!(provider.index_file_name(), "antigravity-index.json");
        assert!(provider.supports_delete());
    }
}
