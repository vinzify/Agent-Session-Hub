use serde_json::json;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn cli_command(root: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_agent-session-hub"));
    command
        .args(args)
        .env("CODEX_SESSION_HUB_SESSION_ROOT", root.join("sessions"))
        .env(
            "CODEX_SESSION_HUB_CLAUDE_SESSION_ROOT",
            root.join("sessions"),
        )
        .env("CODEX_SESSION_HUB_CONFIG_ROOT", root.join("config"))
        .env("CODEX_SESSION_HUB_CACHE_ROOT", root.join("cache"))
        .env_remove("CODEX_SESSION_HUB_DISABLE_CACHE")
        .stdin(std::process::Stdio::null());
    command
}

fn cli(root: &Path, args: &[&str]) -> Output {
    cli_command(root, args).output().unwrap()
}

fn query(root: &Path) -> String {
    let output = cli(root, &["--provider", "codex", "__query"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn write_session(path: &Path, id: &str, project: &Path, title: &str) {
    let meta = json!({
        "type": "session_meta",
        "payload": { "id": id, "timestamp": "2026-01-01T10:00:00Z", "cwd": project }
    });
    let message = json!({
        "type": "event_msg", "payload": { "type": "user_message", "message": title }
    });
    fs::write(path, format!("{meta}\n{message}\n")).unwrap();
}

#[test]
fn cached_queries_follow_file_changes_aliases_new_sessions_and_deletions() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let sessions = root.join("sessions");
    fs::create_dir(&sessions).unwrap();
    let first = sessions.join("first.jsonl");
    write_session(
        &first,
        "first",
        &root.join("project"),
        "Fix the account settings page",
    );

    let cold = query(root);
    assert_eq!(query(root), cold);
    assert!(root.join("cache/codex.json").exists());

    let rename = cli(
        root,
        &[
            "--provider",
            "codex",
            "rename",
            "first",
            "--name",
            "Account settings",
        ],
    );
    assert!(rename.status.success());
    assert!(query(root).contains("Account settings"));
    assert!(
        cli(root, &["--provider", "codex", "reset", "first"])
            .status
            .success()
    );
    write_session(
        &first,
        "first",
        &root.join("project"),
        "Implement multilingual settings and preferences",
    );
    let changed = query(root);
    assert!(changed.contains("Implement multilingual settings"));
    assert!(!changed.contains("Fix the account settings page"));

    // An incomplete log must become visible once its writer appends metadata.
    let second = sessions.join("second.jsonl");
    fs::write(&second, "{incomplete\n").unwrap();
    assert!(!query(root).contains("S:codex:second"));
    write_session(
        &second,
        "second",
        &root.join("other"),
        "Prepare the release checklist",
    );
    assert!(query(root).contains("S:codex:second"));
    fs::remove_file(&first).unwrap();
    assert!(!query(root).contains("S:codex:first"));
}

#[test]
fn cached_queries_refresh_live_git_branch() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let project = root.join("project");
    fs::create_dir(&project).unwrap();
    let run_git = |args: &[&str]| {
        let output = Command::new("git")
            .arg("-C")
            .arg(&project)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run_git(&["init", "--initial-branch=initial"]);
    fs::create_dir(root.join("sessions")).unwrap();
    write_session(
        &root.join("sessions/chat.jsonl"),
        "chat",
        &project,
        "Review the current implementation",
    );
    assert!(query(root).contains("project @ initial"));
    run_git(&["symbolic-ref", "HEAD", "refs/heads/updated"]);
    assert!(query(root).contains("project @ updated"));
}

#[test]
fn explicit_provider_command_never_prompts_for_a_provider() {
    let temp = tempfile::tempdir().unwrap();
    let output = cli(temp.path(), &["codex", "help"]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("csx cleanup"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Select CLI"));
}

#[test]
fn query_aware_preview_replaces_a_stale_selection_during_reload() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::create_dir(root.join("sessions")).unwrap();
    write_session(
        &root.join("sessions/first.jsonl"),
        "first",
        &root.join("alpha"),
        "Review the account settings implementation",
    );
    write_session(
        &root.join("sessions/second.jsonl"),
        "second",
        &root.join("beta"),
        "Release the compatibility update",
    );
    let output = cli(
        root,
        &[
            "--provider",
            "codex",
            "__preview",
            "--query",
            "title:Release",
            "S:codex:first",
        ],
    );
    assert!(output.status.success());
    let preview = String::from_utf8(output.stdout).unwrap();
    assert!(preview.contains("Release the compatibility update"));
    assert!(!preview.contains("Review the account settings implementation"));

    let output = cli(
        root,
        &[
            "--provider",
            "codex",
            "__preview",
            "--query",
            "title:no-such-session",
            "S:codex:first",
        ],
    );
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().trim().is_empty());
}

#[test]
fn browsing_survives_an_unavailable_cache_directory() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::create_dir(root.join("sessions")).unwrap();
    write_session(
        &root.join("sessions/chat.jsonl"),
        "chat",
        &root.join("project"),
        "Review the current implementation",
    );
    let blocked = root.join("blocked-cache");
    fs::write(&blocked, "this is a file, not a directory").unwrap();
    let output = cli_command(root, &["--provider", "codex", "__query"])
        .env("CODEX_SESSION_HUB_CACHE_ROOT", &blocked)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("S:codex:chat"));
}

