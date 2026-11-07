use serde_json::json;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn cli_command(root: &Path, args: &[&str]) -> Command {
    cli_at(
        root,
        Path::new(env!("CARGO_BIN_EXE_agent-session-hub")),
        args,
    )
}

fn cli_at(root: &Path, binary: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(binary);
    command
        .args(args)
        .env("CODEX_SESSION_HUB_SESSION_ROOT", root.join("sessions"))
        .env(
            "CODEX_SESSION_HUB_CLAUDE_SESSION_ROOT",
            root.join("sessions"),
        )
        .env(
            "CODEX_SESSION_HUB_ANTIGRAVITY_SESSION_ROOT",
            root.join("sessions"),
        )
        .env("AGENT_SESSION_HUB_NO_UPDATE_CHECK", "1")
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

fn managed_install(
    root: &Path,
) -> (
    std::path::PathBuf,
    std::path::PathBuf,
    Vec<std::path::PathBuf>,
) {
    let install = root.join("managed install è");
    let bin = root.join("custom launchers è");
    fs::create_dir_all(install.join("bin")).unwrap();
    fs::create_dir(&bin).unwrap();
    let install = install.canonicalize().unwrap();
    let bin = bin.canonicalize().unwrap();
    let executable = install.join("bin").join(if cfg!(windows) {
        "agent-session-hub.exe"
    } else {
        "agent-session-hub"
    });
    let mut copies = vec![executable.clone()];
    if cfg!(windows) {
        copies.push(bin.join("agent-session-hub.exe"));
    } else {
        for name in ["csx", "clx", "opx", "sessionhub"] {
            copies.push(bin.join(name));
        }
    }
    for path in &copies {
        fs::copy(env!("CARGO_BIN_EXE_agent-session-hub"), path).unwrap();
    }
    if cfg!(windows) {
        for name in ["csx.cmd", "clx.cmd", "opx.cmd", "sessionhub.cmd"] {
            fs::write(bin.join(name), b"custom existing wrapper\r\n").unwrap();
        }
    }
    let record = json!({"format":1,"install_root":install,"bin_root":bin});
    fs::write(install.join("installation.json"), record.to_string()).unwrap();
    fs::write(
        bin.join(".agent-session-hub-install.json"),
        record.to_string(),
    )
    .unwrap();
    (executable, bin, copies)
}

#[test]
fn installed_commands_backfill_missing_shortcuts_without_installer_or_network() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let (exe, bin, copies) = managed_install(root);
    let agx = bin.join(if cfg!(windows) { "agx.cmd" } else { "agx" });
    let profile = root.join("profile");
    fs::write(&profile, b"custom shell configuration stays untouched\n").unwrap();
    let query = cli_at(root, &exe, &["--provider", "codex", "__query"])
        .output()
        .unwrap();
    assert!(query.status.success());
    assert!(!agx.exists());
    let probe_dir = root.join("version probe");
    fs::create_dir(&probe_dir).unwrap();
    let probe = probe_dir.join(if cfg!(windows) {
        "candidate.exe"
    } else {
        "candidate"
    });
    fs::copy(&exe, &probe).unwrap();
    assert!(
        cli_at(root, &probe, &["--version"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        !agx.exists(),
        "a downloaded version probe must not migrate an installation"
    );
    let output = cli_at(root, &exe, &["providers"])
        .env("PROFILE", &profile)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Added missing Agent Session Hub shortcuts")
    );
    assert!(agx.is_file());
    if cfg!(windows) {
        assert!(
            fs::read_to_string(&agx)
                .unwrap()
                .contains("--provider antigravity")
        );
        assert_eq!(
            fs::read(bin.join("csx.cmd")).unwrap(),
            b"custom existing wrapper\r\n"
        );
    } else {
        assert_eq!(fs::read(&agx).unwrap(), fs::read(&exe).unwrap());
    }
    let again = cli_at(root, &exe, &["version"]).output().unwrap();
    assert!(again.status.success());
    assert!(again.stderr.is_empty());
    assert!(
        cli_at(root, &exe, &["__migrate-shortcuts"])
            .output()
            .unwrap()
            .status
            .success()
    );
    for copy in copies {
        assert_eq!(fs::read(copy).unwrap(), fs::read(&exe).unwrap());
    }
    assert_eq!(
        fs::read(profile).unwrap(),
        b"custom shell configuration stays untouched\n"
    );
    assert!(!root.join("config").exists());
    assert!(!root.join("cache/update-check.json").exists());
}

#[test]
fn shortcut_migration_never_overwrites_existing_commands() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let (exe, bin, _) = managed_install(root);
    let agx = bin.join(if cfg!(windows) { "agx.cmd" } else { "agx" });
    fs::write(&agx, b"unrelated user command").unwrap();
    let output = cli_at(root, &exe, &["__migrate-shortcuts"])
        .output()
        .unwrap();
    assert_eq!(output.status.success(), cfg!(windows));
    assert_eq!(fs::read(&agx).unwrap(), b"unrelated user command");
    #[cfg(unix)]
    {
        fs::remove_file(&agx).unwrap();
        std::os::unix::fs::symlink(root.join("missing target"), &agx).unwrap();
        assert!(
            !cli_at(root, &exe, &["__migrate-shortcuts"])
                .output()
                .unwrap()
                .status
                .success()
        );
        assert!(fs::symlink_metadata(&agx).unwrap().file_type().is_symlink());
    }
}

