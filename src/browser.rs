use crate::formatting::{
    ascii_banner, compress_text, escape_sh_single_quotes, format_bytes, format_count,
};
use crate::provider::ProviderKind;
use crate::session::{DisplaySession, SessionRecord, security_scan_identity};
use anyhow::{Context, Result, anyhow};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Seek, SeekFrom, Write};
use std::process::{Command, Stdio};

#[derive(Clone, Debug)]
pub struct BrowserResult {
    pub action: String,
    pub session_ids: Vec<String>,
}

fn normalize_selected_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if let Some((first, _)) = trimmed.split_once('\t') {
        return first.trim().to_string();
    }

    trimmed.to_string()
}

fn row_key(provider: ProviderKind, session_id: &str) -> String {
    format!("S:{}:{session_id}", provider.name())
}

fn fzf_shell_command(exe: &std::path::Path, command: &str) -> String {
    let exe_text = exe.display().to_string();
    if cfg!(windows) {
        format!("\"{exe_text}\" {command}")
    } else {
        format!("'{}' {command}", escape_sh_single_quotes(&exe_text))
    }
}

fn workspace_row_key(provider: ProviderKind, workspace_key: &str) -> String {
    format!("W:{}:{}", provider.name(), workspace_key)
}

pub fn scan_group_marker(group_key: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in group_key.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("scan-{hash:016x}")
}

fn storage_size(bytes: u64, is_estimate: bool) -> String {
    format!(
        "{}{}",
        if is_estimate { "~" } else { "" },
        format_bytes(bytes)
    )
}

fn fzf_row_with_labels(
    entry: &DisplaySession,
    number: String,
    workspace: String,
    title: String,
) -> String {
    let fields = [
        row_key(entry.session.provider, &entry.session.session_id),
        number,
        storage_size(
            entry.session.storage_bytes,
            entry.session.storage_size_is_estimate,
        ),
        entry.session.last_updated_text.clone(),
        compress_text(&workspace, 28),
        compress_text(&title, 90),
        entry.session.project_path.clone(),
        entry.session.preview.clone(),
    ];
    fields
        .iter()
        .map(|value| value.replace('\t', " ").replace('"', "'"))
        .collect::<Vec<_>>()
        .join("\t")
}

fn agent_title(entry: &DisplaySession) -> String {
    let identity = if !entry.session.agent_nickname.trim().is_empty() {
        entry.session.agent_nickname.trim().to_string()
    } else if !entry.session.agent_path.trim().is_empty() {
        entry
            .session
            .agent_path
            .trim_start_matches("/root/")
            .trim_start_matches('/')
            .to_string()
    } else {
        "agent".to_string()
    };
    let path = entry
        .session
        .agent_path
        .trim_start_matches("/root/")
        .trim_start_matches('/');
    let detail = if path.is_empty() || path == identity {
        identity
    } else {
        format!("{identity} · {path}")
    };
    format!(
        "{}↳ Agent: {detail}",
        "  ".repeat(entry.session.agent_depth)
    )
}

fn push_agent_tree(
    rows: &mut Vec<String>,
    items: &[&DisplaySession],
    parent_id: &str,
    emitted: &mut BTreeSet<String>,
) {
    let mut children = items
        .iter()
        .copied()
        .filter(|item| item.session.is_agent && item.session.parent_session_id == parent_id)
        .collect::<Vec<_>>();
    children.sort_by(|left, right| right.session.last_updated.cmp(&left.session.last_updated));
    for child in children {
        if !emitted.insert(child.session.session_id.clone()) {
            continue;
        }
        rows.push(fzf_row_with_labels(
            child,
            String::new(),
            String::new(),
            agent_title(child),
        ));
        push_agent_tree(rows, items, &child.session.session_id, emitted);
    }
}

