use crate::config::AliasIndex;
use crate::formatting::{compress_text, format_relative_age, format_timestamp, project_name};
use crate::git::{GitContext, apply_recorded_branch, get_git_context, workspace_label};
use crate::paths::provider_session_root;
use crate::provider::ProviderKind;
use anyhow::{Context, Result};
use chrono::{DateTime, Local, TimeZone, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct SessionRecord {
    pub provider: ProviderKind,
    pub provider_label: String,
    pub supports_delete: bool,
    pub session_id: String,
    pub timestamp: DateTime<Local>,
    pub timestamp_text: String,
    pub last_updated: DateTime<Local>,
    pub last_updated_text: String,
    pub last_updated_age: String,
    pub project_path: String,
    pub project_key: String,
    pub project_name: String,
    pub repo_root: String,
    pub repo_name: String,
    pub branch_name: String,
    pub branch_display: String,
    pub is_detached_head: bool,
    pub workspace_key: String,
    pub workspace_label: String,
    pub file_path: PathBuf,
    pub project_exists: bool,
    pub alias: String,
    pub preview: String,
    pub display_title: String,
    pub slug: String,
}

#[derive(Clone, Debug)]
pub struct DisplaySession {
    pub session: SessionRecord,
    pub display_number: usize,
    pub group_key: String,
}

fn meaningful_user_text(text: &str) -> bool {
    let clean = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.len() < 12 {
        return false;
    }
    let lower = clean.to_ascii_lowercase();
    if lower.starts_with("<environment_context>")
        || lower.starts_with("# agents.md")
        || lower.starts_with("agents.md")
    {
        return false;
    }
    !clean.trim_start().starts_with('<')
}

fn obj<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.as_object()?.get(key)
}

fn strv(value: &Value, key: &str) -> String {
    obj(value, key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn codex_preview_candidate(entry: &Value) -> String {
    let entry_type = strv(entry, "type");
    let payload = obj(entry, "payload").cloned().unwrap_or(Value::Null);

    if entry_type == "event_msg" && strv(&payload, "type") == "user_message" {
        return strv(&payload, "message");
    }

    if entry_type == "response_item"
        && strv(&payload, "type") == "message"
        && strv(&payload, "role") == "user"
    {
        if let Some(content) = obj(&payload, "content").and_then(Value::as_array) {
            for item in content {
                if strv(item, "type") == "input_text" {
                    let text = strv(item, "text");
                    if !text.is_empty() {
                        return text;
                    }
                }
            }
        }
    }

    String::new()
}

fn claude_message_content(content: &Value) -> String {
    if let Some(text) = content.as_str() {
        return text.to_string();
    }
    if let Some(items) = content.as_array() {
        for item in items {
            let item_type = strv(item, "type");
            if item_type == "text" || item_type == "input_text" {
                let text = strv(item, "text");
                if !text.is_empty() {
                    return text;
                }
            }
        }
    }
    String::new()
}

fn claude_preview_candidate(entry: &Value) -> String {
    if strv(entry, "type") == "user" {
        if let Some(message) = obj(entry, "message") {
            if let Some(content) = obj(message, "content") {
                return claude_message_content(content);
            }
        }
    }
    String::new()
}

fn parse_rfc3339_local(value: &str) -> Option<DateTime<Local>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Local))
}

fn local_from_system_time(metadata: &fs::Metadata) -> DateTime<Local> {
    metadata
        .modified()
        .ok()
        .map(DateTime::<Local>::from)
        .unwrap_or_else(Local::now)
}

