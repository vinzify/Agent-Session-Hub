//! Bounded, recoverable operations on local CLI history, never project files.
use crate::{
    paths::{config_root, provider_session_root},
    provider::ProviderKind,
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
    process::Command,
    time::SystemTime,
};

pub fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn checked_path(root: &Path, relative: &Path) -> Result<PathBuf> {
    ensure!(
        !relative.as_os_str().is_empty()
            && relative
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "unsafe history path"
    );
    let mut path = root.to_path_buf();
    for component in relative.components() {
        path.push(component);
        match fs::symlink_metadata(&path) {
            Ok(m) => ensure!(
                !m.file_type().is_symlink(),
                "refusing history symlink: {}",
                path.display()
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(path)
}

fn producer(provider: ProviderKind, name: &str, args: &str) -> bool {
    let name = name
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(name)
        .to_ascii_lowercase();
    let args = args.to_ascii_lowercase();
    match provider {
        ProviderKind::Antigravity => {
            name == "agy"
                || name == "agy.exe"
                || name.starts_with("antigravity")
                || name.starts_with("language_server")
        }
        ProviderKind::Claude => {
            name == "claude"
                || name == "claude.exe"
                || name == "claude-code"
                || name == "claude-code.exe"
                || ((name == "node" || name == "node.exe" || name == "bun" || name == "bun.exe")
                    && (args.contains("claude-code")
                        || args.contains("claude_agent_sdk")
                        || args.contains("claude-agent-sdk")))
        }
        _ => false,
    }
}

pub fn ensure_idle(provider: ProviderKind) -> Result<()> {
    if !matches!(provider, ProviderKind::Claude | ProviderKind::Antigravity) {
        return Ok(());
    }
    #[cfg(not(windows))]
    let active = {
        let output = Command::new("ps")
            .args(["-A", "-o", "comm=", "-o", "args="])
            .output()
            .context("cannot verify running history writers; deletion refused")?;
        ensure!(
            output.status.success(),
            "cannot inspect running history writers; deletion refused"
        );
        String::from_utf8_lossy(&output.stdout).lines().any(|line| {
            let line = line.trim();
            let split = line.find(char::is_whitespace).unwrap_or(line.len());
            producer(provider, &line[..split], &line[split..])
        })
    };
    #[cfg(windows)]
    let active = {
        let output = Command::new("powershell.exe").args(["-NoProfile", "-NonInteractive", "-Command", "[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false); Get-CimInstance Win32_Process | Select-Object Name,CommandLine | ConvertTo-Json -Compress"]).output().context("cannot verify running history writers; deletion refused")?;
        ensure!(
            output.status.success(),
            "cannot inspect running history writers; deletion refused"
        );
        let processes: Value = serde_json::from_slice(&output.stdout)?;
        processes
            .as_array()
            .context("unexpected process listing")?
            .iter()
            .any(|p| {
                producer(
                    provider,
                    p["Name"].as_str().unwrap_or(""),
                    p["CommandLine"].as_str().unwrap_or(""),
                )
            })
    };
    ensure!(
        !active,
        "Close {} and its background processes before deleting or restoring local history",
        provider.display_name()
    );
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedFile {
    pub relative: PathBuf,
    pub bytes: u64,
    pub modified: SystemTime,
    pub sha256: String,
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

#[derive(Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub provider: String,
    pub root: PathBuf,
    pub state: String,
    pub files: Vec<SavedFile>,
    pub metadata: Value,
    #[serde(default)]
    pub aliases: std::collections::BTreeMap<String, String>,
}

pub struct Backup {
    pub dir: PathBuf,
    pub manifest: Manifest,
}

pub fn recovery_root(provider: ProviderKind) -> PathBuf {
    config_root().join("recovery").join(provider.name())
}

impl Backup {
    pub fn prepare(
        provider: ProviderKind,
        paths: &[PathBuf],
        metadata: Value,
        ids: &[String],
    ) -> Result<Self> {
        ensure_idle(provider)?;
        let root = provider_session_root(provider).canonicalize()?;
        let mut files = Vec::new();
        let mut seen = HashSet::new();
        for path in paths {
            let relative = path
                .strip_prefix(&root)
                .context("history file is outside its provider store")?
                .to_path_buf();
            let path = checked_path(&root, &relative)?;
            if !seen.insert(relative.clone()) {
                continue;
            }
            let m = fs::symlink_metadata(&path)?;
            ensure!(m.is_file(), "not a regular history file");
            files.push(SavedFile {
                relative,
                bytes: m.len(),
                modified: m.modified()?,
                sha256: hash_file(&path)?,
            });
        }
        ensure!(!files.is_empty(), "no history files selected");
        let destination = recovery_root(provider);
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&destination)?;
        let destination = destination.canonicalize()?;
        ensure!(
            !destination.starts_with(&root),
            "recovery must be outside the provider history store; choose a separate hub configuration directory"
        );
        let temp = tempfile::Builder::new()
            .prefix("backup-")
            .tempdir_in(&destination)?;
        let output = File::create(temp.path().join("history.tar.gz"))?;
        let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
            output,
            flate2::Compression::fast(),
        ));
        for file in &files {
            archive.append_file(
                &file.relative,
                &mut File::open(checked_path(&root, &file.relative)?)?,
            )?;
        }
        archive.into_inner()?.finish()?.sync_all()?;
        let index = crate::config::AliasIndex::load(provider)?;
        let aliases = ids
            .iter()
            .filter_map(|id| {
                let alias = index.get_alias(provider, id);
                (!alias.is_empty()).then(|| (id.clone(), alias))
            })
            .collect();
        let manifest = Manifest {
            version: 1,
            provider: provider.name().to_string(),
            root,
            state: "prepared".to_string(),
            files,
            metadata,
            aliases,
        };
        crate::atomic::write(
            &temp.path().join("manifest.json"),
            &serde_json::to_vec_pretty(&manifest)?,
        )?;
        let backup = Self {
            dir: temp.keep(),
            manifest,
        };
        // The manifest writer flushes the backup directory. Also persist its
        // entry and newly created recovery parents before removing originals.
        #[cfg(unix)]
        for directory in destination.ancestors().take(4) {
            File::open(directory)?.sync_all()?;
        }
        backup.recheck()?;
        Ok(backup)
    }

    pub fn load(provider: ProviderKind, id: &str) -> Result<Self> {
        ensure!(
            valid_component(id) && id.starts_with("backup-"),
            "invalid recovery ID"
        );
        let root = recovery_root(provider).canonicalize()?;
        let dir = checked_path(&root, Path::new(id))?;
        let manifest: Manifest =
            serde_json::from_slice(&fs::read(checked_path(&dir, Path::new("manifest.json"))?)?)?;
        ensure!(
            manifest.version == 1 && manifest.provider == provider.name(),
            "unsupported recovery manifest"
        );
        ensure!(manifest.root.is_absolute(), "invalid recovery root");
        for f in &manifest.files {
            checked_path(&manifest.root, &f.relative)?;
        }
        Ok(Self { dir, manifest })
    }

    pub fn id(&self) -> String {
        self.dir.file_name().unwrap().to_string_lossy().into_owned()
    }
    pub fn mark(&mut self, state: &str) -> Result<()> {
        self.manifest.state = state.to_string();
        crate::atomic::write(
            &self.dir.join("manifest.json"),
            &serde_json::to_vec_pretty(&self.manifest)?,
        )
    }
    pub fn recheck(&self) -> Result<()> {
        for f in &self.manifest.files {
            let path = checked_path(&self.manifest.root, &f.relative)?;
            let m = fs::metadata(&path)?;
            ensure!(
                m.len() == f.bytes && m.modified()? == f.modified && hash_file(&path)? == f.sha256,
                "history changed during review; nothing deleted"
            );
        }
        Ok(())
    }

    pub fn quarantine(&self) -> Result<Quarantine> {
        self.recheck()?;
        let temp = tempfile::Builder::new()
            .prefix(".ash-delete-")
            .tempdir_in(&self.manifest.root)?;
        let mut result = Quarantine {
            temp: Some(temp),
            moved: Vec::new(),
        };
        let moving = (|| -> Result<()> {
            for (index, f) in self.manifest.files.iter().enumerate() {
                let path = checked_path(&self.manifest.root, &f.relative)?;
                let to = result.temp.as_ref().unwrap().path().join(index.to_string());
                fs::rename(&path, &to)?;
                result.moved.push((path, to));
            }
            Ok(())
        })();
        if let Err(error) = moving {
            let rollback = result.rollback();
            return Err(error).with_context(|| {
                format!(
                    "could not quarantine history; rollback: {rollback:?}; recovery {} retained",
                    self.id()
                )
            });
        }
        Ok(result)
    }

    pub fn verify_root(&self) -> Result<()> {
        let provider =
            ProviderKind::parse(&self.manifest.provider).context("invalid recovery provider")?;
        ensure!(
            self.manifest.root == provider_session_root(provider).canonicalize()?,
            "recovery belongs to another session root; restore the original session-root setting first"
        );
        Ok(())
    }

    pub fn restore_files(&self) -> Result<Vec<PathBuf>> {
        let mut created = Vec::new();
        if let Err(error) = self.restore_into(&mut created) {
            let rollback = self.remove_restored(&created);
            return Err(error).with_context(|| {
                format!("restore failed; recovery retained; file rollback: {rollback:?}")
            });
        }
        Ok(created)
    }

    pub fn remove_restored(&self, paths: &[PathBuf]) -> Result<()> {
        let mut failed = Vec::new();
        for path in paths.iter().rev() {
            let expected = self
                .manifest
                .files
                .iter()
                .find(|f| self.manifest.root.join(&f.relative) == *path)
                .context("unrecognised restored file")?;
            if fs::symlink_metadata(path).is_ok_and(|m| m.is_file())
                && hash_file(path)? == expected.sha256
            {
                if let Err(e) = fs::remove_file(path) {
                    failed.push(e.to_string());
                }
            } else {
                failed.push(format!("{} changed; left untouched", path.display()));
            }
        }
        ensure!(failed.is_empty(), "{}", failed.join("; "));
        Ok(())
    }

    fn restore_into(&self, created: &mut Vec<PathBuf>) -> Result<()> {
        self.verify_root()?;
        // Verify the archive before any extraction, including all names and content.
        for apply in [false, true] {
            let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(File::open(
                checked_path(&self.dir, Path::new("history.tar.gz"))?,
            )?));
            let mut seen = HashSet::new();
            for entry in archive.entries()? {
                let mut entry = entry?;
                let relative = entry.path()?.into_owned();
                ensure!(
                    entry.header().entry_type().is_file(),
                    "unexpected recovery entry"
                );
                let expected = self
                    .manifest
                    .files
                    .iter()
                    .find(|f| f.relative == relative)
                    .context("unrecognised recovery entry")?;
                ensure!(
                    seen.insert(relative.clone()) && entry.size() == expected.bytes,
                    "invalid recovery archive"
                );
                let target = checked_path(&self.manifest.root, &relative)?;
                if target.exists() {
                    ensure!(
                        hash_file(&target)? == expected.sha256,
                        "restore would overwrite changed history: {}",
                        target.display()
                    );
                }
                if !apply {
                    let mut hash = Sha256::new();
                    let mut buf = [0; 65536];
                    loop {
                        let n = entry.read(&mut buf)?;
                        if n == 0 {
                            break;
                        }
                        hash.update(&buf[..n]);
                    }
                    ensure!(
                        format!("{:x}", hash.finalize()) == expected.sha256,
                        "damaged recovery archive"
                    );
                } else if !target.exists() {
                    fs::create_dir_all(target.parent().context("missing restore directory")?)?;
                    checked_path(&self.manifest.root, &relative)?;
                    let mut file = tempfile::NamedTempFile::new_in(target.parent().unwrap())?;
                    std::io::copy(&mut entry, &mut file)?;
                    file.as_file().sync_all()?;
                    file.persist_noclobber(&target)
                        .context("restore target appeared; existing history left untouched")?;
                    created.push(target);
                }
            }
            ensure!(
                seen.len() == self.manifest.files.len(),
                "incomplete recovery archive"
            );
        }
        Ok(())
    }
}