#[test]
fn claude_cache_refreshes_appended_messages_and_history_fallbacks() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::create_dir(root.join("sessions")).unwrap();
    let first = json!({
        "sessionId": "claude-cache-test", "cwd": root.join("project"),
        "timestamp": "2026-01-01T10:00:00Z", "gitBranch": "initial"
    });
    let source = root.join("sessions/chat.jsonl");
    fs::write(&source, format!("{first}\n")).unwrap();
    let history = root.join("history.jsonl");
    let entry =
        json!({"sessionId": "claude-cache-test", "display": "First title from Claude history"});
    fs::write(&history, format!("{entry}\n")).unwrap();
    let query_claude = || {
        let output = cli(root, &["--provider", "claude", "__query"]);
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    };
    let initial = query_claude();
    assert!(initial.contains("First title from Claude history"));
    assert_eq!(initial, query_claude());
    let entry =
        json!({"sessionId": "claude-cache-test", "display": "Updated title from Claude history"});
    fs::write(&history, format!("{entry}\n")).unwrap();
    assert!(query_claude().contains("Updated title from Claude history"));
    let message = json!({
        "sessionId": "claude-cache-test", "timestamp": "2026-01-02T10:00:00Z",
        "type": "user", "message": {"content": "A new user request after the cache was created"}
    });
    fs::write(&source, format!("{first}\n{message}\n")).unwrap();
    assert!(query_claude().contains("A new user request after the cache was created"));
}

#[cfg(unix)]
#[test]
fn cleanup_skips_a_scan_that_resumes_during_selection() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;
    use std::time::{Duration, SystemTime};

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::create_dir(root.join("sessions")).unwrap();
    let source = root.join("sessions/scan.jsonl");
    write_session(
        &source,
        "scan",
        Path::new("/tmp/codex-security-scans-test/repo/run/output"),
        "Review a generated security scan session",
    );
    fs::File::open(&source)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(10 * 86400))
        .unwrap();

    let bin = root.join("bin");
    fs::create_dir(&bin).unwrap();
    let fake_fzf = bin.join("fzf");
    fs::write(
        &fake_fzf,
        "#!/bin/sh\nIFS= read -r row\ntouch \"$ASH_TEST_SESSION\"\nprintf '%s\\n' \"$row\"\n",
    )
    .unwrap();
    fs::set_permissions(&fake_fzf, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap();
    let mut child = cli_command(
        root,
        &["--provider", "codex", "cleanup", "--security-scans"],
    )
    .env("PATH", path)
    .env("ASH_TEST_SESSION", &source)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
    child.stdin.take().unwrap().write_all(b"DELETE\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(source.exists());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Skipped 1 groups"));
}