fn new_session_record(
    provider: ProviderKind,
    session_id: String,
    timestamp: DateTime<Local>,
    last_updated: DateTime<Local>,
    project_path: String,
    preview: String,
    alias: String,
    file_path: PathBuf,
    recorded_branch_name: String,
    recorded_detached_head: bool,
    slug: String,
    git_context_cache: &mut HashMap<String, GitContext>,
) -> SessionRecord {
    let normalized_project_path = crate::paths::normalize_path(&project_path);
    let project_name_value = project_name(&normalized_project_path);
    let detected_git = get_git_context(&normalized_project_path, git_context_cache);
    let git_context = apply_recorded_branch(
        &detected_git,
        &normalized_project_path,
        &recorded_branch_name,
        recorded_detached_head,
    );
    let preview_text = compress_text(&preview, 160);
    let display_title = if !alias.trim().is_empty() {
        alias.clone()
    } else if !preview_text.is_empty() {
        preview_text.clone()
    } else if !slug.trim().is_empty() {
        slug.clone()
    } else {
        format!("{} session {}", provider.display_name(), session_id)
    };

    SessionRecord {
        provider,
        provider_label: provider.display_name().to_string(),
        supports_delete: provider.supports_delete(),
        session_id,
        timestamp,
        timestamp_text: format_timestamp(timestamp),
        last_updated,
        last_updated_text: format_timestamp(last_updated),
        last_updated_age: format_relative_age(last_updated),
        project_path: normalized_project_path.clone(),
        project_key: normalized_project_path.to_ascii_lowercase(),
        project_name: project_name_value.clone(),
        repo_root: git_context.repo_root.clone(),
        repo_name: git_context.repo_name.clone(),
        branch_name: git_context.branch_name.clone(),
        branch_display: git_context.branch_display.clone(),
        is_detached_head: git_context.is_detached_head,
        workspace_key: git_context.workspace_key.clone(),
        workspace_label: workspace_label(
            &git_context.repo_name,
            &git_context.branch_display,
            &project_name_value,
            &normalized_project_path,
            &git_context.repo_root,
        ),
        file_path,
        project_exists: !normalized_project_path.is_empty()
            && Path::new(&normalized_project_path).exists(),
        alias,
        preview: preview_text,
        display_title,
        slug,
    }
}

fn read_codex_session_file(
    file_path: &Path,
    index: &AliasIndex,
    git_cache: &mut HashMap<String, GitContext>,
) -> Result<Option<SessionRecord>> {
    let metadata =
        fs::metadata(file_path).with_context(|| format!("read {}", file_path.display()))?;
    let file = File::open(file_path).with_context(|| format!("open {}", file_path.display()))?;
    let reader = BufReader::new(file);

    let mut meta: Option<Value> = None;
    let mut preview = String::new();
    let mut fallback_preview = String::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if meta.is_none() && strv(&entry, "type") == "session_meta" {
            meta = obj(&entry, "payload").cloned();
        }

        let candidate = codex_preview_candidate(&entry);
        if !candidate.trim().is_empty() {
            if fallback_preview.is_empty() {
                fallback_preview = candidate.clone();
            }
            if preview.is_empty() && meaningful_user_text(&candidate) {
                preview = candidate;
            }
        }
        if meta.is_some() && !preview.is_empty() {
            break;
        }
    }

    let Some(meta) = meta else {
        return Ok(None);
    };
    let session_id = strv(&meta, "id");
    if session_id.trim().is_empty() {
        return Ok(None);
    }

    let timestamp = parse_rfc3339_local(&strv(&meta, "timestamp"))
        .unwrap_or_else(|| local_from_system_time(&metadata));
    let last_updated = local_from_system_time(&metadata);
    if preview.is_empty() {
        preview = fallback_preview;
    }

    Ok(Some(new_session_record(
        ProviderKind::Codex,
        session_id.clone(),
        timestamp,
        last_updated,
        strv(&meta, "cwd"),
        preview,
        index.get_alias(ProviderKind::Codex, &session_id),
        file_path.to_path_buf(),
        String::new(),
        false,
        String::new(),
        git_cache,
    )))
}

fn claude_history_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
        .join("history.jsonl")
}

fn claude_history_index() -> Result<HashMap<String, Value>> {
    let history_path = claude_history_path();
    if !history_path.exists() {
        return Ok(HashMap::new());
    }
    let file =
        File::open(&history_path).with_context(|| format!("open {}", history_path.display()))?;
    let reader = BufReader::new(file);
    let mut index = HashMap::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let session_id = strv(&entry, "sessionId");
        if !session_id.is_empty() {
            index.insert(session_id, entry);
        }
    }
    Ok(index)
}

