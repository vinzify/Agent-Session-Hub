//! Adapter for Antigravity CLI's local SQLite conversation store.
//! The per-conversation steps are protobuf blobs; use the summary index instead
//! of guessing at that private format or invoking the provider while browsing.
//! Mutations require an idle provider, a known schema and a recovery backup.
use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDateTime, Utc};
use rusqlite::{Connection, OpenFlags};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct Conversation {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub updated: DateTime<Local>,
    pub workspace: String,
    pub path: PathBuf,
    pub storage_bytes: u64,
    pub parent: String,
    pub depth: usize,
    pub agent_name: String,
    pub active: bool,
}

pub fn table_columns(connection: &Connection, table: &str) -> Result<Vec<String>> {
    let mut query = connection.prepare(&format!(
        "PRAGMA table_info(\"{}\")",
        table.replace('"', "\"\"")
    ))?;
    Ok(query
        .query_map([], |r| r.get(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn valid_id(id: &str) -> bool {
    // IDs become filenames and a CLI argument, never paths or options. Permit
    // future non-UUID IDs without accepting separators, dots or shell controls.
    !id.is_empty()
        && id.len() <= 128
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn local_workspace(raw: &str) -> Result<Option<String>> {
    let uris: Vec<String> = serde_json::from_str(raw).context("invalid workspace_uris JSON")?;
    Ok(uris.iter().find_map(|uri| {
        let url = reqwest::Url::parse(uri).ok()?;
        // to_file_path rejects non-file schemes and non-local hosts on Unix,
        // and handles Windows drive letters/UNC paths on Windows.
        let path = url.to_file_path().ok()?;
        path.is_absolute()
            .then(|| path.to_string_lossy().into_owned())
    }))
}

fn parse_time(raw: &str) -> Option<DateTime<Local>> {
    DateTime::parse_from_rfc3339(raw)
        .or_else(|_| DateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S%.f%:z"))
        .map(|time| time.with_timezone(&Local))
        .ok()
        .or_else(|| {
            NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S%.f")
                .ok()
                .map(|time| time.and_utc().with_timezone(&Local))
        })
        .filter(|time| time.timestamp() > 0)
}

fn file_size(path: &Path) -> Result<u64> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.len()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error).with_context(|| format!("stat {}", path.display())),
    }
}

pub fn load(root: &Path) -> Result<Vec<Conversation>> {
    let index = root.join("conversation_summaries.db");
    if !index.try_exists()? {
        return Ok(Vec::new());
    }
    // Do not use immutable=1: it ignores WAL entries from a running CLI.
    let connection = Connection::open_with_flags(&index, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("read Antigravity index {}", index.display()))?;
    connection.busy_timeout(Duration::from_millis(250))?;
    let columns = table_columns(&connection, "conversation_summaries")?;
    let extra = |name: &str, default: &str| {
        if columns.iter().any(|c| c == name) {
            format!("coalesce({name},{default})")
        } else {
            default.to_string()
        }
    };
    let sql=format!(
        "SELECT conversation_id, title, preview, last_modified_time, workspace_uris
         , {}, {}, {}, {} FROM conversation_summaries ORDER BY last_modified_time DESC, conversation_id",
         extra("parent_conversation_id","''"),extra("nesting_depth","0"),extra("agent_name","''"),extra("not_fully_idle","0"));
    let mut statement = connection.prepare(&sql).context("unsupported Antigravity CLI index schema; expected a local conversation_summaries SQLite index")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, i64>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, bool>(8)?,
        ))
    })?;
    let mut conversations = Vec::new();
    for row in rows {
        let (id, title, preview, modified, workspaces, parent, depth, agent_name, active) = row?;
        if !valid_id(&id) {
            continue;
        }
        let path = root.join("conversations").join(format!("{id}.db"));
        let metadata = match fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() => metadata,
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error).context("stat Antigravity conversation"),
        };
        // The index can also contain old IDE-only and remote histories. Do not
        // present those as locally resumable CLI sessions.
        let Some(workspace) = local_workspace(&workspaces)
            .with_context(|| format!("read Antigravity workspace for {id}"))?
        else {
            continue;
        };
        let updated = parse_time(&modified).unwrap_or_else(|| {
            DateTime::<Utc>::from(metadata.modified().unwrap_or(std::time::UNIX_EPOCH))
                .with_timezone(&Local)
        });
        let storage_bytes = metadata
            .len()
            .saturating_add(file_size(&path.with_file_name(format!("{id}.db-wal")))?)
            .saturating_add(file_size(&path.with_file_name(format!("{id}.db-shm")))?);
        conversations.push(Conversation {
            id,
            title,
            preview,
            updated,
            workspace,
            path,
            storage_bytes,
            parent,
            depth: usize::try_from(depth).unwrap_or(0),
            agent_name,
            active,
        });
    }
    conversations.sort_by(|a, b| b.updated.cmp(&a.updated).then_with(|| a.id.cmp(&b.id)));
    Ok(conversations)
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
enum StoredValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}
impl From<rusqlite::types::Value> for StoredValue {
    fn from(v: rusqlite::types::Value) -> Self {
        use rusqlite::types::Value as V;
        match v {
            V::Null => Self::Null,
            V::Integer(n) => Self::Integer(n),
            V::Real(n) => Self::Real(n),
            V::Text(s) => Self::Text(s),
            V::Blob(b) => Self::Blob(b),
        }
    }
}
impl From<&StoredValue> for rusqlite::types::Value {
    fn from(v: &StoredValue) -> Self {
        use StoredValue as V;
        match v {
            V::Null => Self::Null,
            V::Integer(n) => Self::Integer(*n),
            V::Real(n) => Self::Real(*n),
            V::Text(s) => Self::Text(s.clone()),
            V::Blob(b) => Self::Blob(b.clone()),
        }
    }
}
#[derive(serde::Serialize, serde::Deserialize)]
struct StoredRows {
    table: String,
    columns: Vec<String>,
    rows: Vec<Vec<StoredValue>>,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct CacheRows {
    name: String,
    entries: std::collections::BTreeMap<String, serde_json::Value>,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct DeletedMetadata {
    ids: Vec<String>,
    tables: Vec<StoredRows>,
    caches: Vec<CacheRows>,
}
struct CacheEdit {
    path: PathBuf,
    before: Option<Vec<u8>>,
    after: Vec<u8>,
}

fn writable_index(root: &Path) -> Result<Connection> {
    let path = crate::local_store::checked_path(root, Path::new("conversation_summaries.db"))?;
    for suffix in ["-wal", "-shm", "-journal"] {
        crate::local_store::checked_path(
            root,
            Path::new(&format!("conversation_summaries.db{suffix}")),
        )?;
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    conn.busy_timeout(Duration::from_millis(250))?;
    let mut statement = conn
        .prepare("SELECT name,type FROM sqlite_master WHERE type IN ('table','view','trigger')")?;
    for row in statement.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (name, kind) = row?;
        anyhow::ensure!(
            kind == "table"
                && (name == "conversation_summaries"
                    || name == "search_schema_version"
                    || name == "conversations_fts"
                    || name.starts_with("conversations_fts_")
                    || name.starts_with("sqlite_")),
            "unrecognised Antigravity index schema; no history changed"
        );
    }
    drop(statement);
    Ok(conn)
}

fn placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(",")
}
fn cache_object<'a>(
    value: &'a mut serde_json::Value,
    name: &str,
) -> Result<&'a mut serde_json::Map<String, serde_json::Value>> {
    let v = if name == "conversation_metadata.json" {
        value
            .get_mut("conversations")
            .context("unsupported Antigravity conversation cache")?
    } else {
        value
    };
    v.as_object_mut()
        .context("invalid Antigravity cache; file left untouched")
}
fn cache_edits(root: &Path, caches: &[CacheRows], restore: bool) -> Result<Vec<CacheEdit>> {
    let mut edits = Vec::new();
    for cache in caches {
        anyhow::ensure!(
            matches!(
                cache.name.as_str(),
                "conversation_metadata.json" | "last_conversations.json"
            ),
            "unsupported recovery cache"
        );
        let path = crate::local_store::checked_path(root, &Path::new("cache").join(&cache.name))?;
        let before = match fs::read(&path) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        let mut value = if let Some(bytes) = &before {
            serde_json::from_slice(bytes)?
        } else if cache.name == "conversation_metadata.json" {
            serde_json::json!({"conversations":{}})
        } else {
            serde_json::json!({})
        };
        let map = cache_object(&mut value, &cache.name)?;
        for (key, old) in &cache.entries {
            if restore {
                if let Some(current) = map.get(key) {
                    anyhow::ensure!(
                        current == old,
                        "restore would overwrite newer Antigravity cache data"
                    );
                } else {
                    map.insert(key.clone(), old.clone());
                }
            } else {
                anyhow::ensure!(
                    map.get(key) == Some(old),
                    "Antigravity cache changed during review"
                );
                map.remove(key);
            }
        }
        edits.push(CacheEdit {
            path,
            before,
            after: serde_json::to_vec_pretty(&value)?,
        });
    }
    Ok(edits)
}
fn apply_caches(edits: &[CacheEdit]) -> Result<()> {
    for CacheEdit {
        path,
        before,
        after,
    } in edits
    {
        anyhow::ensure!(
            fs::read(path).ok().as_ref() == before.as_ref(),
            "Antigravity cache changed; operation cancelled"
        );
        crate::atomic::write(path, after)?;
    }
    Ok(())
}
fn rollback_caches(edits: &[CacheEdit]) -> Result<()> {
    let mut errors = Vec::new();
    for CacheEdit {
        path,
        before,
        after,
    } in edits.iter().rev()
    {
        let current = fs::read(path).ok();
        if current.as_ref() == before.as_ref() {
            continue;
        }
        if current.as_ref() != Some(after) {
            errors.push(format!("{} changed during rollback", path.display()));
            continue;
        }
        let result = if let Some(before) = before {
            crate::atomic::write(path, before)
        } else {
            fs::remove_file(path).map_err(anyhow::Error::from)
        };
        if let Err(e) = result {
            errors.push(format!("{}: {e:#}", path.display()));
        }
    }
    anyhow::ensure!(
        errors.is_empty(),
        "cache recovery required: {}",
        errors.join("; ")
    );
    Ok(())
}