#[cfg(windows)]
#[test]
fn windows_update_helper_runs_new_binary_migration_and_rolls_back_hook_failure() {
    use sha2::{Digest, Sha256};
    for fail_hook in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let (exe, bin, copies) = managed_install(root);
        let source = root.join("new.exe");
        fs::copy(&exe, &source).unwrap();
        for path in &copies {
            fs::write(path, b"old executable").unwrap();
        }
        let stage = tempfile::Builder::new()
            .prefix("agent-session-hub-update-")
            .tempdir()
            .unwrap();
        let files = copies.iter().map(|path| json!({"destination":path,"source":source,"sha256":format!("{:x}",Sha256::digest(b"old executable"))})).collect::<Vec<_>>();
        let migration = if fail_hook {
            bin.join("missing.exe")
        } else {
            exe.clone()
        };
        let plan = stage.path().join("plan.json");
        fs::write(
            &plan,
            json!({"files":files,"stage":stage.path(),"migration":migration}).to_string(),
        )
        .unwrap();
        let script = stage.path().join("apply.ps1");
        fs::write(&script, include_str!("../src/update-windows.ps1")).unwrap();
        let result = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(script)
            .arg("-PlanPath")
            .arg(plan)
            .arg("-AshParentId")
            .arg("2147483647")
            .output()
            .unwrap();
        assert_eq!(
            result.status.success(),
            !fail_hook,
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        for path in copies {
            assert_eq!(
                fs::read(path).unwrap(),
                if fail_hook {
                    b"old executable".to_vec()
                } else {
                    fs::read(&source).unwrap()
                }
            );
        }
        assert_eq!(bin.join("agx.cmd").exists(), !fail_hook);
    }
}

fn antigravity_fixture(root: &Path) -> rusqlite::Connection {
    let sessions = root.join("sessions");
    fs::create_dir_all(sessions.join("conversations")).unwrap();
    let project = root.join("Sicilia café project");
    fs::create_dir(&project).unwrap();
    let connection =
        rusqlite::Connection::open(sessions.join("conversation_summaries.db")).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE conversation_summaries (
        conversation_id TEXT PRIMARY KEY, title TEXT, preview TEXT,
        last_modified_time TEXT, workspace_uris TEXT)",
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO conversation_summaries VALUES ('ag-chat', 'Antigravity release plan',
        'Review the new release together', '2026-11-07 10:43:48.388633693+00:00', ?1)",
            [json!([reqwest::Url::from_directory_path(&project)
                .unwrap()
                .as_str()])
            .to_string()],
        )
        .unwrap();
    let db = rusqlite::Connection::open(sessions.join("conversations/ag-chat.db")).unwrap();
    db.execute_batch("CREATE TABLE steps (idx INTEGER PRIMARY KEY, step_payload BLOB)")
        .unwrap();
    connection
}

fn history_action(root: &Path, args: &[&str]) -> Command {
    let mut command = cli_command(root, args);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Isolate the process probe from whatever coding apps the developer is
        // running. All destructive tests point only at this test's store.
        let bin = root.join("process-probe");
        fs::create_dir_all(&bin).unwrap();
        let ps = bin.join("ps");
        fs::write(&ps, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&ps, fs::Permissions::from_mode(0o755)).unwrap();
        let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        )))
        .unwrap();
        command.env("PATH", path);
    }
    command
}

fn only_backup(root: &Path, provider: &str) -> String {
    let entries = fs::read_dir(root.join("config/recovery").join(provider))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    entries[0].clone()
}

