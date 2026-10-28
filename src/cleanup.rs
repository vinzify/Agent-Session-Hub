use crate::formatting::{compress_text, format_bytes, format_count};
use crate::session::{SessionRecord, security_scan_identity};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Local};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{self, Write};
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CleanupKind {
    SecurityScan,
    LinkedAgents,
    OrphanAgents,
}

impl CleanupKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::SecurityScan => "Security scan",
            Self::LinkedAgents => "Chat agents",
            Self::OrphanAgents => "Orphan agents",
        }
    }
}

#[derive(Clone, Debug)]
pub struct CleanupPolicy {
    pub security_scan_age: Duration,
    pub linked_agent_age: Duration,
    pub orphan_agent_age: Duration,
    pub include_security_scans: bool,
    pub include_linked_agents: bool,
    pub include_orphan_agents: bool,
}

impl Default for CleanupPolicy {
    fn default() -> Self {
        Self {
            security_scan_age: Duration::days(7),
            linked_agent_age: Duration::days(30),
            orphan_agent_age: Duration::days(7),
            include_security_scans: true,
            include_linked_agents: true,
            include_orphan_agents: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CleanupGroup {
    pub key: String,
    pub kind: CleanupKind,
    pub title: String,
    pub detail: String,
    pub session_ids: Vec<String>,
    pub storage_bytes: u64,
    pub latest_updated: DateTime<Local>,
}

impl CleanupGroup {
    pub fn age_days(&self, now: DateTime<Local>) -> i64 {
        (now - self.latest_updated).num_days().max(0)
    }
}

fn is_old_enough(last_updated: DateTime<Local>, age: Duration, now: DateTime<Local>) -> bool {
    last_updated <= now - age
}

fn group_from_sessions(
    key: String,
    kind: CleanupKind,
    title: String,
    detail: String,
    sessions: &[&SessionRecord],
) -> Option<CleanupGroup> {
    let latest_updated = sessions.iter().map(|session| session.last_updated).max()?;
    Some(CleanupGroup {
        key,
        kind,
        title,
        detail,
        session_ids: sessions
            .iter()
            .map(|session| session.session_id.clone())
            .collect(),
        storage_bytes: sessions.iter().map(|session| session.storage_bytes).sum(),
        latest_updated,
    })
}

pub fn cleanup_groups(
    sessions: &[SessionRecord],
    policy: &CleanupPolicy,
    now: DateTime<Local>,
) -> Vec<CleanupGroup> {
    let mut groups = Vec::new();
    let mut scan_sessions = BTreeMap::<String, Vec<&SessionRecord>>::new();
    let mut scan_ids = HashSet::new();
    for session in sessions {
        if let Some(scan) = security_scan_identity(&session.project_path) {
            scan_ids.insert(session.session_id.clone());
            scan_sessions.entry(scan.key).or_default().push(session);
        }
    }

    if policy.include_security_scans {
        for (scan_key, items) in scan_sessions {
            let latest = items.iter().map(|session| session.last_updated).max();
            if !latest.is_some_and(|value| is_old_enough(value, policy.security_scan_age, now)) {
                continue;
            }
            let identity = items
                .first()
                .and_then(|session| security_scan_identity(&session.project_path));
            let title = identity
                .as_ref()
                .map(|scan| scan.label.clone())
                .unwrap_or_else(|| "Codex Security scan".to_string());
            let detail = identity
                .map(|scan| scan.root_path)
                .unwrap_or_else(|| items[0].project_path.clone());
            if let Some(group) = group_from_sessions(
                format!("scan:{scan_key}"),
                CleanupKind::SecurityScan,
                title,
                detail,
                &items,
            ) {
                groups.push(group);
            }
        }
    }

    let roots = sessions
        .iter()
        .filter(|session| !session.is_agent)
        .map(|session| (session.session_id.as_str(), session))
        .collect::<HashMap<_, _>>();
    let mut linked_agents = BTreeMap::<String, Vec<&SessionRecord>>::new();
    let mut orphan_agents = BTreeMap::<String, Vec<&SessionRecord>>::new();

    for session in sessions
        .iter()
        .filter(|session| session.is_agent && !scan_ids.contains(&session.session_id))
    {
        if roots.contains_key(session.root_session_id.as_str()) {
            if policy.include_linked_agents
                && is_old_enough(session.last_updated, policy.linked_agent_age, now)
            {
                linked_agents
                    .entry(session.root_session_id.clone())
                    .or_default()
                    .push(session);
            }
        } else if policy.include_orphan_agents
            && is_old_enough(session.last_updated, policy.orphan_agent_age, now)
        {
            let key = if session.workspace_key.trim().is_empty() {
                session.project_key.clone()
            } else {
                session.workspace_key.clone()
            };
            orphan_agents.entry(key).or_default().push(session);
        }
    }

    for (root_id, items) in linked_agents {
        let root = roots.get(root_id.as_str()).copied();
        let title = root
            .map(|session| session.display_title.clone())
            .unwrap_or_else(|| root_id.clone());
        let detail = root
            .map(|session| session.workspace_label.clone())
            .unwrap_or_default();
        if let Some(group) = group_from_sessions(
            format!("agents:{root_id}"),
            CleanupKind::LinkedAgents,
            title,
            detail,
            &items,
        ) {
            groups.push(group);
        }
    }

    for (workspace_key, items) in orphan_agents {
        let title = items
            .first()
            .map(|session| session.workspace_label.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "Unknown workspace".to_string());
        let detail = items
            .first()
            .map(|session| session.project_path.clone())
            .unwrap_or_default();
        if let Some(group) = group_from_sessions(
            format!("orphans:{workspace_key}"),
            CleanupKind::OrphanAgents,
            title,
            detail,
            &items,
        ) {
            groups.push(group);
        }
    }

    groups.sort_by(|left, right| {
        right
            .storage_bytes
            .cmp(&left.storage_bytes)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.title.cmp(&right.title))
    });
    groups
}

fn clean_field(value: &str) -> String {
    value.replace(['\t', '\r', '\n'], " ")
}

pub fn select_cleanup_groups(groups: &[CleanupGroup]) -> Result<Vec<String>> {
    let now = Local::now();
    let rows = groups
        .iter()
        .map(|group| {
            [
                group.key.clone(),
                group.kind.label().to_string(),
                format_bytes(group.storage_bytes),
                format!("{}d old", group.age_days(now)),
                format_count(group.session_ids.len(), "session"),
                compress_text(&group.title, 72),
                group.detail.clone(),
            ]
            .iter()
            .map(|field| clean_field(field))
            .collect::<Vec<_>>()
            .join("\t")
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut child = Command::new("fzf")
        .args([
            "--multi",
            "--layout=reverse",
            "--height=100%",
            "--border",
            "--delimiter=\t",
            "--with-nth=2,3,4,5,6,7",
            "--nth=2,6,7",
            "--header=Cleanup candidates | Tab mark | Enter review | Esc cancel",
            "--prompt=Cleanup> ",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("spawn cleanup selector")?;

    if let Some(mut stdin) = child.stdin.take()
        && let Err(error) = stdin.write_all(rows.as_bytes())
        && error.kind() != io::ErrorKind::BrokenPipe
    {
        return Err(error.into());
    }
    let output = child
        .wait_with_output()
        .context("wait for cleanup selector")?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split('\t').next())
        .map(ToOwned::to_owned)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatting::{format_relative_age, format_timestamp};
    use crate::provider::ProviderKind;
    use std::path::PathBuf;

    fn session(id: &str, path: &str, age: Duration) -> SessionRecord {
        let now = Local::now();
        let updated = now - age;
        SessionRecord {
            provider: ProviderKind::Codex,
            provider_label: "Codex".to_string(),
            supports_delete: true,
            session_id: id.to_string(),
            timestamp: updated,
            timestamp_text: format_timestamp(updated),
            last_updated: updated,
            last_updated_text: format_timestamp(updated),
            last_updated_age: format_relative_age(updated),
            project_path: path.to_string(),
            project_key: path.to_string(),
            project_name: "project".to_string(),
            repo_root: path.to_string(),
            repo_name: "project".to_string(),
            branch_name: "main".to_string(),
            branch_display: "main".to_string(),
            is_detached_head: false,
            workspace_key: format!("{path}|main"),
            workspace_label: "project @ main".to_string(),
            file_path: PathBuf::from(format!("/tmp/{id}.jsonl")),
            project_exists: false,
            alias: String::new(),
            preview: "preview".to_string(),
            display_title: format!("title {id}"),
            slug: String::new(),
            storage_bytes: 1024,
            storage_size_is_estimate: false,
            is_agent: false,
            root_session_id: id.to_string(),
            parent_session_id: String::new(),
            agent_depth: 0,
            agent_path: String::new(),
            agent_nickname: String::new(),
        }
    }

    fn agent(id: &str, root_id: &str, path: &str, age: Duration) -> SessionRecord {
        let mut value = session(id, path, age);
        value.is_agent = true;
        value.root_session_id = root_id.to_string();
        value.parent_session_id = root_id.to_string();
        value.agent_depth = 1;
        value
    }

    #[test]
    fn scan_cleanup_groups_whole_runs_and_waits_for_every_session_to_age() {
        let now = Local::now();
        let old_path = "/tmp/codex-security-scans-one/repo/run-old/output";
        let recent_path = "/tmp/codex-security-scans-two/repo/run-active/output";
        let sessions = vec![
            session("old-root", old_path, Duration::days(20)),
            agent("old-agent", "old-root", old_path, Duration::days(19)),
            session("active-root", recent_path, Duration::days(20)),
            agent(
                "active-agent",
                "active-root",
                recent_path,
                Duration::hours(1),
            ),
        ];

        let groups = cleanup_groups(&sessions, &CleanupPolicy::default(), now);
        let scans = groups
            .iter()
            .filter(|group| group.kind == CleanupKind::SecurityScan)
            .collect::<Vec<_>>();
        assert_eq!(scans.len(), 1);
        assert_eq!(scans[0].title, "repo · run-old");
        assert_eq!(scans[0].session_ids.len(), 2);
    }

    #[test]
    fn agent_cleanup_preserves_roots_and_classifies_orphans() {
        let now = Local::now();
        let root = session("root", "/work/project", Duration::days(90));
        let old_agent = agent("old-agent", "root", "/work/project", Duration::days(40));
        let recent_agent = agent("recent-agent", "root", "/work/project", Duration::days(2));
        let orphan = agent("orphan", "missing", "/work/gone", Duration::days(10));

        let groups = cleanup_groups(
            &[root, old_agent, recent_agent, orphan],
            &CleanupPolicy::default(),
            now,
        );
        let linked = groups
            .iter()
            .find(|group| group.kind == CleanupKind::LinkedAgents)
            .expect("linked agents group");
        assert_eq!(linked.session_ids, vec!["old-agent"]);
        assert!(!linked.session_ids.iter().any(|id| id == "root"));

        let orphaned = groups
            .iter()
            .find(|group| group.kind == CleanupKind::OrphanAgents)
            .expect("orphan agents group");
        assert_eq!(orphaned.session_ids, vec!["orphan"]);
    }
}