fn read_claude_session_file(
    file_path: &Path,
    index: &AliasIndex,
    git_cache: &mut HashMap<String, GitContext>,
    history_index: &HashMap<String, Value>,
) -> Result<Option<SessionRecord>> {
    let metadata =
        fs::metadata(file_path).with_context(|| format!("read {}", file_path.display()))?;
    let last_modified = local_from_system_time(&metadata);
    let file = File::open(file_path).with_context(|| format!("open {}", file_path.display()))?;
    let reader = BufReader::new(file);

    let mut session_id = String::new();
    let mut project_path = String::new();
    let mut preview = String::new();
    let mut fallback_preview = String::new();
    let mut timestamp: Option<DateTime<Local>> = None;
    let mut last_updated: Option<DateTime<Local>> = None;
    let mut recorded_branch_name = String::new();
    let mut recorded_detached_head = false;
    let mut slug = String::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(&line) else {
            continue;
        };

        if session_id.is_empty() {
            session_id = strv(&entry, "sessionId");
        }
        if project_path.is_empty() {
            project_path = strv(&entry, "cwd");
        }
        if slug.is_empty() {
            slug = strv(&entry, "slug");
        }

        let git_branch = strv(&entry, "gitBranch");
        if !git_branch.is_empty() {
            if git_branch == "HEAD" {
                recorded_detached_head = true;
                recorded_branch_name.clear();
            } else {
                recorded_detached_head = false;
                recorded_branch_name = git_branch;
            }
        }

        let entry_timestamp = strv(&entry, "timestamp");
        if let Some(parsed) = parse_rfc3339_local(&entry_timestamp) {
            timestamp = match timestamp {
                Some(current) if current <= parsed => Some(current),
                _ => Some(parsed),
            };
            last_updated = match last_updated {
                Some(current) if current >= parsed => Some(current),
                _ => Some(parsed),
            };
        }

        let candidate = claude_preview_candidate(&entry);
        if !candidate.trim().is_empty() {
            if fallback_preview.is_empty() {
                fallback_preview = candidate.clone();
            }
            if preview.is_empty() && meaningful_user_text(&candidate) {
                preview = candidate;
            }
        }
    }

    if session_id.is_empty() {
        session_id = file_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();
    }

    let history_entry = history_index.get(&session_id);
    if preview.is_empty() {
        if let Some(display) = history_entry.map(|entry| strv(entry, "display")) {
            if meaningful_user_text(&display) {
                preview = display;
            }
        }
    }
    if preview.is_empty() {
        preview = fallback_preview;
    }

    if project_path.is_empty() {
        if let Some(entry) = history_entry {
            project_path = strv(entry, "project");
        }
    }

    if timestamp.is_none() {
        if let Some(entry) = history_entry {
            if let Some(millis) = obj(entry, "timestamp").and_then(Value::as_i64) {
                if let Some(dt_utc) = Utc.timestamp_millis_opt(millis).single() {
                    timestamp = Some(dt_utc.with_timezone(&Local));
                }
            }
        }
    }

    let timestamp = timestamp.unwrap_or(last_modified);
    let last_updated = last_updated.unwrap_or(last_modified);

    Ok(Some(new_session_record(
        ProviderKind::Claude,
        session_id.clone(),
        timestamp,
        last_updated,
        project_path,
        preview,
        index.get_alias(ProviderKind::Claude, &session_id),
        file_path.to_path_buf(),
        recorded_branch_name,
        recorded_detached_head,
        slug,
        git_cache,
    )))
}

pub fn load_sessions(provider: ProviderKind, index: &AliasIndex) -> Result<Vec<SessionRecord>> {
    let session_root = provider_session_root(provider);
    if !session_root.exists() {
        return Ok(Vec::new());
    }

    let mut files = WalkDir::new(&session_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("jsonl"))
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    files.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    files.reverse();

    let mut git_cache = HashMap::new();
    let history = if provider == ProviderKind::Claude {
        claude_history_index()?
    } else {
        HashMap::new()
    };
    let mut sessions = Vec::new();
    for file in files {
        let session = match provider {
            ProviderKind::Codex => read_codex_session_file(&file, index, &mut git_cache)?,
            ProviderKind::Claude => {
                read_claude_session_file(&file, index, &mut git_cache, &history)?
            }
        };
        if let Some(session) = session {
            sessions.push(session);
        }
    }

    sessions.sort_by(|left, right| {
        right
            .timestamp
            .cmp(&left.timestamp)
            .then_with(|| left.project_path.cmp(&right.project_path))
    });
    Ok(sessions)
}

pub fn display_sessions(sessions: &[SessionRecord]) -> Vec<DisplaySession> {
    let mut grouped = HashMap::<(ProviderKind, String), Vec<SessionRecord>>::new();
    for session in sessions {
        let key = if session.workspace_key.trim().is_empty() {
            session.project_key.clone()
        } else {
            session.workspace_key.clone()
        };
        grouped
            .entry((session.provider, key))
            .or_default()
            .push(session.clone());
    }

    let mut ordered_projects = grouped
        .into_iter()
        .map(|((provider, group_key), mut items)| {
            items.sort_by(|left, right| right.timestamp.cmp(&left.timestamp));
            let latest_time = items
                .first()
                .map(|item| item.timestamp)
                .unwrap_or_else(Local::now);
            let project_path = items
                .first()
                .map(|item| item.project_path.clone())
                .unwrap_or_default();
            (provider, group_key, latest_time, project_path, items)
        })
        .collect::<Vec<_>>();

    ordered_projects.sort_by(|left, right| right.2.cmp(&left.2).then_with(|| left.3.cmp(&right.3)));

    let mut display = Vec::new();
    for (_, group_key, _, _, items) in ordered_projects {
        for session in items {
            display.push(DisplaySession {
                group_key: group_key.clone(),
                display_number: display.len() + 1,
                session,
            });
        }
    }
    display
}

