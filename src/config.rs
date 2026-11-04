use crate::paths::{index_path, legacy_index_path};
use crate::provider::ProviderKind;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionAlias {
    pub alias: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderBucket {
    #[serde(default)]
    pub sessions: BTreeMap<String, SessionAlias>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AliasIndex {
    #[serde(default)]
    pub version: Option<u32>,
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderBucket>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyAliasIndex {
    #[serde(default)]
    sessions: BTreeMap<String, SessionAlias>,
}

impl AliasIndex {
    pub fn load(provider: ProviderKind) -> Result<Self> {
        Self::load_from(
            provider,
            &index_path(provider),
            &legacy_index_path(provider),
        )
    }

    fn read(path: &Path) -> Result<Option<String>> {
        match fs::read_to_string(path) {
            Ok(raw) => Ok(Some(raw)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).with_context(|| {
                format!("read alias index {}; file left unchanged", path.display())
            }),
        }
    }

    fn parse(raw: &str, path: &Path) -> Result<Self> {
        let parsed: Self = serde_json::from_str(raw).with_context(|| format!(
            "invalid alias index {}; file left unchanged. Repair its JSON or restore a backup before retrying", path.display()
        ))?;
        parsed.validate()?;
        Ok(parsed)
    }

    fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            matches!(self.version, None | Some(2)),
            "unsupported alias index version {:?}; file left unchanged. Use a compatible Agent Session Hub version",
            self.version
        );
        Ok(())
    }

    fn load_from(provider: ProviderKind, primary: &Path, legacy: &Path) -> Result<Self> {
        if let Some(raw) = Self::read(primary)? {
            let parsed = Self::parse(&raw, primary)?;
            return Ok(parsed.normalized(provider));
        }

        if let Some(raw) = Self::read(legacy)? {
            let legacy_parsed: LegacyAliasIndex = serde_json::from_str(&raw)
                .with_context(|| format!("invalid legacy alias index {}; file left unchanged. Repair its JSON or restore a backup before retrying", legacy.display()))?;
            let mut index = AliasIndex::default();
            index
                .providers
                .entry(provider.name().to_string())
                .or_default()
                .sessions = legacy_parsed.sessions;
            index.version = Some(2);
            index.save_to(primary)?;
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
        self.save_to(&index_path(provider))
    }

    fn save_to(&self, path: &Path) -> Result<()> {
        self.validate()?;
        // Refuse to overwrite a file corrupted or upgraded since it was loaded.
        if let Some(raw) = Self::read(path)? {
            Self::parse(&raw, path)?;
        }
        let text = serde_json::to_string_pretty(self).context("serialize alias index")?;
        crate::atomic::write(path, text.as_bytes())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_primary_and_future_formats_are_never_overwritten() {
        for raw in [
            "{truncated",
            "",
            "null",
            r#"{"version":3,"providers":{}}"#,
            r#"{"version":2,"providers":{},"future":true}"#,
            r#"{"version":2,"providers":{"codex":{"sessions":{"one":{"alias":4}}}}}"#,
        ] {
            let temp = tempfile::tempdir().unwrap();
            let primary = temp.path().join("index.json");
            fs::write(&primary, raw).unwrap();
            assert!(
                AliasIndex::load_from(ProviderKind::Codex, &primary, &temp.path().join("legacy"))
                    .is_err()
            );
            assert!(AliasIndex::default().save_to(&primary).is_err());
            assert_eq!(fs::read_to_string(primary).unwrap(), raw);
        }
    }

    #[test]
    fn corrupt_legacy_does_not_create_a_primary_file() {
        let temp = tempfile::tempdir().unwrap();
        let primary = temp.path().join("new.json");
        let legacy = temp.path().join("old.json");
        fs::write(&legacy, "{broken").unwrap();
        assert!(AliasIndex::load_from(ProviderKind::Codex, &primary, &legacy).is_err());
        assert!(!primary.exists());
        assert_eq!(fs::read_to_string(legacy).unwrap(), "{broken");
    }

    #[test]
    fn valid_legacy_migrates_without_changing_the_original() {
        let temp = tempfile::tempdir().unwrap();
        let primary = temp.path().join("new.json");
        let legacy = temp.path().join("old.json");
        let raw = r#"{"sessions":{"one":{"alias":"Keep me"}}}"#;
        fs::write(&legacy, raw).unwrap();
        let index = AliasIndex::load_from(ProviderKind::Codex, &primary, &legacy).unwrap();
        assert_eq!(index.get_alias(ProviderKind::Codex, "one"), "Keep me");
        assert_eq!(fs::read_to_string(legacy).unwrap(), raw);
        assert_eq!(
            AliasIndex::parse(&fs::read_to_string(primary.clone()).unwrap(), &primary)
                .unwrap()
                .version,
            Some(2)
        );
    }

    #[test]
    fn save_rechecks_corruption_and_preserves_all_provider_buckets() {
        let temp = tempfile::tempdir().unwrap();
        let primary = temp.path().join("index.json");
        let mut index = AliasIndex::default().normalized(ProviderKind::Codex);
        index.set_alias(ProviderKind::Codex, "one", "First");
        index.set_alias(ProviderKind::Claude, "two", "Second");
        index.save_to(&primary).unwrap();
        index.set_alias(ProviderKind::Codex, "one", "Renamed");
        index.save_to(&primary).unwrap();
        let loaded =
            AliasIndex::load_from(ProviderKind::Codex, &primary, &temp.path().join("missing"))
                .unwrap();
        assert_eq!(loaded.get_alias(ProviderKind::Codex, "one"), "Renamed");
        assert_eq!(loaded.get_alias(ProviderKind::Claude, "two"), "Second");
        fs::write(&primary, "corrupted after load").unwrap();
        assert!(index.save_to(&primary).is_err());
        assert_eq!(fs::read_to_string(primary).unwrap(), "corrupted after load");
    }

    #[test]
    fn failed_atomic_replace_keeps_destination_and_cleans_staging() {
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("directory");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("keep"), "existing data").unwrap();
        assert!(crate::atomic::write(&destination, b"replacement").is_err());
        assert_eq!(
            fs::read_to_string(destination.join("keep")).unwrap(),
            "existing data"
        );
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }
}