pub fn fzf_rows(
    entries: &[DisplaySession],
    all_entries: &[DisplaySession],
    expanded_roots: &BTreeSet<String>,
    reveal_agents: bool,
) -> Vec<String> {
    let mut all_groups = BTreeMap::<(ProviderKind, String), Vec<&DisplaySession>>::new();
    let mut group_order = BTreeMap::new();
    for (position, entry) in all_entries.iter().enumerate() {
        group_order
            .entry((entry.session.provider, entry.group_key.clone()))
            .or_insert(position);
        all_groups
            .entry((entry.session.provider, entry.group_key.clone()))
            .or_default()
            .push(entry);
    }
    let mut groups = BTreeMap::<(ProviderKind, String), Vec<&DisplaySession>>::new();
    for entry in entries {
        groups
            .entry((entry.session.provider, entry.group_key.clone()))
            .or_default()
            .push(entry);
    }

    let mut groups = groups.into_iter().collect::<Vec<_>>();
    // Keep exactly the same workspace order used for conversation numbering.
    groups.sort_by_key(|(key, _)| group_order.get(key).copied().unwrap_or(usize::MAX));
    let mut rows = Vec::new();
    for ((provider, group_key), items) in groups {
        let is_security_scan = group_key.starts_with("security-scan:");
        let scan_expanded = expanded_roots.contains(&scan_group_marker(&group_key));
        let all_items = all_groups
            .get(&(provider, group_key.clone()))
            .cloned()
            .unwrap_or_else(|| items.clone());
        let representative = all_items
            .iter()
            .copied()
            .find(|item| !item.session.is_agent)
            .or_else(|| all_items.first().copied())
            .or_else(|| items.first().copied());
        let scan_identity =
            representative.and_then(|item| security_scan_identity(&item.session.project_path));
        let workspace_label = scan_identity
            .as_ref()
            .map(|scan| {
                format!(
                    "{} Security scan · {}",
                    if scan_expanded { "▼" } else { "▶" },
                    scan.label
                )
            })
            .or_else(|| representative.map(|item| item.session.workspace_label.clone()))
            .unwrap_or_default();
        let project_path = scan_identity
            .as_ref()
            .map(|scan| scan.root_path.clone())
            .or_else(|| representative.map(|item| item.session.project_path.clone()))
            .unwrap_or_default();
        let total_bytes = all_items
            .iter()
            .map(|item| item.session.storage_bytes)
            .sum::<u64>();
        let size_is_estimate = all_items
            .iter()
            .any(|item| item.session.storage_size_is_estimate);
        let conversation_count = all_items
            .iter()
            .filter(|item| !item.session.is_agent)
            .count();
        let agent_count = all_items
            .iter()
            .filter(|item| item.session.is_agent)
            .count();
        let count_label = if is_security_scan {
            format!(
                "{} · {}",
                format_count(conversation_count, "scan chat"),
                format_count(agent_count, "agent")
            )
        } else if agent_count > 0 {
            format!(
                "{} · {}",
                format_count(conversation_count, "chat"),
                format_count(agent_count, "agent")
            )
        } else {
            format_count(conversation_count, "chat")
        };
        let header = [
            workspace_row_key(provider, &group_key),
            String::new(),
            storage_size(total_bytes, size_is_estimate),
            String::new(),
            format!("[{count_label}] {workspace_label}"),
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
        let mut roots = items
            .iter()
            .copied()
            .filter(|item| !item.session.is_agent)
            .collect::<Vec<_>>();
        roots.sort_by(|left, right| right.session.last_updated.cmp(&left.session.last_updated));
        let mut emitted = BTreeSet::new();
        if is_security_scan && !scan_expanded && !reveal_agents {
            continue;
        }
        for item in roots {
            let agents = all_items
                .iter()
                .copied()
                .filter(|candidate| {
                    candidate.session.is_agent
                        && candidate.session.root_session_id == item.session.session_id
                })
                .collect::<Vec<_>>();
            let agent_bytes = agents
                .iter()
                .map(|agent| agent.session.storage_bytes)
                .sum::<u64>();
            let has_filtered_agents = items.iter().any(|candidate| {
                candidate.session.is_agent
                    && candidate.session.root_session_id == item.session.session_id
            });
            let explicitly_expanded = expanded_roots.contains(&item.session.session_id);
            let expanded = explicitly_expanded || (reveal_agents && has_filtered_agents);
            let filtered_agent_count = items
                .iter()
                .filter(|candidate| {
                    candidate.session.is_agent
                        && candidate.session.root_session_id == item.session.session_id
                })
                .count();
            let agent_count_label =
                if expanded && !explicitly_expanded && filtered_agent_count < agents.len() {
                    format!("{filtered_agent_count}/{}", agents.len())
                } else {
                    agents.len().to_string()
                };
            let title = if agents.is_empty() {
                item.session.display_title.clone()
            } else {
                format!(
                    "{} {} {} · {}  │  {}",
                    if expanded { "▼" } else { "▶" },
                    agent_count_label,
                    if agents.len() == 1 { "agent" } else { "agents" },
                    storage_size(
                        agent_bytes,
                        agents
                            .iter()
                            .any(|agent| agent.session.storage_size_is_estimate)
                    ),
                    item.session.display_title,
                )
            };
            let number_label = if item.display_number == 0 {
                String::new()
            } else {
                item.display_number.to_string()
            };
            rows.push(fzf_row_with_labels(
                item,
                number_label,
                item.session.workspace_label.clone(),
                title,
            ));
            emitted.insert(item.session.session_id.clone());
            if expanded {
                let visible_agents = if explicitly_expanded {
                    &all_items
                } else {
                    &items
                };
                push_agent_tree(
                    &mut rows,
                    visible_agents,
                    &item.session.session_id,
                    &mut emitted,
                );
                for agent in visible_agents.iter().filter(|candidate| {
                    candidate.session.is_agent
                        && candidate.session.root_session_id == item.session.session_id
                }) {
                    if emitted.insert(agent.session.session_id.clone()) {
                        rows.push(fzf_row_with_labels(
                            agent,
                            String::new(),
                            String::new(),
                            agent_title(agent),
                        ));
                    }
                }
            }
        }
        let known_roots = all_items
            .iter()
            .filter(|item| !item.session.is_agent)
            .map(|item| item.session.session_id.as_str())
            .collect::<BTreeSet<_>>();
        // Keep orphaned agent sessions accessible even when their root file is gone.
        for item in items.iter().filter(|item| {
            item.session.is_agent && !known_roots.contains(item.session.root_session_id.as_str())
        }) {
            if emitted.insert(item.session.session_id.clone()) {
                rows.push(fzf_row_with_labels(
                    item,
                    String::new(),
                    item.session.workspace_label.clone(),
                    format!("[orphan] {}", agent_title(item)),
                ));
            }
        }
    }
    rows
}

fn parse_fzf_output(stdout: &str) -> Option<BrowserResult> {
    if stdout.trim().is_empty() {
        return None;
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
        && !matches!(
            action.as_str(),
            "enter" | "ctrl-a" | "ctrl-d" | "ctrl-e" | "ctrl-r"
        )
    {
        action = "enter".to_string();
        selected_rows = lines;
    }

    Some(BrowserResult {
        action,
        session_ids: selected_rows
            .into_iter()
            .map(|value| normalize_selected_value(&value))
            .filter(|value| !value.is_empty())
            .collect(),
    })
}

pub fn browser_header(provider: ProviderKind) -> String {
    let keys_line = if provider.supports_delete() {
        "Keys: Enter open | Ctrl-A expand | Tab mark | Ctrl-E rename | Ctrl-R reset | Ctrl-D delete(confirm)"
    } else {
        "Keys: Enter open | Ctrl-A expand | Tab mark | Ctrl-E rename | Ctrl-R reset"
    };
    format!("Find: text folder/repo | # number | title:term | repo:term | branch:term\n{keys_line}")
}

pub fn run_fzf(
    provider: ProviderKind,
    initial_query: &str,
    initial_rows: &[String],
    exe: &std::path::Path,
    expanded_roots: &BTreeSet<String>,
) -> Result<Option<BrowserResult>> {
    run_fzf_with_program(
        provider,
        initial_query,
        initial_rows,
        exe,
        std::path::Path::new("fzf"),
        expanded_roots,
    )
}

fn run_fzf_with_program(
    provider: ProviderKind,
    initial_query: &str,
    initial_rows: &[String],
    exe: &std::path::Path,
    fzf_program: &std::path::Path,
    expanded_roots: &BTreeSet<String>,
) -> Result<Option<BrowserResult>> {
    let provider_flag = format!("--provider {}", provider.name());
    let expanded = if expanded_roots.is_empty() {
        "-".to_string()
    } else {
        expanded_roots.iter().cloned().collect::<Vec<_>>().join(",")
    };
    let query_command = format!("{provider_flag} __query --expanded {expanded}");
    let mut args = vec![
        "--ansi".to_string(),
        "--multi".to_string(),
        "--disabled".to_string(),
        "--layout=reverse".to_string(),
        "--height=100%".to_string(),
        "--border".to_string(),
        "--delimiter".to_string(),
        "\t".to_string(),
        "--with-nth".to_string(),
        "2,3,4,5,6".to_string(),
        "--nth".to_string(),
        "2".to_string(),
        "--preview".to_string(),
        fzf_shell_command(
            exe,
            &format!("{provider_flag} __preview --query {{q}} {{}}"),
        ),
        "--preview-window".to_string(),
        "right:40%:wrap".to_string(),
        "--expect".to_string(),
        if provider.supports_delete() {
            "enter,ctrl-a,ctrl-e,ctrl-r,ctrl-d".to_string()
        } else {
            "enter,ctrl-a,ctrl-e,ctrl-r".to_string()
        },
        "--bind".to_string(),
        format!(
            "change:reload({})+first",
            fzf_shell_command(exe, &format!("{query_command} {{q}}"))
        ),
        "--header".to_string(),
        browser_header(provider),
    ];

    if !initial_query.trim().is_empty() {
        args.push("--query".to_string());
        args.push(initial_query.to_string());
    }

    // Seed exactly one initial snapshot. A rewound regular file avoids a pipe
    // writer racing with reload/cancellation and works without newer fzf events.
    let mut input = tempfile::tempfile().context("create initial session list")?;
    for row in initial_rows {
        writeln!(input, "{row}").context("write initial session list")?;
    }
    input
        .seek(SeekFrom::Start(0))
        .context("rewind initial session list")?;

    let mut command = Command::new(fzf_program);
    command.args(&args);
    command
        .stdin(Stdio::from(input))
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let child = command.spawn().context("spawn fzf")?;
    let output = child.wait_with_output().context("wait for fzf")?;
    if matches!(output.status.code(), Some(1 | 130)) {
        return Ok(None);
    }
    if !output.status.success() {
        return Err(anyhow!(
            "fzf failed with {}. Run doctor to check the installation.",
            output.status
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_fzf_output(&stdout))
}

pub fn session_preview(session: &DisplaySession, project_sessions: &[&DisplaySession]) -> String {
    let conversation_count = project_sessions
        .iter()
        .filter(|entry| !entry.session.is_agent)
        .count();
    let project_count_text = if conversation_count > 0 {
        format!("{conversation_count} conversations")
    } else {
        String::new()
    };
    let related_agents = project_sessions
        .iter()
        .copied()
        .filter(|entry| {
            entry.session.is_agent
                && entry.session.root_session_id == session.session.root_session_id
        })
        .collect::<Vec<_>>();
    let agent_bytes = related_agents
        .iter()
        .map(|entry| entry.session.storage_bytes)
        .sum::<u64>();
    let connected_root = if session.session.is_agent {
        project_sessions.iter().copied().find(|entry| {
            !entry.session.is_agent && entry.session.session_id == session.session.root_session_id
        })
    } else {
        None
    };
    let heading = if session.session.is_agent {
        format!(
            "Agent {}",
            if session.session.agent_nickname.is_empty() {
                session.session.session_id.as_str()
            } else {
                session.session.agent_nickname.as_str()
            }
        )
    } else {
        format!(
            "#{} {}",
            session.display_number, session.session.workspace_label
        )
    };

    let mut lines = vec![
        ascii_banner("session", &heading, &session.session.last_updated_age),
        format!("Provider: {}", session.session.provider_label),
    ];
    if session.session.is_agent {
        lines.extend([
            format!("Agent:    {}", session.session.agent_nickname),
            format!("Path:     {}", session.session.agent_path),
            format!("Depth:    {}", session.session.agent_depth),
            format!("Parent:   {}", session.session.parent_session_id),
            format!("Root:     {}", session.session.root_session_id),
            connected_root
                .map(|root| {
                    format!(
                        "Chat:     #{} {}",
                        root.display_number, root.session.display_title
                    )
                })
                .unwrap_or_else(|| "Chat:     <root session missing>".to_string()),
            "Open:     Enter resumes the connected chat".to_string(),
        ]);
    }
    lines.extend([
        format!("Title:    {}", session.session.display_title),
        format!(
            "Size:     {}",
            storage_size(
                session.session.storage_bytes,
                session.session.storage_size_is_estimate
            )
        ),
        if !session.session.is_agent && !related_agents.is_empty() {
            format!(
                "Agents:   {} ({})",
                related_agents.len(),
                storage_size(
                    agent_bytes,
                    related_agents
                        .iter()
                        .any(|entry| entry.session.storage_size_is_estimate)
                )
            )
        } else {
            String::new()
        },
        if !session.session.is_agent && !related_agents.is_empty() {
            format!(
                "Total:    {}",
                storage_size(
                    session.session.storage_bytes + agent_bytes,
                    session.session.storage_size_is_estimate
                        || related_agents
                            .iter()
                            .any(|entry| entry.session.storage_size_is_estimate)
                )
            )
        } else {
            String::new()
        },
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
    ]);
    lines
        .into_iter()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn workspace_preview(project_sessions: &[&DisplaySession]) -> String {
    let representative = project_sessions
        .iter()
        .copied()
        .find(|entry| !entry.session.is_agent)
        .unwrap_or(project_sessions[0]);
    let latest = project_sessions
        .iter()
        .copied()
        .max_by_key(|entry| entry.session.last_updated)
        .unwrap_or(representative);
    let scan_identity = security_scan_identity(&representative.session.project_path);
    let session_numbers = project_sessions
        .iter()
        .filter(|entry| !entry.session.is_agent && entry.display_number > 0)
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
    let total_bytes = project_sessions
        .iter()
        .map(|entry| entry.session.storage_bytes)
        .sum::<u64>();
    let size_is_estimate = project_sessions
        .iter()
        .any(|entry| entry.session.storage_size_is_estimate);

    let conversation_count = project_sessions
        .iter()
        .filter(|entry| !entry.session.is_agent)
        .count();
    let agent_count = project_sessions.len() - conversation_count;
    let count_text = if scan_identity.is_some() {
        format!(
            "{} · {}",
            format_count(conversation_count, "scan chat"),
            format_count(agent_count, "agent")
        )
    } else if agent_count > 0 {
        format!(
            "{} · {}",
            format_count(conversation_count, "chat"),
            format_count(agent_count, "agent")
        )
    } else {
        format_count(conversation_count, "chat")
    };
    let mut lines = vec![
        ascii_banner(
            "workspace",
            &scan_identity
                .as_ref()
                .map(|scan| format!("Security scan · {}", scan.label))
                .unwrap_or_else(|| representative.session.workspace_label.clone()),
            &count_text,
        ),
        format!("Provider: {}", representative.session.provider_label),
        format!("Size:     {}", storage_size(total_bytes, size_is_estimate)),
        format!(
            "Path:     {}",
            scan_identity
                .as_ref()
                .map(|scan| scan.root_path.as_str())
                .unwrap_or(&representative.session.project_path)
        ),
    ];
    if !representative.session.repo_root.is_empty() {
        lines.push(format!("Repo:     {}", representative.session.repo_root));
    }
    if !branch_summary.is_empty() {
        lines.push(format!("Branch:   {branch_summary}"));
    }
    lines.extend([
        format!("Exists:   {}", representative.session.project_exists),
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
    for entry in project_sessions
        .iter()
        .filter(|entry| !entry.session.is_agent)
        .take(3)
    {
        lines.push(format!(
            "  {:<7} {:>9}  {}",
            entry.session.last_updated_age,
            storage_size(
                entry.session.storage_bytes,
                entry.session.storage_size_is_estimate
            ),
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
    fn emit_preview(text: &str) -> Result<()> {
        let stdout = io::stdout();
        let mut output = stdout.lock();
        match writeln!(output, "{text}") {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

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
            return emit_preview(&session_preview(session, &project_sessions));
        }
    }

    if workspace_key.trim().is_empty() && project_path.trim().is_empty() {
        return emit_preview("");
    }
    if project_sessions.is_empty() {
        return emit_preview("");
    }
    emit_preview(&workspace_preview(&project_sessions))
}

pub fn parse_row_target(raw: &str) -> (String, String, String) {
    let first_column = raw.split('\t').next().unwrap_or(raw).trim();

    if let Some(rest) = first_column.strip_prefix("S:") {
        let parts = rest.splitn(2, ':').collect::<Vec<_>>();
        if parts.len() == 2 {
            return (parts[1].trim().to_string(), String::new(), String::new());
        }
        return (rest.trim().to_string(), String::new(), String::new());
    }
    if let Some(rest) = first_column.strip_prefix("W:") {
        let parts = rest.splitn(2, ':').collect::<Vec<_>>();
        if parts.len() == 2 {
            return (String::new(), parts[1].trim().to_string(), String::new());
        }
        return (String::new(), rest.trim().to_string(), String::new());
    }
    if raw.contains('\t') {
        let columns = raw.split('\t').collect::<Vec<_>>();
        let session_id = columns.first().copied().unwrap_or_default();
        let project_path = columns.get(6).copied().unwrap_or_default();
        return (
            session_id.trim().to_string(),
            String::new(),
            project_path.trim().to_string(),
        );
    }
    (raw.trim().to_string(), String::new(), String::new())
}

pub fn confirm_delete(sessions: &[SessionRecord]) -> Result<bool> {
    let agent_count = sessions.iter().filter(|session| session.is_agent).count();
    let conversation_count = sessions.len() - agent_count;
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

    if agent_count > 0 {
        eprintln!(
            "Delete {conversation_count} conversation{} and {agent_count} connected agent session{}",
            if conversation_count == 1 { "" } else { "s" },
            if agent_count == 1 { "" } else { "s" }
        );
    } else {
        eprintln!(
            "Delete {} session{}",
            sessions.len(),
            if sessions.len() == 1 { "" } else { "s" }
        );
    }
    eprintln!("Workspace: {workspace_text}");
    eprintln!("Targets:");
    for session in sessions.iter().take(2) {
        eprintln!(
            "  - {}{} {}",
            if session.is_agent { "agent " } else { "chat " },
            session.session_id,
            compress_text(&session.display_title, 72)
        );
    }
    if sessions.len() > 2 {
        eprintln!("  - ... and {} more", sessions.len() - 2);
    }
    eprint!("Confirm delete? [y/N] ");
    io::stderr().flush()?;
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

#[cfg(test)]
mod tests {
    use super::{
        normalize_selected_value, parse_fzf_output, parse_row_target, run_fzf_with_program,
        storage_size,
    };
    use crate::provider::ProviderKind;
    use std::collections::BTreeSet;
    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use std::path::Path;

    #[test]
    fn strips_full_fzf_rows_to_first_column() {
        assert_eq!(
            normalize_selected_value("S:codex:abc123	1	2026-03-27 10:00	repo	Title"),
            "S:codex:abc123"
        );
        assert_eq!(
            normalize_selected_value("W:codex:repo|main			[2] repo"),
            "W:codex:repo|main"
        );
        assert_eq!(
            normalize_selected_value("S:claude:def456"),
            "S:claude:def456"
        );
    }

    #[test]
    fn preserves_expected_shortcut_actions() {
        let result = parse_fzf_output(
            "ctrl-r
S:codex:abc123
",
        )
        .expect("parsed output");
        assert_eq!(result.action, "ctrl-r");
        assert_eq!(result.session_ids, vec!["S:codex:abc123"]);
    }

    #[test]
    fn parses_prefixed_rows_with_extra_columns() {
        assert_eq!(
            parse_row_target("S:codex:abc123	1	2026-03-28 12:00	repo	Title"),
            ("abc123".to_string(), String::new(), String::new())
        );
        assert_eq!(
            parse_row_target("W:codex:repo|main			[2] repo"),
            (String::new(), "repo|main".to_string(), String::new())
        );
        assert_eq!(
            parse_row_target("legacy-id	1	2 KiB	2026-03-28 12:00	repo	Title	/tmp/project	Preview"),
            (
                "legacy-id".to_string(),
                String::new(),
                "/tmp/project".to_string()
            )
        );
    }

    #[test]
    fn marks_estimated_storage_sizes() {
        assert_eq!(storage_size(1536, false), "1.5 KiB");
        assert_eq!(storage_size(1536, true), "~1.5 KiB");
    }

    #[cfg(unix)]
    #[test]
    fn fzf_receives_one_initial_snapshot_and_compatible_reload_bindings() {
        let temp = tempfile::tempdir().unwrap();
        let fake_fzf = temp.path().join("fzf");
        fs::write(
            &fake_fzf,
            "#!/bin/sh\ncase \"$*\" in *start:*|*reload-sync*) exit 20;; esac\n[ -f /dev/stdin ] || exit 21\nIFS= read -r row || exit 22\n[ \"$row\" = 'S:codex:abc123' ] || exit 23\nif IFS= read -r extra; then exit 24; fi\nprintf '\\n%s\\n' \"$row\"\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_fzf).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_fzf, permissions).unwrap();

        let result = run_fzf_with_program(
            ProviderKind::Codex,
            "",
            &["S:codex:abc123".to_string()],
            Path::new("/tmp/agent-session-hub"),
            &fake_fzf,
            &BTreeSet::new(),
        )
        .unwrap()
        .unwrap();

        assert_eq!(result.action, "enter");
        assert_eq!(result.session_ids, vec!["S:codex:abc123"]);
    }

    #[cfg(unix)]
    #[test]
    fn early_fzf_cancellation_does_not_break_an_input_pipe() {
        let temp = tempfile::tempdir().unwrap();
        let fake_fzf = temp.path().join("fzf");
        fs::write(&fake_fzf, "#!/bin/sh\nexit 130\n").unwrap();
        fs::set_permissions(&fake_fzf, fs::Permissions::from_mode(0o755)).unwrap();
        let rows = vec!["S:codex:abc123".repeat(1024); 256];
        let result = run_fzf_with_program(
            ProviderKind::Codex,
            "",
            &rows,
            Path::new("/tmp/agent-session-hub"),
            &fake_fzf,
            &BTreeSet::new(),
        )
        .unwrap();
        assert!(result.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn fzf_startup_failure_is_reported_instead_of_treated_as_cancellation() {
        let temp = tempfile::tempdir().unwrap();
        let fake_fzf = temp.path().join("fzf");
        fs::write(&fake_fzf, "#!/bin/sh\nexit 2\n").unwrap();
        fs::set_permissions(&fake_fzf, fs::Permissions::from_mode(0o755)).unwrap();
        let error = run_fzf_with_program(
            ProviderKind::Codex,
            "",
            &[],
            Path::new("/tmp/agent-session-hub"),
            &fake_fzf,
            &BTreeSet::new(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("fzf failed"));
    }
}