pub fn filter_display_sessions(sessions: &[SessionRecord], query: &str) -> Vec<DisplaySession> {
    let display = display_sessions(sessions);
    let normalized = query.trim().trim_matches('"').trim();
    if normalized.is_empty() {
        return display;
    }

    if normalized.chars().all(|ch| ch.is_ascii_digit()) {
        return display
            .into_iter()
            .filter(|entry| entry.display_number.to_string().starts_with(normalized))
            .collect();
    }

    let lower = normalized.to_ascii_lowercase();
    let (mode, needle) = if let Some(rest) = lower
        .strip_prefix("title:")
        .or_else(|| lower.strip_prefix("t:"))
    {
        ("title", rest.trim().to_string())
    } else if let Some(rest) = lower
        .strip_prefix("repo:")
        .or_else(|| lower.strip_prefix("r:"))
    {
        ("repo", rest.trim().to_string())
    } else if let Some(rest) = lower
        .strip_prefix("branch:")
        .or_else(|| lower.strip_prefix("b:"))
    {
        ("branch", rest.trim().to_string())
    } else {
        ("default", lower)
    };

    if needle.trim().is_empty() {
        return display;
    }

    display
        .into_iter()
        .filter(|entry| match mode {
            "title" => entry
                .session
                .display_title
                .to_ascii_lowercase()
                .contains(&needle),
            "repo" => entry
                .session
                .repo_name
                .to_ascii_lowercase()
                .contains(&needle),
            "branch" => entry
                .session
                .branch_display
                .to_ascii_lowercase()
                .contains(&needle),
            _ => {
                entry
                    .session
                    .project_name
                    .to_ascii_lowercase()
                    .contains(&needle)
                    || entry
                        .session
                        .repo_name
                        .to_ascii_lowercase()
                        .contains(&needle)
            }
        })
        .collect()
}

pub fn find_session<'a>(
    sessions: &'a [DisplaySession],
    session_id: &str,
    provider: Option<ProviderKind>,
) -> Option<&'a DisplaySession> {
    let exact = sessions
        .iter()
        .filter(|entry| {
            entry.session.session_id == session_id
                && provider
                    .map(|p| entry.session.provider == p)
                    .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    if exact.len() == 1 {
        return exact.first().copied();
    }

    let prefix = sessions
        .iter()
        .filter(|entry| {
            entry.session.session_id.starts_with(session_id)
                && provider
                    .map(|p| entry.session.provider == p)
                    .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    if prefix.len() == 1 {
        return prefix.first().copied();
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;

    fn sample_session(name: &str, number: usize) -> SessionRecord {
        let now = Local::now();
        SessionRecord {
            provider: ProviderKind::Codex,
            provider_label: "Codex".to_string(),
            supports_delete: true,
            session_id: format!("session-{number}"),
            timestamp: now,
            timestamp_text: format_timestamp(now),
            last_updated: now,
            last_updated_text: format_timestamp(now),
            last_updated_age: format_relative_age(now),
            project_path: format!("/tmp/{name}"),
            project_key: format!("/tmp/{name}"),
            project_name: name.to_string(),
            repo_root: format!("/tmp/{name}"),
            repo_name: name.to_string(),
            branch_name: "main".to_string(),
            branch_display: "main".to_string(),
            is_detached_head: false,
            workspace_key: format!("{name}|main"),
            workspace_label: format!("{name} @ main"),
            file_path: PathBuf::from(format!("/tmp/{name}.jsonl")),
            project_exists: false,
            alias: String::new(),
            preview: "preview text".to_string(),
            display_title: format!("title {number}"),
            slug: String::new(),
        }
    }

    #[test]
    fn filters_title_queries() {
        let sessions = vec![sample_session("alpha", 1), sample_session("beta", 2)];
        let filtered = filter_display_sessions(&sessions, "title:title 2");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].session.project_name, "beta");
    }
}
