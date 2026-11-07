//! Read-only adapter for Antigravity CLI's local SQLite conversation store.
//! The per-conversation steps are protobuf blobs; use the summary index instead
//! of guessing at that private format or invoking the provider while browsing.
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
    let mut statement = connection.prepare(
        "SELECT conversation_id, title, preview, last_modified_time, workspace_uris
         FROM conversation_summaries ORDER BY last_modified_time DESC, conversation_id",
    ).context("unsupported Antigravity CLI index schema; expected a local conversation_summaries SQLite index")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;
    let mut conversations = Vec::new();
    for row in rows {
        let (id, title, preview, modified, workspaces) = row?;
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
        });
    }
    conversations.sort_by(|a, b| b.updated.cmp(&a.updated).then_with(|| a.id.cmp(&b.id)));
    Ok(conversations)
}

#[cfg(test)]
mod tests {
    use super::*;

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
