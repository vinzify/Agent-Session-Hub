use crate::formatting::{ascii_banner, compress_text};
use crate::provider::ProviderKind;
use crate::session::{DisplaySession, SessionRecord};
use anyhow::{Context, Result, anyhow};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::process::{Command, Stdio};

#[derive(Clone, Debug)]
pub struct BrowserResult {
    pub action: String,
    pub session_ids: Vec<String>,
}

fn row_key(provider: ProviderKind, session_id: &str) -> String {
    format!("S:{}:{session_id}", provider.name())
}

fn workspace_row_key(provider: ProviderKind, workspace_key: &str) -> String {
    format!("W:{}:{}", provider.name(), workspace_key)
}

pub fn fzf_row(entry: &DisplaySession) -> String {
    let fields = [
        row_key(entry.session.provider, &entry.session.session_id),
        entry.display_number.to_string(),
        entry.session.timestamp_text.clone(),
        compress_text(&entry.session.workspace_label, 28),
        compress_text(&entry.session.display_title, 90),
        entry.session.project_path.clone(),
        entry.session.preview.clone(),
    ];
    fields
        .iter()
        .map(|value| value.replace('\t', " ").replace('"', "'"))
        .collect::<Vec<_>>()
        .join("\t")
}

pub fn fzf_rows(entries: &[DisplaySession]) -> Vec<String> {
    let mut groups = BTreeMap::<(ProviderKind, String), Vec<&DisplaySession>>::new();
    for entry in entries {
        groups
            .entry((entry.session.provider, entry.group_key.clone()))
            .or_default()
            .push(entry);
    }

    let mut rows = Vec::new();
    for ((provider, group_key), items) in groups {
        let workspace_label = items
            .first()
            .map(|item| item.session.workspace_label.clone())
            .unwrap_or_default();
        let project_path = items
            .first()
            .map(|item| item.session.project_path.clone())
            .unwrap_or_default();
        let header = [
            workspace_row_key(provider, &group_key),
            String::new(),
            String::new(),
            format!("[{}] {workspace_label}", items.len()),
            compress_text(&project_path, 100),
            project_path,
            String::new(),
        ];
        rows.push(
            header
                .iter()
                .map(|value| value.replace('\t', " ").replace('"', "'"))
                .collect::<Vec<_>>()
                .join("\t"),
        );
        for item in items {
            rows.push(fzf_row(item));
        }
    }
    rows
}

pub fn browser_header(provider: ProviderKind) -> String {
    let keys_line = if provider.supports_delete() {
        "Keys: Enter open | Tab mark | Ctrl-E rename | Ctrl-R reset | Ctrl-D delete(confirm)"
    } else {
        "Keys: Enter open | Tab mark | Ctrl-E rename | Ctrl-R reset"
    };
    format!("Find: text folder/repo | # number | title:term | repo:term | branch:term\n{keys_line}")
}

pub fn run_fzf(
    provider: ProviderKind,
    initial_query: &str,
    rows: &[String],
    exe: &std::path::Path,
) -> Result<Option<BrowserResult>> {
    let mut args = vec![
        "--ansi".to_string(),
        "--multi".to_string(),
        "--disabled".to_string(),
        "--layout=reverse".to_string(),
        "--height=100%".to_string(),
        "--border".to_string(),
        "--delimiter".to_string(),
        "\t".to_string(),
        "--accept-nth".to_string(),
        "1".to_string(),
        "--with-nth".to_string(),
        "2,3,4,5".to_string(),
        "--nth".to_string(),
        "2".to_string(),
        "--preview".to_string(),
        format!("'{}' __preview {{}}", exe.display()),
        "--preview-window".to_string(),
        "right:40%:wrap".to_string(),
        "--bind".to_string(),
        if provider.supports_delete() {
            format!(
                "start:reload-sync('{}' __query),change:reload-sync('{}' __query {{q}})+first,enter:print(enter)+accept,ctrl-e:print(ctrl-e)+accept,ctrl-r:print(ctrl-r)+accept,ctrl-d:print(ctrl-d)+accept",
                exe.display(),
                exe.display()
            )
        } else {
            format!(
                "start:reload-sync('{}' __query),change:reload-sync('{}' __query {{q}})+first,enter:print(enter)+accept,ctrl-e:print(ctrl-e)+accept,ctrl-r:print(ctrl-r)+accept",
                exe.display(),
                exe.display()
            )
        },
        "--header".to_string(),
        browser_header(provider),
    ];

    if !initial_query.trim().is_empty() {
        args.push("--query".to_string());
        args.push(initial_query.to_string());
    }

    let mut command = Command::new("fzf");
    command.args(&args);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let mut child = command.spawn().context("spawn fzf")?;
    if let Some(stdin) = child.stdin.as_mut() {
        for row in rows {
            writeln!(stdin, "{row}")?;
        }
    }
    let output = child.wait_with_output().context("wait for fzf")?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(None);
    }
    let mut lines = stdout.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    let mut action = lines
        .first()
        .cloned()
        .unwrap_or_else(|| "enter".to_string());
    if action.trim().is_empty() {
        action = "enter".to_string();
    }
    let mut selected_rows = lines.split_off(1);
    if selected_rows.is_empty()
        && lines.len() == 1
        && !matches!(action.as_str(), "enter" | "ctrl-d" | "ctrl-e" | "ctrl-r")
    {
        action = "enter".to_string();
        selected_rows = lines;
    }
    Ok(Some(BrowserResult {
        action,
        session_ids: selected_rows
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
    }))
}

