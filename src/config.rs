use crate::paths::{ensure_parent, index_path, legacy_index_path};
use crate::provider::ProviderKind;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionAlias {
    pub alias: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProviderBucket {
    #[serde(default)]
    pub sessions: BTreeMap<String, SessionAlias>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AliasIndex {
    #[serde(default)]
    pub version: Option<u32>,
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderBucket>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct LegacyAliasIndex {
    #[serde(default)]
    sessions: BTreeMap<String, SessionAlias>,
}

impl AliasIndex {
    pub fn load(provider: ProviderKind) -> Result<Self> {
        let primary = index_path(provider);
        let legacy = legacy_index_path(provider);

        if primary.exists() {
            let raw = fs::read_to_string(&primary)
                .with_context(|| format!("read {}", primary.display()))?;
            let parsed: AliasIndex = serde_json::from_str(&raw).unwrap_or_default();
            return Ok(parsed.normalized(provider));
        }

        if legacy.exists() {
            let raw = fs::read_to_string(&legacy)
                .with_context(|| format!("read {}", legacy.display()))?;
            let legacy_parsed: LegacyAliasIndex = serde_json::from_str(&raw).unwrap_or_default();
            let mut index = AliasIndex::default();
            index
                .providers
                .entry(provider.name().to_string())
                .or_default()
                .sessions = legacy_parsed.sessions;
            index.version = Some(2);
            index.save(provider)?;
            return Ok(index.normalized(provider));
        }

        Ok(Self::default().normalized(provider))
    }

    fn normalized(mut self, provider: ProviderKind) -> Self {
        self.version.get_or_insert(2);
        self.providers
            .entry(provider.name().to_string())
            .or_default();
        self
    }

    pub fn save(&self, provider: ProviderKind) -> Result<()> {
        let path = index_path(provider);
        ensure_parent(&path)?;
        let text = serde_json::to_string_pretty(self).context("serialize alias index")?;
        fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    pub fn get_alias(&self, provider: ProviderKind, session_id: &str) -> String {
        self.providers
            .get(provider.name())
            .and_then(|bucket| bucket.sessions.get(session_id))
            .map(|entry| entry.alias.clone())
            .unwrap_or_default()
    }

    pub fn set_alias(&mut self, provider: ProviderKind, session_id: &str, alias: &str) {
        let bucket = self
            .providers
            .entry(provider.name().to_string())
            .or_default();
        if alias.trim().is_empty() {
            bucket.sessions.remove(session_id);
        } else {
            bucket.sessions.insert(
                session_id.to_string(),
                SessionAlias {
                    alias: alias.trim().to_string(),
                },
            );
        }
    }

    pub fn remove_alias(&mut self, provider: ProviderKind, session_id: &str) {
        let bucket = self
            .providers
            .entry(provider.name().to_string())
            .or_default();
        bucket.sessions.remove(session_id);
    }
}
