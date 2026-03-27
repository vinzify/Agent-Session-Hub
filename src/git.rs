use crate::paths::normalize_path;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

#[derive(Clone, Debug, Default)]
pub struct GitContext {
    pub repo_root: String,
    pub repo_name: String,
    pub branch_name: String,
    pub branch_display: String,
    pub is_detached_head: bool,
    pub workspace_key: String,
}

fn branch_display(branch_name: &str, detached: bool) -> String {
    if !branch_name.trim().is_empty() {
        return branch_name.trim().to_string();
    }
    if detached {
        return "detached".to_string();
    }
    String::new()
}

fn workspace_key(repo_root: &str, branch_name: &str, project_path: &str) -> String {
    [repo_root, branch_name, project_path]
        .into_iter()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("|")
}

fn project_name(value: &str) -> String {
    Path::new(value)
        .file_name()
        .and_then(|part| part.to_str())
        .unwrap_or(value)
        .to_string()
}

pub fn workspace_label(
    repo_name: &str,
    branch_display: &str,
    project_name_value: &str,
    project_path: &str,
    repo_root: &str,
) -> String {
    if repo_name.trim().is_empty() {
        return project_name_value.to_string();
    }

    let mut label = repo_name.to_string();
    if !branch_display.trim().is_empty() {
        label = format!("{label} @ {branch_display}");
    }

    let normalized_project = normalize_path(project_path);
    let normalized_root = normalize_path(repo_root);
    if !normalized_project.is_empty()
        && !normalized_root.is_empty()
        && normalized_project != normalized_root
    {
        let leaf = project_name(&normalized_project);
        if !leaf.trim().is_empty() && leaf != repo_name {
            label = format!("{label} / {leaf}");
        }
    }

    label
}

pub fn get_git_context(path: &str, cache: &mut HashMap<String, GitContext>) -> GitContext {
    let normalized = normalize_path(path);
    let default_workspace_key = normalized.to_ascii_lowercase();
    if normalized.is_empty() {
        return GitContext {
            workspace_key: default_workspace_key,
            ..GitContext::default()
        };
    }

    if let Some(cached) = cache.get(&normalized) {
        return cached.clone();
    }

    let default = GitContext {
        workspace_key: default_workspace_key,
        ..GitContext::default()
    };

    let path_obj = Path::new(&normalized);
    if !path_obj.exists() {
        cache.insert(normalized, default.clone());
        return default;
    }

    let repo_root_output = Command::new("git")
        .args(["-C", &normalized, "rev-parse", "--show-toplevel"])
        .output();
    let context = match repo_root_output {
        Ok(output) if output.status.success() => {
            let repo_root = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let repo_root = normalize_path(&repo_root);
            let branch_output = Command::new("git")
                .args(["-C", &normalized, "branch", "--show-current"])
                .output();
            let branch_name = match branch_output {
                Ok(branch) if branch.status.success() => {
                    String::from_utf8_lossy(&branch.stdout).trim().to_string()
                }
                _ => String::new(),
            };
            let is_detached_head = branch_name.trim().is_empty();
            let branch_display = branch_display(&branch_name, is_detached_head);
            GitContext {
                repo_name: project_name(&repo_root),
                workspace_key: workspace_key(&repo_root, &branch_display, &normalized),
                repo_root,
                branch_name,
                branch_display,
                is_detached_head,
            }
        }
        _ => default.clone(),
    };

    cache.insert(normalized, context.clone());
    context
}

pub fn apply_recorded_branch(
    git_context: &GitContext,
    project_path: &str,
    recorded_branch_name: &str,
    recorded_detached_head: bool,
) -> GitContext {
    if recorded_branch_name.trim().is_empty() && !recorded_detached_head {
        return git_context.clone();
    }

    let branch_name = if recorded_detached_head {
        String::new()
    } else {
        recorded_branch_name.trim().to_string()
    };
    let branch_display = branch_display(&branch_name, recorded_detached_head);
    GitContext {
        repo_root: git_context.repo_root.clone(),
        repo_name: git_context.repo_name.clone(),
        branch_name,
        branch_display: branch_display.clone(),
        is_detached_head: recorded_detached_head,
        workspace_key: workspace_key(&git_context.repo_root, &branch_display, project_path),
    }
}

pub fn _assert_result(_: Result<()>) {}