pub fn delete(sessions: &[crate::session::SessionRecord]) -> Result<String> {
    use crate::{
        local_store::{self, Backup},
        provider::ProviderKind,
    };
    let provider = ProviderKind::Antigravity;
    local_store::ensure_idle(provider)?;
    let root = crate::paths::provider_session_root(provider).canonicalize()?;
    let ids = sessions
        .iter()
        .map(|s| s.session_id.clone())
        .collect::<Vec<_>>();
    anyhow::ensure!(
        !ids.is_empty() && ids.iter().all(|id| valid_id(id)),
        "invalid Antigravity selection"
    );
    let mut conn = writable_index(&root)?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .context("Antigravity index is busy; close Antigravity first")?;
    // The reserved write lock prevents a metadata writer from changing activity
    // or lineage between this check and our scoped deletes.
    let live = load(&root)?;
    for session in sessions {
        let current = live
            .iter()
            .find(|c| c.id == session.session_id)
            .context("Antigravity conversation disappeared")?;
        anyhow::ensure!(
            !current.active
                && current.storage_bytes == session.storage_bytes
                && current.updated == session.last_updated
                && session
                    .file_modified
                    .is_none_or(|modified| fs::metadata(&current.path)
                        .and_then(|m| m.modified())
                        .is_ok_and(|now| now == modified)),
            "Antigravity conversation is active or changed; no history deleted"
        );
    }
    let mut tables = Vec::new();
    for table in ["conversation_summaries", "conversations_fts"] {
        let columns = table_columns(&tx, table)?;
        if columns.is_empty() {
            continue;
        }
        let mut query = tx.prepare(&format!(
            "SELECT * FROM {table} WHERE conversation_id IN ({})",
            placeholders(ids.len())
        ))?;
        let mut rows = query.query(rusqlite::params_from_iter(&ids))?;
        let mut data = Vec::new();
        while let Some(row) = rows.next()? {
            data.push(
                (0..columns.len())
                    .map(|i| {
                        row.get::<_, rusqlite::types::Value>(i)
                            .map(StoredValue::from)
                    })
                    .collect::<rusqlite::Result<Vec<_>>>()?,
            );
        }
        if table == "conversation_summaries" {
            anyhow::ensure!(data.len() == ids.len(), "Antigravity selection changed");
        }
        tables.push(StoredRows {
            table: table.to_string(),
            columns,
            rows: data,
        });
    }
    let summary_columns = table_columns(&tx, "conversation_summaries")?;
    if summary_columns
        .iter()
        .any(|c| c == "parent_conversation_id")
    {
        let nested = if summary_columns.iter().any(|c| c == "nesting_depth") {
            " AND nesting_depth > 0"
        } else {
            ""
        };
        let mut query=tx.prepare(&format!("SELECT conversation_id FROM conversation_summaries WHERE parent_conversation_id IN ({}){nested}",placeholders(ids.len())))?;
        for child in query.query_map(rusqlite::params_from_iter(&ids), |r| r.get::<_, String>(0))? {
            anyhow::ensure!(
                ids.contains(&child?),
                "select the connected child conversations too; no history deleted"
            );
        }
    }
    let mut caches = Vec::new();
    if root.join("cache").try_exists()? {
        for entry in fs::read_dir(root.join("cache"))? {
            let name = entry?.file_name().to_string_lossy().into_owned();
            anyhow::ensure!(
                !(name.contains("conversation") && name.ends_with(".json"))
                    || matches!(
                        name.as_str(),
                        "conversation_metadata.json" | "last_conversations.json"
                    ),
                "unrecognised Antigravity conversation cache; no history changed"
            );
        }
    }
    for name in ["conversation_metadata.json", "last_conversations.json"] {
        let path = local_store::checked_path(&root, &Path::new("cache").join(name))?;
        if !path.try_exists()? {
            continue;
        }
        let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
        let map = cache_object(&mut value, name)?;
        let mut entries = std::collections::BTreeMap::new();
        for (key, value) in map.iter() {
            if (name == "conversation_metadata.json" && ids.contains(key))
                || (name == "last_conversations.json"
                    && value.as_str().is_some_and(|v| ids.iter().any(|id| id == v)))
            {
                anyhow::ensure!(
                    value
                        .pointer("/summary/Cloud")
                        .and_then(serde_json::Value::as_bool)
                        != Some(true),
                    "remote/cloud conversations must be deleted through Antigravity"
                );
                entries.insert(key.clone(), value.clone());
            }
        }
        if !entries.is_empty() {
            caches.push(CacheRows {
                name: name.to_string(),
                entries,
            });
        }
    }
    let metadata = DeletedMetadata {
        ids: ids.clone(),
        tables,
        caches,
    };
    let edits = cache_edits(&root, &metadata.caches, false)?;
    let mut paths = Vec::new();
    for id in &ids {
        for suffix in [".db", ".db-wal", ".db-shm"] {
            let p = local_store::checked_path(
                &root,
                &Path::new("conversations").join(format!("{id}{suffix}")),
            )?;
            if p.try_exists()? {
                paths.push(p);
            }
        }
    }
    let mut backup = Backup::prepare(provider, &paths, serde_json::to_value(&metadata)?, &ids)?;
    local_store::ensure_idle(provider)?;
    let mut quarantine = backup.quarantine()?;
    let result = (|| -> Result<()> {
        for table in &metadata.tables {
            tx.execute(
                &format!(
                    "DELETE FROM {} WHERE conversation_id IN ({})",
                    table.table,
                    placeholders(ids.len())
                ),
                rusqlite::params_from_iter(&ids),
            )?;
        }
        apply_caches(&edits)?;
        tx.commit()?;
        Ok(())
    })();
    if let Err(error) = result {
        let cache_result = rollback_caches(&edits);
        let file_result = quarantine.rollback();
        anyhow::ensure!(
            cache_result.is_ok() && file_result.is_ok(),
            "deletion failed: {error:#}; recovery {} retained; cache rollback: {cache_result:?}; file rollback: {file_result:?}",
            backup.id()
        );
        return Err(error).with_context(|| {
            format!(
                "Antigravity deletion rolled back; recovery {} retained",
                backup.id()
            )
        });
    }
    if let Err(error) = backup.mark("deleted") {
        eprintln!(
            "History removed; recovery {} retained but its status could not be updated: {error:#}",
            backup.id()
        );
    }
    Ok(backup.id())
}

