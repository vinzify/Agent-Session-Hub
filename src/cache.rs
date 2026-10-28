use crate::provider::ProviderKind;
use crate::session::SessionMetadata;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, Metadata};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const CACHE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct FileStamp {
    bytes: u64,
    modified: SystemTime,
}

impl FileStamp {
    fn from_metadata(metadata: &Metadata) -> Option<Self> {
        Some(Self {
            bytes: metadata.len(),
            modified: metadata.modified().ok()?,
        })
    }

    fn for_path(path: &Path) -> Option<Self> {
        Self::from_metadata(&fs::metadata(path).ok()?)
    }
}

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    stamp: FileStamp,
    session: Option<SessionMetadata>,
}

#[derive(Serialize, Deserialize)]
struct CacheData {
    version: u32,
    source_root: PathBuf,
    history_stamp: Option<FileStamp>,
    entries: HashMap<PathBuf, CacheEntry>,
}

pub fn cache_root() -> PathBuf {
    if let Some(root) =
        std::env::var_os("CODEX_SESSION_HUB_CACHE_ROOT").filter(|value| !value.is_empty())
    {
        return root.into();
    }
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("agent-session-hub")
}

pub struct SessionCache {
    path: Option<PathBuf>,
    data: CacheData,
    seen: HashSet<PathBuf>,
    dirty: bool,
}

impl SessionCache {
    pub fn load(provider: ProviderKind, source_root: &Path, history_path: Option<&Path>) -> Self {
        let disabled =
            std::env::var("CODEX_SESSION_HUB_DISABLE_CACHE").is_ok_and(|value| value == "1");
        let path = (!disabled).then(|| cache_root().join(format!("{}.json", provider.name())));
        Self::load_from(path, source_root, history_path)
    }

    fn load_from(path: Option<PathBuf>, source_root: &Path, history_path: Option<&Path>) -> Self {
        let history_stamp = history_path.and_then(FileStamp::for_path);
        let source_root = source_root
            .canonicalize()
            .unwrap_or_else(|_| source_root.to_path_buf());
        let data = path
            .as_ref()
            .and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<CacheData>(&bytes).ok())
            .filter(|data| {
                data.version == CACHE_VERSION
                    && data.source_root == source_root
                    && data.history_stamp == history_stamp
            })
            .unwrap_or_else(|| CacheData {
                version: CACHE_VERSION,
                source_root,
                history_stamp,
                entries: HashMap::new(),
            });
        Self {
            path,
            data,
            seen: HashSet::new(),
            dirty: false,
        }
    }

    pub fn get(&mut self, path: &Path, metadata: &Metadata) -> Option<Option<SessionMetadata>> {
        self.seen.insert(path.to_path_buf());
        let stamp = FileStamp::from_metadata(metadata)?;
        let entry = self.data.entries.get(path)?;
        (entry.stamp == stamp).then(|| entry.session.clone())
    }

    pub fn insert(&mut self, path: &Path, metadata: &Metadata, session: Option<SessionMetadata>) {
        let Some(stamp) = FileStamp::from_metadata(metadata) else {
            return;
        };
        self.data
            .entries
            .insert(path.to_path_buf(), CacheEntry { stamp, session });
        self.dirty = true;
    }

    pub fn save(mut self) {
        let original_len = self.data.entries.len();
        self.data.entries.retain(|path, _| self.seen.contains(path));
        if !self.dirty && self.data.entries.len() == original_len {
            return;
        }
        let Some(path) = self.path else {
            return;
        };
        let Some(parent) = path.parent() else {
            return;
        };
        // A disposable cache must never prevent browsing. NamedTempFile keeps
        // private permissions and atomically replaces the old complete index.
        let save = || -> anyhow::Result<()> {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(parent)?;
            let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
            {
                let mut writer = BufWriter::new(temporary.as_file_mut());
                serde_json::to_writer(&mut writer, &self.data)?;
                std::io::Write::flush(&mut writer)?;
            }
            temporary.persist(&path)?;
            Ok(())
        };
        let _ = save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_invalidates_changed_deleted_and_relocated_files() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("cache.json");
        let source = temp.path().join("session.jsonl");
        fs::write(&source, "incomplete").unwrap();
        let mut cache = SessionCache::load_from(Some(path.clone()), temp.path(), None);
        let metadata = fs::metadata(&source).unwrap();
        assert!(cache.get(&source, &metadata).is_none());
        cache.insert(&source, &metadata, None);
        cache.save();

        let mut cache = SessionCache::load_from(Some(path.clone()), temp.path(), None);
        assert!(matches!(cache.get(&source, &metadata), Some(None)));
        fs::write(&source, "now complete session").unwrap();
        assert!(
            cache
                .get(&source, &fs::metadata(&source).unwrap())
                .is_none()
        );
        let other_root = temp.path().join("other");
        let mut other = SessionCache::load_from(Some(path.clone()), &other_root, None);
        assert!(other.get(&source, &metadata).is_none());

        SessionCache::load_from(Some(path.clone()), temp.path(), None).save();
        let cache = SessionCache::load_from(Some(path), temp.path(), None);
        assert!(cache.data.entries.is_empty());
    }

    #[test]
    fn cache_recovers_from_corruption_and_history_changes() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("cache.json");
        fs::write(&path, "{interrupted write").unwrap();
        let history = temp.path().join("history.jsonl");
        fs::write(&history, "one").unwrap();
        let source = temp.path().join("session.jsonl");
        fs::write(&source, "session").unwrap();
        let metadata = fs::metadata(&source).unwrap();
        let mut cache = SessionCache::load_from(Some(path.clone()), temp.path(), Some(&history));
        assert!(cache.get(&source, &metadata).is_none());
        cache.insert(&source, &metadata, None);
        cache.save();
        fs::write(&history, "a newer history entry").unwrap();
        let mut cache = SessionCache::load_from(Some(path), temp.path(), Some(&history));
        assert!(cache.get(&source, &metadata).is_none());
    }
}