#[test]
fn antigravity_delete_and_restore_preserve_unrelated_rows_caches_aliases_and_projects() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let conn = antigravity_fixture(root);
    conn.execute_batch("ALTER TABLE conversation_summaries ADD COLUMN parent_conversation_id TEXT DEFAULT '';
        ALTER TABLE conversation_summaries ADD COLUMN nesting_depth INTEGER DEFAULT 0;
        ALTER TABLE conversation_summaries ADD COLUMN agent_name TEXT DEFAULT '';
        ALTER TABLE conversation_summaries ADD COLUMN not_fully_idle INTEGER DEFAULT 0;
        INSERT INTO conversation_summaries SELECT 'ag-child','Worker','Child preview',last_modified_time,workspace_uris,'ag-chat',1,'Worker',0 FROM conversation_summaries WHERE conversation_id='ag-chat';
        INSERT INTO conversation_summaries SELECT 'keep','Keep','Keep preview',last_modified_time,workspace_uris,'ag-chat',0,'',0 FROM conversation_summaries WHERE conversation_id='ag-chat';
        CREATE VIRTUAL TABLE conversations_fts USING fts5(conversation_id UNINDEXED,title,body);
        INSERT INTO conversations_fts VALUES('ag-chat','Root','Root transcript'),('ag-child','Child','Child transcript'),('keep','Keep','Keep transcript');").unwrap();
    for id in ["ag-child", "keep"] {
        let db = rusqlite::Connection::open(root.join(format!("sessions/conversations/{id}.db")))
            .unwrap();
        db.execute_batch("CREATE TABLE steps (idx INTEGER PRIMARY KEY, step_payload BLOB)")
            .unwrap();
    }
    let cache = root.join("sessions/cache");
    fs::create_dir(&cache).unwrap();
    fs::write(cache.join("conversation_metadata.json"),json!({"conversations":{"ag-chat":{"summary":{"ID":"ag-chat","Cloud":false}},"ag-child":{"summary":{"ID":"ag-child","Cloud":false}},"keep":{"summary":{"ID":"keep"}}}}).to_string()).unwrap();
    fs::write(
        cache.join("last_conversations.json"),
        json!({"/example/root":"ag-chat","/example/other":"keep"}).to_string(),
    )
    .unwrap();
    let project = root.join("Sicilia café project/keep.rs");
    fs::write(&project, "user project stays unchanged\n").unwrap();
    assert!(
        cli(
            root,
            &["antigravity", "rename", "ag-chat", "--name", "Release work"]
        )
        .status
        .success()
    );
    let original = fs::read(root.join("sessions/conversations/ag-chat.db")).unwrap();
    let preview = history_action(root, &["antigravity", "delete", "ag-chat", "--dry-run"])
        .output()
        .unwrap();
    assert!(preview.status.success());
    assert!(String::from_utf8_lossy(&preview.stdout).contains("ag-child"));
    // A depth-zero fork has a parent too, but is an independent chat and must
    // survive deletion of the conversation it was forked from.
    assert!(!String::from_utf8_lossy(&preview.stdout).contains("[would delete] keep"));
    let deleted = history_action(root, &["antigravity", "delete", "ag-chat", "--yes"])
        .output()
        .unwrap();
    assert!(
        deleted.status.success(),
        "{}",
        String::from_utf8_lossy(&deleted.stderr)
    );
    assert!(!root.join("sessions/conversations/ag-chat.db").exists());
    assert!(!root.join("sessions/conversations/ag-child.db").exists());
    assert!(root.join("sessions/conversations/keep.db").exists());
    assert_eq!(
        conn.query_row("SELECT count(*) FROM conversation_summaries", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM conversations_fts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let meta: serde_json::Value =
        serde_json::from_slice(&fs::read(cache.join("conversation_metadata.json")).unwrap())
            .unwrap();
    assert!(meta["conversations"].get("ag-chat").is_none());
    assert!(meta["conversations"].get("keep").is_some());
    let id = only_backup(root, "antigravity");
    // Another conversation/cache entry appears after deletion. Restore must not
    // replace the shared index/cache with an old whole-store snapshot.
    conn.execute(
        "UPDATE conversation_summaries SET title='Keep changed' WHERE conversation_id='keep'",
        [],
    )
    .unwrap();
    let mut meta = meta;
    meta["conversations"]["new unrelated entry"] = json!({"summary":{"ID":"new unrelated entry"}});
    fs::write(cache.join("conversation_metadata.json"), meta.to_string()).unwrap();
    let restored = history_action(root, &["antigravity", "recovery", "restore", &id])
        .output()
        .unwrap();
    assert!(
        restored.status.success(),
        "{}",
        String::from_utf8_lossy(&restored.stderr)
    );
    assert_eq!(
        fs::read(root.join("sessions/conversations/ag-chat.db")).unwrap(),
        original
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM conversation_summaries", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM conversations_fts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        conn.query_row(
            "SELECT title FROM conversation_summaries WHERE conversation_id='keep'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "Keep changed"
    );
    let meta: serde_json::Value =
        serde_json::from_slice(&fs::read(cache.join("conversation_metadata.json")).unwrap())
            .unwrap();
    assert!(meta["conversations"].get("new unrelated entry").is_some());
    assert!(
        String::from_utf8_lossy(&cli(root, &["antigravity", "__query"]).stdout)
            .contains("Release work")
    );
    assert_eq!(
        fs::read_to_string(project).unwrap(),
        "user project stays unchanged\n"
    );
    assert!(
        history_action(root, &["antigravity", "recovery", "restore", &id])
            .output()
            .unwrap()
            .status
            .success(),
        "restore is idempotent"
    );
    assert!(
        !cli(root, &["antigravity", "recovery", "purge", &id])
            .status
            .success()
    );
    assert!(
        cli(root, &["antigravity", "recovery", "purge", &id, "--yes"])
            .status
            .success()
    );
    assert!(!root.join("config/recovery/antigravity").join(id).exists());
}

#[test]
fn antigravity_deletion_fails_closed_on_active_busy_corrupt_or_unknown_stores() {
    for scenario in ["active", "busy", "corrupt cache", "unknown table"] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let conn = antigravity_fixture(root);
        match scenario {
            "active"=>conn.execute_batch("ALTER TABLE conversation_summaries ADD COLUMN not_fully_idle INTEGER DEFAULT 1").unwrap(),
            "busy"=>conn.execute_batch("BEGIN IMMEDIATE").unwrap(),
            "corrupt cache"=>{fs::create_dir(root.join("sessions/cache")).unwrap();fs::write(root.join("sessions/cache/conversation_metadata.json"),b"{ damaged").unwrap();},
            "unknown table"=>conn.execute_batch("CREATE TABLE future_links (conversation_id TEXT)").unwrap(),
            _=>unreachable!(),
        }
        let path = root.join("sessions/conversations/ag-chat.db");
        let before = fs::read(&path).unwrap();
        let output = history_action(root, &["antigravity", "delete", "ag-chat", "--yes"])
            .output()
            .unwrap();
        assert!(!output.status.success(), "{scenario}");
        assert_eq!(fs::read(path).unwrap(), before);
        assert!(
            !root.join("config/recovery/antigravity").exists(),
            "{scenario}"
        );
    }
}

fn claude_fixture(root: &Path) {
    let project = root.join("project");
    fs::create_dir(&project).unwrap();
    fs::write(project.join("keep.rs"), "keep user code").unwrap();
    for (relative, prompt) in [
        ("root-chat.jsonl", "Main project work"),
        ("root-chat/subagents/agent-audit.jsonl", "Audit work"),
        (
            "root-chat/subagents/workflows/run/agent-review.jsonl",
            "Review work",
        ),
    ] {
        let path = root.join("sessions/proj").join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path,format!("{}\n",json!({"sessionId":"root-chat","cwd":project,"timestamp":"2026-01-01T12:00:00Z","type":"user","message":{"content":prompt}}))).unwrap();
    }
    fs::write(
        root.join("sessions/proj/root-chat/subagents/workflows/run/agent-review.meta.json"),
        json!({"parentAgentId":"audit","toolUseId":"tool-1"}).to_string(),
    )
    .unwrap();
}