pub fn restore(backup: &crate::local_store::Backup) -> Result<()> {
    crate::local_store::ensure_idle(crate::provider::ProviderKind::Antigravity)?;
    backup.verify_root()?;
    let metadata: DeletedMetadata = serde_json::from_value(backup.manifest.metadata.clone())?;
    anyhow::ensure!(
        !metadata.ids.is_empty()
            && metadata.ids.iter().all(|id| valid_id(id))
            && metadata
                .ids
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                == metadata.ids.len(),
        "invalid recovery session IDs"
    );
    let mut seen_caches = std::collections::HashSet::new();
    for cache in &metadata.caches {
        anyhow::ensure!(seen_caches.insert(&cache.name), "duplicate recovery cache");
        for (key, value) in &cache.entries {
            anyhow::ensure!(
                match cache.name.as_str() {
                    "conversation_metadata.json" => metadata.ids.contains(key),
                    "last_conversations.json" => value
                        .as_str()
                        .is_some_and(|id| metadata.ids.iter().any(|selected| selected == id)),
                    _ => false,
                },
                "unexpected recovery cache entry"
            );
        }
    }
    let mut conn = writable_index(&backup.manifest.root)?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let mut already_present = std::collections::HashSet::new();
    let mut seen_tables = std::collections::HashSet::new();
    for table in &metadata.tables {
        anyhow::ensure!(
            matches!(
                table.table.as_str(),
                "conversation_summaries" | "conversations_fts"
            ),
            "invalid recovery table"
        );
        anyhow::ensure!(seen_tables.insert(&table.table), "duplicate recovery table");
        anyhow::ensure!(
            table_columns(&tx, &table.table)? == table.columns,
            "Antigravity schema changed; backup retained"
        );
        let id_column = table
            .columns
            .iter()
            .position(|c| c == "conversation_id")
            .context("missing recovery conversation ID column")?;
        for row in &table.rows {
            anyhow::ensure!(
                row.len() == table.columns.len()
                    && matches!(&row[id_column], StoredValue::Text(id) if metadata.ids.contains(id)),
                "recovery contains unexpected conversation data"
            );
        }
        if table.table == "conversation_summaries" {
            let ids = table
                .rows
                .iter()
                .map(|row| match &row[id_column] {
                    StoredValue::Text(id) => id,
                    _ => unreachable!(), // validated above
                })
                .collect::<std::collections::HashSet<_>>();
            anyhow::ensure!(
                table.rows.len() == metadata.ids.len() && ids.len() == metadata.ids.len(),
                "incomplete recovery summary"
            );
        }
        let existing: i64 = tx.query_row(
            &format!(
                "SELECT count(*) FROM {} WHERE conversation_id IN ({})",
                table.table,
                placeholders(metadata.ids.len())
            ),
            rusqlite::params_from_iter(&metadata.ids),
            |r| r.get(0),
        )?;
        if existing > 0 {
            let mut statement = tx.prepare(&format!(
                "SELECT * FROM {} WHERE conversation_id IN ({})",
                table.table,
                placeholders(metadata.ids.len())
            ))?;
            let current = statement
                .query_map(rusqlite::params_from_iter(&metadata.ids), |row| {
                    (0..table.columns.len())
                        .map(|i| {
                            row.get::<_, rusqlite::types::Value>(i)
                                .map(StoredValue::from)
                        })
                        .collect::<rusqlite::Result<Vec<_>>>()
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let mut old = table
                .rows
                .iter()
                .map(serde_json::to_string)
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let mut now = current
                .iter()
                .map(serde_json::to_string)
                .collect::<std::result::Result<Vec<_>, _>>()?;
            old.sort();
            now.sort();
            anyhow::ensure!(
                old == now,
                "Antigravity contains newer history; refusing to overwrite it"
            );
            already_present.insert(table.table.clone());
        }
    }
    anyhow::ensure!(
        seen_tables.contains(&"conversation_summaries".to_string()),
        "missing recovery summary table"
    );
    let edits = cache_edits(&backup.manifest.root, &metadata.caches, true)?;
    let restored = backup.restore_files()?;
    let result = (|| -> Result<()> {
        for table in &metadata.tables {
            if already_present.contains(&table.table) {
                continue;
            }
            let columns = table
                .columns
                .iter()
                .map(|c| format!("\"{}\"", c.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(",");
            for row in &table.rows {
                tx.execute(
                    &format!(
                        "INSERT INTO {} ({columns}) VALUES ({})",
                        table.table,
                        placeholders(row.len())
                    ),
                    rusqlite::params_from_iter(row.iter().map(rusqlite::types::Value::from)),
                )?;
            }
        }
        apply_caches(&edits)?;
        tx.commit()?;
        Ok(())
    })();
    if let Err(error) = result {
        let caches = rollback_caches(&edits);
        let files = backup.remove_restored(&restored);
        return Err(error).with_context(||format!("metadata restoration failed; backup retained; cache rollback: {caches:?}; file rollback: {files:?}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_rollback_attempts_every_file_and_preserves_concurrent_changes() {
        let temp = tempfile::tempdir().unwrap();
        let first = temp.path().join("first.json");
        let second = temp.path().join("second.json");
        fs::write(&first, b"before-first").unwrap();
        fs::write(&second, b"before-second").unwrap();
        let edits = vec![
            CacheEdit {
                path: first.clone(),
                before: Some(b"before-first".to_vec()),
                after: b"after-first".to_vec(),
            },
            CacheEdit {
                path: second.clone(),
                before: Some(b"before-second".to_vec()),
                after: b"after-second".to_vec(),
            },
        ];
        apply_caches(&edits).unwrap();
        fs::write(&second, b"concurrent-user-change").unwrap();
        assert!(rollback_caches(&edits).is_err());
        assert_eq!(fs::read(&first).unwrap(), b"before-first");
        assert_eq!(fs::read(&second).unwrap(), b"concurrent-user-change");
    }

    #[test]
    fn ids_cannot_escape_the_store_or_become_options() {
        for id in [
            "../secret",
            "/tmp/chat",
            "..",
            "--help",
            "a/b",
            "a\\b",
            "a\n",
            "",
        ] {
            assert!(!valid_id(id), "{id:?}");
        }
        assert!(valid_id("7fa67383-6f32-4f9e-b4c1-27470d446060"));
    }

    #[test]
    fn decodes_local_file_uris_and_rejects_remote_workspaces() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("Sicilia café #1");
        let uri = reqwest::Url::from_directory_path(&workspace).unwrap();
        let raw =
            serde_json::to_string(&vec!["https://example.com/project", uri.as_str()]).unwrap();
        assert_eq!(
            PathBuf::from(local_workspace(&raw).unwrap().unwrap()),
            workspace
        );
        assert!(
            local_workspace("[\"ssh://host/project\"]")
                .unwrap()
                .is_none()
        );
        assert!(local_workspace("not json").is_err());
    }

    #[test]
    fn reads_go_sqlite_and_rfc3339_timestamps_without_losing_nanoseconds() {
        let sql = parse_time("2026-11-07 10:43:48.388633693+00:00").unwrap();
        assert_eq!(Some(sql), parse_time("2026-11-07T10:43:48.388633693Z"));
        assert_eq!(Some(sql), parse_time("2026-11-07 10:43:48.388633693"));
        assert!(parse_time("0001-01-01 00:00:00+00:00").is_none());
        assert!(parse_time("broken").is_none());
    }

    #[test]
    fn reads_live_index_wal_skips_stale_entries_and_refreshes_file_sizes() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir(root.join("conversations")).unwrap();
        let index = root.join("conversation_summaries.db");
        let writer = Connection::open(&index).unwrap();
        writer
            .execute_batch(
                "PRAGMA journal_mode=WAL;
            CREATE TABLE conversation_summaries (conversation_id TEXT, title TEXT,
                preview TEXT, last_modified_time TEXT, workspace_uris TEXT);",
            )
            .unwrap();
        let workspace = reqwest::Url::from_directory_path(root.join("project")).unwrap();
        let uris = serde_json::to_string(&vec![workspace.as_str()]).unwrap();
        for (id, modified) in [
            ("live", "2026-11-07T10:00:00Z"),
            ("older", "2026-11-06T10:00:00Z"),
            ("stale", "2026-11-07T11:00:00Z"),
            ("../escape", "2026-11-07T12:00:00Z"),
        ] {
            writer
                .execute(
                    "INSERT INTO conversation_summaries VALUES (?1,'Title','Preview',?2,?3)",
                    rusqlite::params![id, modified, uris],
                )
                .unwrap();
        }
        // Opaque step databases are not opened or decoded by the hub.
        fs::write(root.join("conversations/live.db"), [0u8; 1024]).unwrap();
        fs::write(root.join("conversations/live.db-wal"), [0u8; 512]).unwrap();
        fs::write(root.join("conversations/live.db-shm"), [0u8; 128]).unwrap();
        fs::write(root.join("conversations/older.db"), [0u8; 256]).unwrap();
        fs::write(root.join("escape.db"), [0u8; 128]).unwrap();
        let before = fs::read(&index).unwrap();
        let wal_before = fs::read(root.join("conversation_summaries.db-wal")).unwrap();
        let loaded = load(root).unwrap();
        assert_eq!(
            loaded.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            ["live", "older"]
        );
        assert_eq!(loaded[0].storage_bytes, 1024 + 512 + 128);
        assert_eq!(loaded[0].preview, "Preview");
        assert_eq!(fs::read(&index).unwrap(), before);
        assert_eq!(
            fs::read(root.join("conversation_summaries.db-wal")).unwrap(),
            wal_before
        );
        fs::write(root.join("conversations/live.db-wal"), [0u8; 2048]).unwrap();
        assert_eq!(load(root).unwrap()[0].storage_bytes, 1024 + 2048 + 128);
        writer.execute("UPDATE conversation_summaries SET last_modified_time='2026-11-08 10:00:00+00:00' WHERE conversation_id='older'", []).unwrap();
        assert_eq!(load(root).unwrap()[0].id, "older");
        fs::remove_file(root.join("conversations/live.db")).unwrap();
        assert_eq!(load(root).unwrap().len(), 1);
    }

    #[test]
    fn missing_index_is_empty_but_damaged_or_unsupported_indexes_fail() {
        let temp = tempfile::tempdir().unwrap();
        assert!(load(temp.path()).unwrap().is_empty());
        let index = temp.path().join("conversation_summaries.db");
        fs::write(&index, b"damaged database").unwrap();
        assert!(load(temp.path()).is_err());
        assert_eq!(fs::read(&index).unwrap(), b"damaged database");
        fs::remove_file(&index).unwrap();
        Connection::open(&index)
            .unwrap()
            .execute_batch("CREATE TABLE unsupported (id TEXT)")
            .unwrap();
        assert!(
            load(temp.path())
                .err()
                .unwrap()
                .to_string()
                .contains("unsupported Antigravity CLI index schema")
        );
    }
}
