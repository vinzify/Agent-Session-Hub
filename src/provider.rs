use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ProviderKind {
    Codex,
    Claude,
    Opencode,
}

impl ProviderKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "codex" => Some(Self::Codex),
            "claude" => Some(Self::Claude),
            "opencode" => Some(Self::Opencode),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Opencode => "opencode",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude",
            Self::Opencode => "OpenCode",
        }
    }

    pub fn launcher_name(self) -> &'static str {
        match self {
            Self::Codex => "csx",
            Self::Claude => "clx",
            Self::Opencode => "opx",
        }
    }

    pub fn binary_name(self) -> &'static str {
        self.name()
    }

    pub fn supports_delete(self) -> bool {
        matches!(self, Self::Codex | Self::Opencode)
    }

    pub fn session_root_env(self) -> &'static str {
        match self {
            Self::Codex => "CODEX_SESSION_HUB_SESSION_ROOT",
            Self::Claude => "CODEX_SESSION_HUB_CLAUDE_SESSION_ROOT",
            Self::Opencode => "CODEX_SESSION_HUB_OPENCODE_SESSION_ROOT",
        }
    }

    pub fn index_file_name(self) -> &'static str {
        match self {
            Self::Codex => "index.json",
            Self::Claude => "claude-index.json",
            Self::Opencode => "opencode-index.json",
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
        }
    }

    pub fn alias_from_argv0(argv0: &str) -> Self {
        let normalized = argv0.replace('\\', "/");
        let file_name = normalized.rsplit('/').next().unwrap_or(argv0);
        let lower = file_name.trim_end_matches(".exe").to_ascii_lowercase();
        match lower.as_str() {
            "clx" => Self::Claude,
            "opx" => Self::Opencode,
            _ => Self::Codex,
        }
    }
}