#[test]
fn claude_agent_grouping_sidecar_refresh_delete_and_recovery_work_together() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    claude_fixture(root);
    let collapsed = cli(root, &["claude", "__query"]);
    assert!(collapsed.status.success());
    assert!(!String::from_utf8_lossy(&collapsed.stdout).contains("S:claude:root-chat:agent:"));
    let review = "root-chat:agent:workflows/run/agent-review.jsonl";
    let preview = cli(
        root,
        &["claude", "__preview", &format!("S:claude:{review}")],
    );
    assert!(String::from_utf8_lossy(&preview.stdout).contains("Depth:    2"));
    fs::write(
        root.join("sessions/proj/root-chat/subagents/workflows/run/agent-review.meta.json"),
        json!({"parentAgentId":""}).to_string(),
    )
    .unwrap();
    let preview = cli(
        root,
        &["claude", "__preview", &format!("S:claude:{review}")],
    );
    assert!(String::from_utf8_lossy(&preview.stdout).contains("Depth:    1"));
    assert!(
        cli(
            root,
            &["claude", "rename", "root-chat", "--name", "Claude work"]
        )
        .status
        .success()
    );
    let plan = cli(root, &["claude", "cleanup", "--agents", "--dry-run"]);
    assert!(plan.status.success());
    assert!(String::from_utf8_lossy(&plan.stdout).contains("2 sessions"));
    let deleted = history_action(root, &["claude", "delete", "root-chat", "--yes"])
        .output()
        .unwrap();
    assert!(
        deleted.status.success(),
        "{}",
        String::from_utf8_lossy(&deleted.stderr)
    );
    assert!(!root.join("sessions/proj/root-chat.jsonl").exists());
    assert!(
        !root
            .join("sessions/proj/root-chat/subagents/agent-audit.jsonl")
            .exists()
    );
    let id = only_backup(root, "claude");
    let restored = history_action(root, &["claude", "recovery", "restore", &id])
        .output()
        .unwrap();
    assert!(
        restored.status.success(),
        "{}",
        String::from_utf8_lossy(&restored.stderr)
    );
    assert!(
        root.join("sessions/proj/root-chat/subagents/workflows/run/agent-review.meta.json")
            .exists()
    );
    assert!(
        String::from_utf8_lossy(&cli(root, &["claude", "__query"]).stdout).contains("Claude work")
    );
    assert_eq!(
        fs::read_to_string(root.join("project/keep.rs")).unwrap(),
        "keep user code"
    );
    fs::write(
        root.join("sessions/proj/root-chat.jsonl"),
        "newer user transcript",
    )
    .unwrap();
    assert!(
        !history_action(root, &["claude", "recovery", "restore", &id])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(
        fs::read_to_string(root.join("sessions/proj/root-chat.jsonl")).unwrap(),
        "newer user transcript"
    );
}