pub fn session_preview(session: &DisplaySession, project_session_count: usize) -> String {
    let project_count_text = if project_session_count > 0 {
        format!("{project_session_count} sessions")
    } else {
        String::new()
    };

    [
        ascii_banner(
            "session",
            &format!(
                "#{} {}",
                session.display_number, session.session.workspace_label
            ),
            &session.session.last_updated_age,
        ),
        format!("Provider: {}", session.session.provider_label),
        format!("Title:    {}", session.session.display_title),
        format!("Project:  {}", session.session.project_path),
        if session.session.repo_root.is_empty() {
            String::new()
        } else {
            format!("Repo:     {}", session.session.repo_root)
        },
        if session.session.branch_display.is_empty() {
            String::new()
        } else {
            format!("Branch:   {}", session.session.branch_display)
        },
        format!("Exists:   {}", session.session.project_exists),
        if project_count_text.is_empty() {
            String::new()
        } else {
            format!("Group:    {project_count_text}")
        },
        format!("Started:  {}", session.session.timestamp_text),
        format!(
            "Updated:  {} ({})",
            session.session.last_updated_age, session.session.last_updated_text
        ),
        format!("Session:  {}", session.session.session_id),
        String::new(),
        "Preview".to_string(),
        "-------".to_string(),
        if session.session.preview.is_empty() {
            "<no meaningful preview>".to_string()
        } else {
            session.session.preview.clone()
        },
    ]
    .into_iter()
    .filter(|line| !line.is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}

pub fn workspace_preview(project_sessions: &[&DisplaySession]) -> String {
    let latest = project_sessions[0];
    let session_numbers = project_sessions
        .iter()
        .map(|entry| entry.display_number)
        .collect::<Vec<_>>();
    let range_text = if session_numbers.is_empty() {
        "-".to_string()
    } else {
        format!(
            "#{} -> #{}",
            session_numbers.iter().min().unwrap_or(&0),
            session_numbers.iter().max().unwrap_or(&0)
        )
    };
    let branch_names = project_sessions
        .iter()
        .map(|entry| entry.session.branch_display.clone())
        .filter(|value| !value.trim().is_empty())
        .collect::<BTreeSet<_>>();
    let branch_summary = if branch_names.len() == 1 {
        branch_names.iter().next().cloned().unwrap_or_default()
    } else if branch_names.len() > 1 {
        let visible = branch_names.iter().take(3).cloned().collect::<Vec<_>>();
        let mut summary = visible.join(", ");
        if branch_names.len() > visible.len() {
            summary = format!("{summary} +{} more", branch_names.len() - visible.len());
        }
        summary
    } else {
        String::new()
    };

    let mut lines = vec![
        ascii_banner(
            "workspace",
            &latest.session.workspace_label,
            &format!("{} sessions", project_sessions.len()),
        ),
        format!("Provider: {}", latest.session.provider_label),
        format!("Path:     {}", latest.session.project_path),
    ];
    if !latest.session.repo_root.is_empty() {
        lines.push(format!("Repo:     {}", latest.session.repo_root));
    }
    if !branch_summary.is_empty() {
        lines.push(format!("Branch:   {branch_summary}"));
    }
    lines.extend([
        format!("Exists:   {}", latest.session.project_exists),
        format!(
            "Latest:   {} ({})",
            latest.session.last_updated_age, latest.session.last_updated_text
        ),
        format!("Started:  {}", latest.session.timestamp_text),
        format!("Range:    {range_text}"),
        String::new(),
        "Recent".to_string(),
        "------".to_string(),
    ]);
    for entry in project_sessions.iter().take(3) {
        lines.push(format!(
            "  {:<7} {}",
            entry.session.last_updated_age,
            compress_text(&entry.session.display_title, 52)
        ));
    }
    lines.join("\n")
}

pub fn write_preview(
    provider: ProviderKind,
    display_sessions: &[DisplaySession],
    session_id: &str,
    workspace_key: &str,
    project_path: &str,
) -> Result<()> {
    let mut project_sessions = Vec::new();
    if !workspace_key.trim().is_empty() {
        project_sessions = display_sessions
            .iter()
            .filter(|entry| entry.session.provider == provider && entry.group_key == workspace_key)
            .collect::<Vec<_>>();
    } else if !project_path.trim().is_empty() {
        project_sessions = display_sessions
            .iter()
            .filter(|entry| {
                entry.session.provider == provider && entry.session.project_path == project_path
            })
            .collect::<Vec<_>>();
    }

    if !session_id.trim().is_empty() {
        let session = display_sessions.iter().find(|entry| {
            entry.session.provider == provider && entry.session.session_id == session_id
        });
        if let Some(session) = session {
            if project_sessions.is_empty() {
                project_sessions = display_sessions
                    .iter()
                    .filter(|entry| {
                        entry.session.provider == provider && entry.group_key == session.group_key
                    })
                    .collect::<Vec<_>>();
            }
            println!("{}", session_preview(session, project_sessions.len()));
            return Ok(());
        }
    }

    if workspace_key.trim().is_empty() && project_path.trim().is_empty() {
        println!();
        return Ok(());
    }
    if project_sessions.is_empty() {
        println!();
        return Ok(());
    }
    println!("{}", workspace_preview(&project_sessions));
    Ok(())
}

pub fn parse_row_target(raw: &str) -> (String, String, String) {
    if let Some(rest) = raw.strip_prefix("S:") {
        let parts = rest.splitn(2, ':').collect::<Vec<_>>();
        if parts.len() == 2 {
            return (parts[1].to_string(), String::new(), String::new());
        }
        return (rest.to_string(), String::new(), String::new());
    }
    if let Some(rest) = raw.strip_prefix("W:") {
        let parts = rest.splitn(2, ':').collect::<Vec<_>>();
        if parts.len() == 2 {
            return (String::new(), parts[1].to_string(), String::new());
        }
        return (String::new(), rest.to_string(), String::new());
    }
    if raw.contains('\t') {
        let columns = raw.split('\t').collect::<Vec<_>>();
        let session_id = columns.first().copied().unwrap_or_default();
        let project_path = columns.get(5).copied().unwrap_or_default();
        return (
            session_id.to_string(),
            String::new(),
            project_path.to_string(),
        );
    }
    (raw.to_string(), String::new(), String::new())
}

pub fn confirm_delete(sessions: &[SessionRecord]) -> Result<bool> {
    let workspace_labels = sessions
        .iter()
        .map(|session| session.workspace_label.clone())
        .filter(|value| !value.trim().is_empty())
        .collect::<BTreeSet<_>>();
    let workspace_text = if workspace_labels.len() == 1 {
        workspace_labels.iter().next().cloned().unwrap_or_default()
    } else if workspace_labels.len() > 1 {
        format!("{} workspaces", workspace_labels.len())
    } else {
        format!("{} sessions", sessions.len())
    };

    println!(
        "Delete {} session{}",
        sessions.len(),
        if sessions.len() == 1 { "" } else { "s" }
    );
    println!("Workspace: {workspace_text}");
    println!("Targets:");
    for session in sessions.iter().take(2) {
        println!(
            "  - #{} {}",
            session.session_id,
            compress_text(&session.display_title, 72)
        );
    }
    if sessions.len() > 2 {
        println!("  - ... and {} more", sessions.len() - 2);
    }
    print!("Confirm delete? [y/N] ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(matches!(
        input.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

pub fn resolve_selected_sessions<'a>(
    display_sessions: &'a [DisplaySession],
    selected_ids: &[String],
) -> Vec<&'a DisplaySession> {
    let mut selected = Vec::new();
    for selected_id in selected_ids {
        if let Some(rest) = selected_id.strip_prefix("S:") {
            let mut parts = rest.splitn(2, ':');
            let provider = parts.next().unwrap_or_default();
            let id = parts.next().unwrap_or(provider);
            if let Some(entry) = display_sessions
                .iter()
                .find(|entry| entry.session.session_id == id)
            {
                selected.push(entry);
            }
            continue;
        }
        if let Some(rest) = selected_id.strip_prefix("W:") {
            let mut parts = rest.splitn(2, ':');
            let _provider = parts.next();
            let workspace_key = parts.next().unwrap_or(rest);
            for entry in display_sessions
                .iter()
                .filter(|entry| entry.group_key == workspace_key)
            {
                if !selected
                    .iter()
                    .any(|existing| existing.session.session_id == entry.session.session_id)
                {
                    selected.push(entry);
                }
            }
            continue;
        }
        if let Some(entry) = display_sessions
            .iter()
            .find(|entry| entry.session.session_id == *selected_id)
        {
            selected.push(entry);
        }
    }
    selected
}

pub fn ensure_fzf() -> Result<()> {
    which::which("fzf").map(|_| ()).map_err(|_| {
        anyhow!("fzf is required but was not found in PATH. Run doctor for install help.")
    })
}