pub struct Quarantine {
    temp: Option<tempfile::TempDir>,
    moved: Vec<(PathBuf, PathBuf)>,
}
impl Quarantine {
    pub fn rollback(&mut self) -> Result<()> {
        let mut errors = Vec::new();
        for (from, to) in std::mem::take(&mut self.moved).into_iter().rev() {
            if fs::symlink_metadata(&from).is_ok() {
                errors.push("history appeared during recovery".to_string());
                continue;
            }
            if let Err(error) = fs::rename(&to, &from) {
                errors.push(error.to_string());
            }
        }
        if !errors.is_empty() {
            let retained = self.temp.take().map(|temp| temp.keep());
            bail!(
                "rollback incomplete: {}; quarantined files retained at {retained:?}; use the recovery backup",
                errors.join("; ")
            );
        }
        Ok(())
    }
}

pub fn claude_delete(sessions: &[crate::session::SessionRecord]) -> Result<String> {
    let provider = ProviderKind::Claude;
    ensure_idle(provider)?;
    let configured = provider_session_root(provider);
    let root = configured.canonicalize()?;
    let mut paths = Vec::new();
    for session in sessions {
        let relative = session
            .file_path
            .strip_prefix(&configured)
            .or_else(|_| session.file_path.strip_prefix(&root))?;
        let path = checked_path(&root, relative)?;
        ensure!(
            path.extension().is_some_and(|e| e == "jsonl"),
            "not a Claude transcript"
        );
        ensure!(
            fs::metadata(&path)?.len() == session.storage_bytes,
            "Claude transcript changed during review"
        );
        if let Some(modified) = session.file_modified {
            ensure!(
                fs::metadata(&path)?.modified()? == modified,
                "Claude transcript changed during review"
            );
        }
        paths.push(path.clone());
        if !session.is_agent {
            ensure!(
                path.file_stem().and_then(|s| s.to_str()) == Some(session.session_id.as_str())
                    && valid_component(&session.session_id),
                "Claude session identity mismatch"
            );
            let children = path.with_extension("");
            if children.try_exists()? {
                for entry in walkdir::WalkDir::new(&children) {
                    let entry = entry?;
                    ensure!(
                        !entry.file_type().is_symlink(),
                        "refusing Claude session symlink"
                    );
                    if entry.file_type().is_file() {
                        paths.push(entry.into_path());
                    }
                }
            }
        } else {
            let sidecar = path.with_extension("meta.json");
            if sidecar.try_exists()? {
                paths.push(sidecar);
            }
        }
    }
    let ids = sessions
        .iter()
        .map(|s| s.session_id.clone())
        .collect::<Vec<_>>();
    let mut backup = Backup::prepare(provider, &paths, Value::Null, &ids)?;
    ensure_idle(provider)?;
    let mut quarantine = backup.quarantine()?;
    if let Err(error) = backup.mark("deleted") {
        quarantine.rollback()?;
        return Err(error);
    }
    Ok(backup.id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_rollback_preserves_conflicts_and_retains_quarantined_originals() {
        let temp = tempfile::tempdir().unwrap();
        let files = ["a.jsonl", "b.jsonl"]
            .into_iter()
            .map(|name| {
                let path = temp.path().join(name);
                fs::write(&path, name).unwrap();
                let stat = fs::metadata(&path).unwrap();
                SavedFile {
                    relative: name.into(),
                    bytes: stat.len(),
                    modified: stat.modified().unwrap(),
                    sha256: hash_file(&path).unwrap(),
                }
            })
            .collect();
        let backup = Backup {
            dir: temp.path().join("backup-test"),
            manifest: Manifest {
                version: 1,
                provider: "claude".into(),
                root: temp.path().to_path_buf(),
                state: "prepared".into(),
                files,
                metadata: Value::Null,
                aliases: Default::default(),
            },
        };
        let mut quarantine = backup.quarantine().unwrap();
        let retained = quarantine.temp.as_ref().unwrap().path().to_path_buf();
        fs::write(temp.path().join("b.jsonl"), "new user history").unwrap();
        assert!(quarantine.rollback().is_err());
        drop(quarantine);
        assert_eq!(
            fs::read_to_string(temp.path().join("a.jsonl")).unwrap(),
            "a.jsonl"
        );
        assert_eq!(
            fs::read_to_string(temp.path().join("b.jsonl")).unwrap(),
            "new user history"
        );
        assert_eq!(fs::read_to_string(retained.join("1")).unwrap(), "b.jsonl");
    }
    #[test]
    fn detects_producers_without_matching_shells_or_the_hub() {
        assert!(producer(ProviderKind::Antigravity, "agy", ""));
        assert!(producer(
            ProviderKind::Antigravity,
            "language_server_linux_x64",
            ""
        ));
        assert!(!producer(ProviderKind::Antigravity, "agx", ""));
        assert!(!producer(
            ProviderKind::Claude,
            "bash",
            "claude mention in a script"
        ));
        assert!(producer(
            ProviderKind::Claude,
            "node",
            "/usr/bin/node /npm/claude-code/cli.js"
        ));
    }
    #[test]
    fn rejects_escaping_and_symlink_paths() {
        let temp = tempfile::tempdir().unwrap();
        assert!(checked_path(temp.path(), Path::new("../outside")).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/tmp", temp.path().join("link")).unwrap();
            assert!(checked_path(temp.path(), Path::new("link/other")).is_err());
        }
    }
}