#[cfg(unix)]
#[test]
fn active_producer_or_history_symlink_blocks_local_deletion() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let _conn = antigravity_fixture(root);
    let mut command = history_action(root, &["antigravity", "delete", "ag-chat", "--yes"]);
    let ps = root.join("process-probe/ps");
    fs::write(&ps, "#!/bin/sh\nprintf 'agy agy\\n'\n").unwrap();
    fs::set_permissions(ps, fs::Permissions::from_mode(0o755)).unwrap();
    let output = command.output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Close Antigravity"));
    let path = root.join("sessions/conversations/ag-chat.db");
    fs::remove_file(&path).unwrap();
    let outside = root.join("project-file.db");
    fs::write(&outside, b"unrelated project file").unwrap();
    std::os::unix::fs::symlink(&outside, &path).unwrap();
    assert!(
        !history_action(root, &["antigravity", "delete", "ag-chat", "--yes"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(fs::read(outside).unwrap(), b"unrelated project file");
}

#[test]
fn damaged_or_relocated_recovery_is_refused_without_partial_restore_and_can_be_purged() {
    for scenario in ["corrupt archive", "wrong root", "unsafe path"] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        claude_fixture(root);
        let deleted = history_action(root, &["claude", "delete", "root-chat", "--yes"])
            .output()
            .unwrap();
        assert!(
            deleted.status.success(),
            "{}",
            String::from_utf8_lossy(&deleted.stderr)
        );
        let id = only_backup(root, "claude");
        let dir = root.join("config/recovery/claude").join(&id);
        match scenario {
            "corrupt archive" => fs::write(dir.join("history.tar.gz"), b"damaged backup").unwrap(),
            "wrong root" => {
                let other = root.join("other-store");
                fs::create_dir(&other).unwrap();
                let path = dir.join("manifest.json");
                let mut manifest: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                manifest["root"] = json!(other.canonicalize().unwrap());
                fs::write(path, manifest.to_string()).unwrap();
            }
            "unsafe path" => {
                let path = dir.join("manifest.json");
                let mut manifest: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                manifest["files"][0]["relative"] = json!("../project/keep.rs");
                fs::write(path, manifest.to_string()).unwrap();
            }
            _ => unreachable!(),
        }
        let restored = history_action(root, &["claude", "recovery", "restore", &id])
            .output()
            .unwrap();
        assert!(!restored.status.success(), "{scenario}");
        assert!(
            !root.join("sessions/proj/root-chat.jsonl").exists(),
            "{scenario}"
        );
        assert!(
            !root
                .join("sessions/proj/root-chat/subagents/agent-audit.jsonl")
                .exists(),
            "{scenario}"
        );
        assert_eq!(
            fs::read_to_string(root.join("project/keep.rs")).unwrap(),
            "keep user code"
        );
        assert!(dir.exists());
        // Even a damaged archive can be explicitly purged. An invalid manifest
        // is deliberately left untouched, since its scope cannot be validated.
        if scenario == "corrupt archive" {
            let purged = cli(root, &["claude", "recovery", "purge", &id, "--yes"]);
            assert!(purged.status.success());
            assert!(!dir.exists());
        }
    }
}

#[cfg(unix)]
#[test]
fn claude_chat_cleanup_requires_confirmation_and_keeps_a_recoverable_copy() {
    use std::{io::Write, os::unix::fs::PermissionsExt, process::Stdio};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    claude_fixture(root);
    for answer in ["cancel\n", "DELETE\n"] {
        let mut command = history_action(
            root,
            &["claude", "cleanup", "--chats", "--older-than", "0d"],
        );
        let fzf = root.join("process-probe/fzf");
        fs::write(&fzf, "#!/bin/sh\nif [ \"${1-}\" = \"--version\" ]; then printf '0.67.0\\n'; exit 0; fi\nIFS= read -r row\nprintf '%s\\n' \"$row\"\n").unwrap();
        fs::set_permissions(&fzf, fs::Permissions::from_mode(0o755)).unwrap();
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(answer.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if answer == "cancel\n" {
            assert!(root.join("sessions/proj/root-chat.jsonl").exists());
            assert!(!root.join("config/recovery/claude").exists());
        }
    }
    assert!(!root.join("sessions/proj/root-chat.jsonl").exists());
    assert!(
        !root
            .join("sessions/proj/root-chat/subagents/agent-audit.jsonl")
            .exists()
    );
    let id = only_backup(root, "claude");
    assert!(
        history_action(root, &["claude", "recovery", "restore", &id])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(root.join("sessions/proj/root-chat.jsonl").exists());
    assert_eq!(
        fs::read_to_string(root.join("project/keep.rs")).unwrap(),
        "keep user code"
    );
}

#[test]
fn alternate_opencode_store_is_browse_only_and_native_delete_is_not_invoked() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::create_dir(root.join("sessions")).unwrap();
    let path = root.join("sessions/opencode.db");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE session (id TEXT PRIMARY KEY, slug TEXT, directory TEXT, title TEXT, time_created INTEGER, time_updated INTEGER, time_archived INTEGER, workspace_id TEXT, parent_id TEXT);
        CREATE TABLE workspace (id TEXT, branch TEXT);
        CREATE TABLE message (id TEXT, session_id TEXT, time_created INTEGER, data TEXT);
        CREATE TABLE part (id TEXT, message_id TEXT, time_created INTEGER, data TEXT);
        INSERT INTO session VALUES ('op-chat','chat','','Local history',1,1,NULL,NULL,NULL);").unwrap();
    let command = |args: &[&str]| {
        let mut command = cli_command(root, args);
        command.env(
            "CODEX_SESSION_HUB_OPENCODE_SESSION_ROOT",
            root.join("sessions"),
        );
        command
    };
    let query = command(&["opencode", "__query"]).output().unwrap();
    assert!(query.status.success());
    assert!(String::from_utf8_lossy(&query.stdout).contains("Local history"));
    let before = fs::read(&path).unwrap();
    let deleted = command(&["opencode", "delete", "op-chat", "--yes"])
        .output()
        .unwrap();
    assert!(!deleted.status.success());
    assert!(
        String::from_utf8_lossy(&deleted.stderr)
            .contains("alternate hub session roots are browse-only")
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    let cleanup = command(&["opencode", "cleanup", "--chats", "--dry-run"])
        .output()
        .unwrap();
    assert!(cleanup.status.success());
    assert!(String::from_utf8_lossy(&cleanup.stdout).contains("No cleanup candidates"));
}

#[test]
fn antigravity_recovery_preserves_committed_messages_in_wal_companions() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let _index = antigravity_fixture(root);
    // Create a stable snapshot with an uncheckpointed committed WAL, then copy
    // all three files to an offline conversation store (no writer owns it).
    let source = rusqlite::Connection::open(root.join("source.db")).unwrap();
    source
        .execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;
        CREATE TABLE steps (idx INTEGER PRIMARY KEY, step_payload BLOB);
        INSERT INTO steps VALUES (1,x'6c6174657374206d657373616765');",
        )
        .unwrap();
    for suffix in ["", "-wal", "-shm"] {
        fs::copy(
            root.join(format!("source.db{suffix}")),
            root.join(format!("sessions/conversations/ag-chat.db{suffix}")),
        )
        .unwrap();
    }
    let before = ["", "-wal", "-shm"].map(|suffix| {
        fs::read(root.join(format!("sessions/conversations/ag-chat.db{suffix}"))).unwrap()
    });
    let deleted = history_action(root, &["antigravity", "delete", "ag-chat", "--yes"])
        .output()
        .unwrap();
    assert!(
        deleted.status.success(),
        "{}",
        String::from_utf8_lossy(&deleted.stderr)
    );
    let id = only_backup(root, "antigravity");
    let restored = history_action(root, &["antigravity", "recovery", "restore", &id])
        .output()
        .unwrap();
    assert!(
        restored.status.success(),
        "{}",
        String::from_utf8_lossy(&restored.stderr)
    );
    for (index, suffix) in ["", "-wal", "-shm"].into_iter().enumerate() {
        assert_eq!(
            fs::read(root.join(format!("sessions/conversations/ag-chat.db{suffix}"))).unwrap(),
            before[index]
        );
    }
    let restored =
        rusqlite::Connection::open(root.join("sessions/conversations/ag-chat.db")).unwrap();
    assert_eq!(
        restored
            .query_row("SELECT step_payload FROM steps WHERE idx=1", [], |r| r
                .get::<_, Vec<u8>>(0))
            .unwrap(),
        b"latest message"
    );
}

#[test]
fn antigravity_queries_previews_and_aliases_leave_conversation_data_untouched() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let connection = antigravity_fixture(root);
    let index_path = root.join("sessions/conversation_summaries.db");
    let session_path = root.join("sessions/conversations/ag-chat.db");
    let index_before = fs::read(&index_path).unwrap();
    let session_before = fs::read(&session_path).unwrap();
    let output = cli(root, &["antigravity", "__query"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows = String::from_utf8(output.stdout).unwrap();
    assert!(rows.contains("S:antigravity:ag-chat"));
    assert!(rows.contains("Antigravity release plan"));
    assert!(rows.contains("Sicilia café project"));
    let preview = cli(root, &["agx", "__preview", "S:antigravity:ag-chat"]);
    assert!(preview.status.success());
    let preview = String::from_utf8(preview.stdout).unwrap();
    assert!(preview.contains("Review the new release together"));
    assert!(preview.contains("Size:"));
    assert!(!preview.contains("Started:"));
    assert!(
        cli(
            root,
            &[
                "antigravity",
                "rename",
                "ag-chat",
                "--name",
                "AG local alias"
            ]
        )
        .status
        .success()
    );
    let output = cli(root, &["agx", "__query", "title:AG local alias"]);
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("AG local alias")
    );
    assert!(root.join("config/antigravity-index.json").is_file());
    assert!(
        cli(root, &["antigravity", "reset", "ag-chat"])
            .status
            .success()
    );
    assert!(
        String::from_utf8(cli(root, &["antigravity", "__query"]).stdout)
            .unwrap()
            .contains("Antigravity release plan")
    );
    assert!(
        !cli(root, &["antigravity", "delete", "ag-chat"])
            .status
            .success()
    );
    assert!(
        cli(root, &["antigravity", "cleanup", "--dry-run"])
            .status
            .success()
    );
    assert_eq!(fs::read(index_path).unwrap(), index_before);
    assert_eq!(fs::read(session_path).unwrap(), session_before);
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM conversation_summaries", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(!root.join("cache/update-check.json").exists());
}

#[test]
fn antigravity_launcher_identity_and_provider_diagnostics_are_correct() {
    let temp = tempfile::tempdir().unwrap();
    let launcher = temp
        .path()
        .join(if cfg!(windows) { "agx.exe" } else { "agx" });
    // This tests argv[0], not installed-copy migration (covered separately).
    // A symlink avoids executing a just-written inode while concurrent POSIX
    // test subprocesses may briefly inherit its writable descriptor.
    #[cfg(unix)]
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_agent-session-hub"), &launcher).unwrap();
    #[cfg(not(unix))]
    fs::copy(env!("CARGO_BIN_EXE_agent-session-hub"), &launcher).unwrap();
    let help = Command::new(&launcher).arg("help").output().unwrap();
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("agx browse"));
    assert!(help.contains("agx delete"));
    assert!(help.contains("agx cleanup"));
    let doctor = cli(temp.path(), &["agx", "doctor"]);
    assert!(doctor.status.success());
    let doctor = String::from_utf8(doctor.stdout).unwrap();
    assert!(doctor.contains("Provider: antigravity"));
    assert!(doctor.contains("BinaryName: agy"));
    let providers = cli(temp.path(), &["providers"]);
    assert!(
        String::from_utf8(providers.stdout)
            .unwrap()
            .contains("agx => Antigravity CLI (antigravity)")
    );
}

#[cfg(unix)]
#[test]
fn antigravity_browse_resumes_with_the_documented_arguments_and_workspace() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let _connection = antigravity_fixture(root);
    let bin = root.join("bin");
    fs::create_dir(&bin).unwrap();
    for (name, content) in [
        (
            "fzf",
            "#!/bin/sh\nprintf '\\nenter\\nS:antigravity:ag-chat\\n'\n",
        ),
        (
            "agy",
            "#!/bin/sh\nprintf 'ARG1=%s\\nARG2=%s\\nCWD=%s\\n' \"$1\" \"$2\" \"$PWD\"\n",
        ),
    ] {
        let path = bin.join(name);
        fs::write(&path, content).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap();
    let output = cli_command(root, &["antigravity", "browse"])
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result = String::from_utf8(output.stdout).unwrap();
    assert!(result.contains("ARG1=--conversation\nARG2=ag-chat\n"));
    let cwd = result
        .lines()
        .find_map(|line| line.strip_prefix("CWD="))
        .unwrap();
    assert_eq!(
        Path::new(cwd).canonicalize().unwrap(),
        root.join("Sicilia café project").canonicalize().unwrap()
    );
    let invalid = cli_command(root, &["antigravity", "--resume", "--help"])
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
}

#[test]
fn corrupt_alias_index_blocks_mutations_without_touching_sessions_or_aliases() {
    for command in [
        vec!["rename", "chat", "--name", "New title"],
        vec!["reset", "chat"],
        vec!["delete", "chat"],
        vec!["cleanup", "--dry-run"],
        vec!["__query"],
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir(root.join("sessions")).unwrap();
        fs::create_dir(root.join("config")).unwrap();
        let session = root.join("sessions/chat.jsonl");
        write_session(
            &session,
            "chat",
            &root.join("project"),
            "Keep this conversation",
        );
        let before = fs::read(&session).unwrap();
        let index = root.join("config/index.json");
        let damaged = b"{\"providers\": { damaged";
        fs::write(&index, damaged).unwrap();
        let mut args = vec!["--provider", "codex"];
        args.extend(command);
        let output = cli(root, &args);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("file left unchanged"));
        assert_eq!(fs::read(index).unwrap(), damaged);
        assert_eq!(fs::read(session).unwrap(), before);
    }
}

#[test]
fn update_management_commands_do_not_prompt_for_a_provider_or_launch_the_picker() {
    let temp = tempfile::tempdir().unwrap();
    for args in [
        vec!["version"],
        vec!["--version"],
        vec!["-V"],
        vec!["-v"],
        vec!["codex", "version"],
        vec!["codex", "-v"],
        vec!["clx", "-v"],
        vec!["opencode", "-v"],
        vec!["antigravity", "-v"],
        vec!["--provider", "claude", "-v"],
        vec!["--provider", "claude", "update", "--help"],
        vec!["check-update", "--help"],
    ] {
        let output = cli(temp.path(), &args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!String::from_utf8_lossy(&output.stderr).contains("Select CLI"));
        assert!(!output.stdout.is_empty());
    }
    let output = cli(temp.path(), &["-v", "unexpected"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("version accepts no arguments"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Select CLI"));
    let output = cli(temp.path(), &["update", "--unexpected"]);
    assert!(!output.status.success());
    let output = cli(temp.path(), &["update", "--yes"]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("not a recorded installer-managed installation")
    );
    assert!(!temp.path().join("cache/update-check.json").exists());
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
