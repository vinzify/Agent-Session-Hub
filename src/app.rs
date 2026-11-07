use crate::browser::{
    confirm_delete, ensure_fzf, fzf_rows, parse_row_target, resolve_selected_sessions, run_fzf,
    scan_group_marker, write_preview,
};
use crate::cleanup::{CleanupGroup, CleanupPolicy, cleanup_groups, select_cleanup_groups};
use crate::config::AliasIndex;
use crate::formatting::format_bytes;
use crate::paths::{config_root, current_exe, powershell_profile_path, provider_session_root};
use crate::provider::ProviderKind;
use crate::session::{display_sessions, filter_display_sessions, find_session, load_sessions};
use crate::shell::{
    install_cmd_launchers, install_posix_shell_integration, install_powershell_shell_integration,
    uninstall_posix_shell_integration, uninstall_powershell_shell_integration,
};
use anyhow::{Context, Result, anyhow};
use chrono::{Duration, Local};
use std::collections::{BTreeSet, HashSet};
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::process::Command;

fn normalized_argv0(argv0: &str) -> String {
    let normalized = argv0.replace('\\', "/");
    normalized
        .rsplit('/')
        .next()
        .unwrap_or(argv0)
        .trim_end_matches(".exe")
        .to_ascii_lowercase()
}

fn is_generic_entrypoint(argv0: &str) -> bool {
    matches!(
        normalized_argv0(argv0).as_str(),
        "sessionhub" | "agent-session-hub"
    )
}

fn generic_provider(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::Codex => "codex",
        ProviderKind::Claude => "claude",
        ProviderKind::Opencode => "opencode",
        ProviderKind::Antigravity => "antigravity",
    }
}

fn generic_provider_from_arg(value: &str) -> Option<ProviderKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        "codex" | "csx" => Some(ProviderKind::Codex),
        "claude" | "clx" => Some(ProviderKind::Claude),
        "opencode" | "opx" => Some(ProviderKind::Opencode),
        "antigravity" | "agx" => Some(ProviderKind::Antigravity),
        _ => None,
    }
}

fn prompt_for_provider() -> Result<Option<ProviderKind>> {
    eprintln!("Select CLI to launch:");
    for (index, provider) in ProviderKind::all().into_iter().enumerate() {
        eprintln!(
            "  {}. {} => {} ({})",
            index + 1,
            provider.launcher_name(),
            provider.display_name(),
            generic_provider(provider)
        );
    }

    loop {
        eprint!("Choice [1-{},q]: ", ProviderKind::all().len());
        io::stderr().flush()?;
        let mut choice = String::new();
        io::stdin().read_line(&mut choice)?;
        let normalized = choice.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "" | "q" | "quit" | "exit" => return Ok(None),
            "1" | "csx" | "codex" => return Ok(Some(ProviderKind::Codex)),
            "2" | "clx" | "claude" => return Ok(Some(ProviderKind::Claude)),
            "3" | "opx" | "opencode" => return Ok(Some(ProviderKind::Opencode)),
            "4" | "agx" | "antigravity" => return Ok(Some(ProviderKind::Antigravity)),
            _ => eprintln!("Unknown choice: {normalized}"),
        }
    }
}

fn generic_providers() {
    println!("Available session commands:");
    for provider in ProviderKind::all() {
        let availability = if which::which(provider.binary_name()).is_ok() {
            "available"
        } else {
            "missing"
        };
        println!(
            "{} => {} ({}) [{}]",
            provider.launcher_name(),
            provider.display_name(),
            generic_provider(provider),
            availability
        );
    }
}

fn generic_usage() -> Result<()> {
    println!("sessionhub");
    println!("sessionhub providers");
    println!("sessionhub <codex|claude|opencode|antigravity> [query]");
    println!("sessionhub <csx|clx|opx|agx> [query]");
    println!("sessionhub install-shell");
    println!("sessionhub uninstall-shell");
    println!("sessionhub version | --version | -V | -v");
    println!("sessionhub check-update");
    println!("sessionhub update [--yes]");
    println!();
    generic_providers();
    println!();
    println!(
        "Use `csx help`, `clx help`, `opx help`, or `agx help` for provider-specific commands."
    );
    Ok(())
}

fn generic_dispatch(args: &[String]) -> Result<()> {
    if args.is_empty() {
        let Some(provider) = prompt_for_provider()? else {
            return Ok(());
        };
        return dispatch(provider, &[]);
    }

    if args.first().map(|value| value.as_str()) == Some("--provider") {
        if args.len() < 2 {
            return Err(anyhow!("--provider requires a value"));
        }
        let provider = ProviderKind::parse(&args[1])
            .ok_or_else(|| anyhow!("unsupported provider: {}", args[1]))?;
        return dispatch(provider, &args[2..]);
    }

    match args[0].as_str() {
        "help" | "--help" | "-h" => generic_usage(),
        "providers" => {
            generic_providers();
            Ok(())
        }
        "doctor" => {
            generic_providers();
            println!();
            println!(
                "Use `csx doctor`, `clx doctor`, `opx doctor`, or `agx doctor` for provider-specific diagnostics."
            );
            Ok(())
        }
        "install-shell" => install_shell_command(ProviderKind::Codex),
        "uninstall-shell" => uninstall_shell_command(ProviderKind::Codex),
        _ => {
            let explicit = generic_provider_from_arg(&args[0]);
            let provider = match explicit {
                Some(provider) => Some(provider),
                None => prompt_for_provider()?,
            };
            let Some(provider) = provider else {
                return Ok(());
            };
            let provider_args = if explicit.is_some() { &args[1..] } else { args };
            dispatch(provider, provider_args)
        }
    }
}

pub fn run() -> Result<()> {
    let argv = env::args().collect::<Vec<_>>();
    let argv0 = argv
        .first()
        .cloned()
        .unwrap_or_else(|| "agent-session-hub".to_string());
    let mut args = argv.into_iter().skip(1).collect::<Vec<_>>();

    crate::update::migrate_on_start(&args);
    if let Some(result) = crate::update::dispatch(&args) {
        return result;
    }

    if is_generic_entrypoint(&argv0) {
        return generic_dispatch(&args);
    }

    let inferred_provider = ProviderKind::alias_from_argv0(&argv0);
    let mut provider = inferred_provider;
    if args.first().map(|value| value.as_str()) == Some("--provider") {
        if args.len() < 2 {
            return Err(anyhow!("--provider requires a value"));
        }
        provider = ProviderKind::parse(&args[1])
            .ok_or_else(|| anyhow!("unsupported provider: {}", args[1]))?;
        args.drain(0..2);
    }

    dispatch(provider, &args)
}

fn dispatch(provider: ProviderKind, args: &[String]) -> Result<()> {
    if let Some(result) = crate::update::dispatch(args) {
        return result;
    }
    if args.is_empty() {
        return browse_command(provider, "");
    }

    match args[0].as_str() {
        "__preview" => hidden_preview(provider, &args[1..]),
        "__query" => hidden_query(provider, &args[1..]),
        "__select" => hidden_select(provider, &args[1..]),
        "browse" => browse_command(provider, &args[1..].join(" ")),
        "rename" => rename_command(provider, &args[1..]),
        "reset" => reset_command(provider, &args[1..]),
        "delete" => delete_command(provider, &args[1..]),
        "cleanup" => cleanup_command(provider, &args[1..]),
        "recovery" => recovery_command(provider, &args[1..]),
        "doctor" => doctor_command(provider),
        "install-shell" => install_shell_command(provider),
        "uninstall-shell" => uninstall_shell_command(provider),
        "help" | "--help" | "-h" => usage(provider),
        "--resume" => {
            if args.len() < 2 {
                return Err(anyhow!("--resume requires a session id"));
            }
            let session_id = &args[1];
            resume_provider(provider, session_id, None)
        }
        _ => browse_command(provider, &args.join(" ")),
    }
}

fn load_index_and_sessions(
    provider: ProviderKind,
) -> Result<(AliasIndex, Vec<crate::session::SessionRecord>)> {
    let index = AliasIndex::load(provider)?;
    let sessions = load_sessions(provider, &index)?;
    Ok((index, sessions))
}

fn delete_session(
    provider: ProviderKind,
    session: &crate::session::SessionRecord,
    index: &mut AliasIndex,
) -> Result<()> {
    if provider == ProviderKind::Opencode {
        anyhow::ensure!(
            Some(crate::paths::provider_session_root(provider).canonicalize()?)
                == provider.default_session_root().canonicalize().ok(),
            "OpenCode deletion through its native CLI requires the default store; alternate hub session roots are browse-only"
        );
    }
    anyhow::ensure!(
        session.supports_delete,
        "session is not eligible for hub deletion (active, alternate store, or hidden archived descendants); manage it through the provider's own history controls"
    );
    match provider {
        ProviderKind::Codex => {
            fs::remove_file(&session.file_path)
                .with_context(|| format!("remove {}", session.file_path.display()))?;
        }
        ProviderKind::Claude | ProviderKind::Antigravity => {
            return Err(anyhow!(
                "{} session delete is not supported.",
                provider.display_name()
            ));
        }
        ProviderKind::Opencode => {
            let binary =
                which::which(provider.binary_name()).context("opencode was not found in PATH")?;
            let status = Command::new(binary)
                .arg("session")
                .arg("delete")
                .arg(&session.session_id)
                .status()
                .context("run opencode session delete")?;
            if !status.success() {
                return Err(anyhow!("opencode session delete failed"));
            }
        }
    }
    index.remove_alias(provider, &session.session_id);
    Ok(())
}

fn interactive_delete(
    provider: ProviderKind,
    selected: &[&crate::session::DisplaySession],
) -> Result<()> {
    if !provider.supports_delete() {
        return Err(anyhow!(
            "{} session delete is not supported.",
            provider.display_name()
        ));
    }
    let mut index = AliasIndex::load(provider)?;
    let targets = selected
        .iter()
        .map(|entry| entry.session.clone())
        .collect::<Vec<_>>();
    if !confirm_delete(&targets)? {
        return Ok(());
    }
    delete_batch(provider, &targets, &mut index)?;
    index.save(provider)
}

fn delete_batch(
    provider: ProviderKind,
    targets: &[crate::session::SessionRecord],
    index: &mut AliasIndex,
) -> Result<()> {
    if targets.is_empty() {
        return Ok(());
    }
    let backup = match provider {
        ProviderKind::Claude => Some(crate::local_store::claude_delete(targets)?),
        ProviderKind::Antigravity => Some(crate::antigravity::delete(targets)?),
        _ => None,
    };
    if let Some(id) = backup {
        for session in targets {
            index.remove_alias(provider, &session.session_id);
        }
        eprintln!(
            "Local CLI history removed. Recovery backup: {id}\nRestore: sessionhub {} recovery restore {id}\nBackup still uses disk space; purge it explicitly when no longer needed.",
            provider.name()
        );
    } else {
        // Delete deepest children first: OpenCode's native delete may cascade.
        let mut ordered = targets.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|s| std::cmp::Reverse(s.agent_depth));
        let mut errors = Vec::new();
        for session in ordered {
            if let Err(error) = delete_session(provider, session, index) {
                errors.push(format!("{}: {error:#}", session.session_id));
            }
        }
        if !errors.is_empty() {
            index.save(provider)?;
            return Err(anyhow!("some deletions failed: {}", errors.join("; ")));
        }
    }
    Ok(())
}

fn recovery_command(provider: ProviderKind, args: &[String]) -> Result<()> {
    use crate::local_store::{Backup, recovery_root};
    match args.first().map(String::as_str).unwrap_or("list") {
        "list" => {
            anyhow::ensure!(args.len() <= 1, "recovery list accepts no arguments");
            let root = recovery_root(provider);
            if !root.try_exists()? {
                println!("No recovery backups.");
                return Ok(());
            }
            for entry in fs::read_dir(root)? {
                let entry = entry?;
                let id = entry.file_name().to_string_lossy().into_owned();
                if let Ok(backup) = Backup::load(provider, &id) {
                    let size = ["history.tar.gz", "manifest.json"]
                        .iter()
                        .map(|name| fs::metadata(backup.dir.join(name)).map(|m| m.len()))
                        .collect::<std::io::Result<Vec<_>>>();
                    if let Ok(sizes) = size {
                        println!(
                            "{id}  {}  {} stored  {} history files",
                            backup.manifest.state,
                            format_bytes(sizes.iter().sum()),
                            backup.manifest.files.len()
                        );
                    } else {
                        eprintln!("[incomplete] recovery {id}; left untouched");
                    }
                } else {
                    eprintln!("[unreadable] recovery {id}; left untouched")
                }
            }
            Ok(())
        }
        "restore" => {
            anyhow::ensure!(args.len() == 2, "use recovery restore <backup-id>");
            let id = args
                .get(1)
                .ok_or_else(|| anyhow!("recovery restore requires a backup ID"))?;
            let mut backup = Backup::load(provider, id)?;
            let mut index = AliasIndex::load(provider)?;
            crate::local_store::ensure_idle(provider)?;
            match provider {
                ProviderKind::Antigravity => crate::antigravity::restore(&backup)?,
                ProviderKind::Claude => {
                    backup.restore_files()?;
                }
                _ => {
                    return Err(anyhow!(
                        "no recoverable local-history adapter for this provider"
                    ));
                }
            }
            for (id, alias) in &backup.manifest.aliases {
                if index.get_alias(provider, id).is_empty() {
                    index.set_alias(provider, id, alias);
                }
            }
            index.save(provider)?;
            backup.mark("restored")?;
            println!(
                "Restored local CLI history from {id}. Backup retained until explicitly purged."
            );
            Ok(())
        }
        "purge" => {
            anyhow::ensure!(
                (2..=3).contains(&args.len()) && args.iter().skip(2).all(|a| a == "--yes"),
                "use recovery purge <backup-id> [--yes]"
            );
            let id = args
                .get(1)
                .ok_or_else(|| anyhow!("recovery purge requires a backup ID"))?;
            let backup = Backup::load(provider, id)?;
            if !args.iter().any(|a| a == "--yes") {
                anyhow::ensure!(
                    io::stdin().is_terminal(),
                    "purge requires confirmation; use --yes only after reviewing the backup"
                );
                eprint!("Permanently remove recovery {id}? Type PURGE: ");
                io::stderr().flush()?;
                let mut answer = String::new();
                io::stdin().read_line(&mut answer)?;
                if answer.trim() != "PURGE" {
                    println!("Purge cancelled.");
                    return Ok(());
                }
            }
            fs::remove_dir_all(&backup.dir)?;
            println!("Permanently purged recovery {id}; it cannot be restored.");
            Ok(())
        }
        "help" | "--help" | "-h" => {
            println!(
                "recovery list | restore <backup-id> | purge <backup-id> [--yes]\nRecovery backups contain private history and use disk space until purged."
            );
            Ok(())
        }
        _ => Err(anyhow!(
            "use recovery list | restore <backup-id> | purge <backup-id> [--yes]"
        )),
    }
}

fn with_connected_agents<'a>(
    display: &'a [crate::session::DisplaySession],
    selected: &[&'a crate::session::DisplaySession],
) -> Vec<&'a crate::session::DisplaySession> {
    let mut ids = BTreeSet::new();
    for entry in selected {
        ids.insert(entry.session.session_id.clone());
        if !entry.session.is_agent {
            ids.extend(
                display
                    .iter()
                    .filter(|candidate| {
                        candidate.session.is_agent
                            && candidate.session.root_session_id == entry.session.session_id
                    })
                    .map(|candidate| candidate.session.session_id.clone()),
            );
        }
    }
    if display
        .first()
        .is_some_and(|entry| entry.session.provider != ProviderKind::Codex)
    {
        loop {
            let before = ids.len();
            for entry in display {
                if entry.session.is_agent && ids.contains(&entry.session.parent_session_id) {
                    ids.insert(entry.session.session_id.clone());
                }
            }
            if ids.len() == before {
                break;
            }
        }
    }
    display
        .iter()
        .filter(|entry| ids.contains(&entry.session.session_id))
        .collect()
}

fn connected_chat<'a>(
    display: &'a [crate::session::DisplaySession],
    selected: &'a crate::session::DisplaySession,
) -> &'a crate::session::DisplaySession {
    if selected.session.is_agent {
        display
            .iter()
            .find(|entry| {
                !entry.session.is_agent
                    && entry.session.session_id == selected.session.root_session_id
            })
            .unwrap_or(selected)
    } else {
        selected
    }
}

fn toggle_agents(
    selected: &[&crate::session::DisplaySession],
    selected_ids: &[String],
    expanded_roots: &mut BTreeSet<String>,
) {
    if selected_ids.len() == 1
        && selected_ids[0].starts_with("W:")
        && let Some(group_key) = selected.first().map(|entry| entry.group_key.as_str())
        && group_key.starts_with("security-scan:")
    {
        let marker = scan_group_marker(group_key);
        if !expanded_roots.remove(&marker) {
            expanded_roots.insert(marker);
        }
        return;
    }
    let roots = selected
        .iter()
        .map(|entry| {
            if entry.session.is_agent {
                entry.session.root_session_id.clone()
            } else {
                entry.session.session_id.clone()
            }
        })
        .collect::<BTreeSet<_>>();
    if roots.iter().all(|root| expanded_roots.contains(root)) {
        for root in roots {
            expanded_roots.remove(&root);
        }
    } else {
        expanded_roots.extend(roots);
    }
}

fn interactive_rename(
    provider: ProviderKind,
    selected: &crate::session::DisplaySession,
) -> Result<()> {
    eprint!(
        "Rename title for #{} in {} (blank resets): ",
        selected.display_number, selected.session.project_name
    );
    io::stderr().flush()?;
    let mut alias = String::new();
    io::stdin().read_line(&mut alias)?;
    let mut index = AliasIndex::load(provider)?;
    index.set_alias(provider, &selected.session.session_id, alias.trim());
    index.save(provider)
}

fn interactive_reset(
    provider: ProviderKind,
    selected: &crate::session::DisplaySession,
) -> Result<()> {
    let mut index = AliasIndex::load(provider)?;
    index.remove_alias(provider, &selected.session.session_id);
    index.save(provider)
}

fn is_single_workspace_selection(selected_ids: &[String]) -> bool {
    selected_ids.len() == 1
        && selected_ids
            .first()
            .is_some_and(|value| value.starts_with("W:"))
}

fn browse_command(provider: ProviderKind, query: &str) -> Result<()> {
    let notice = crate::update::Notice::start();
    ensure_fzf()?;
    let exe = current_exe()?;
    let query = query.to_string();
    let mut expanded_roots = BTreeSet::new();

    loop {
        let (_index, sessions) = load_index_and_sessions(provider)?;
        let display = display_sessions(&sessions);
        let filtered = filter_display_sessions(&sessions, &query);
        let rows = fzf_rows(
            &filtered,
            &display,
            &expanded_roots,
            !query.trim().is_empty(),
        );
        if rows.is_empty() {
            return Ok(());
        }
        let Some(result) = run_fzf(provider, &query, &rows, &exe, &expanded_roots)? else {
            return Ok(());
        };
        let selected = resolve_selected_sessions(&display, &result.session_ids);
        if selected.is_empty() {
            continue;
        }

        match result.action.as_str() {
            "enter" => {
                if selected.len() > 1 && !is_single_workspace_selection(&result.session_ids) {
                    return Err(anyhow!(
                        "Resume only supports one session at a time. Clear multi-select or choose a single row."
                    ));
                }
                drop(notice);
                return resume_session(&connected_chat(&display, selected[0]).session);
            }
            "ctrl-a" => toggle_agents(&selected, &result.session_ids, &mut expanded_roots),
            "ctrl-d" => {
                let targets = with_connected_agents(&display, &selected);
                interactive_delete(provider, &targets)?
            }
            "ctrl-e" => interactive_rename(provider, selected[0])?,
            "ctrl-r" => interactive_reset(provider, selected[0])?,
            _ => return Err(anyhow!("Unsupported browser action: {}", result.action)),
        }
    }
}

fn rename_command(provider: ProviderKind, args: &[String]) -> Result<()> {
    if args.is_empty() {
        return Err(anyhow!("rename requires a session id."));
    }
    let name_index = args.iter().position(|value| value == "--name");
    let Some(name_index) = name_index else {
        return Err(anyhow!("rename requires --name <alias>."));
    };
    let alias = args
        .get(name_index + 1)
        .ok_or_else(|| anyhow!("rename requires --name <alias>."))?;
    let session_id = &args[0];
    let (_index, sessions) = load_index_and_sessions(provider)?;
    let display = display_sessions(&sessions);
    let session = find_session(&display, session_id, Some(provider))
        .ok_or_else(|| anyhow!("Session not found: {session_id}"))?;
    let mut index = AliasIndex::load(provider)?;
    index.set_alias(provider, &session.session.session_id, alias);
    index.save(provider)?;
    println!("Updated alias for {}", session.session.session_id);
    Ok(())
}

fn reset_command(provider: ProviderKind, args: &[String]) -> Result<()> {
    if args.is_empty() {
        return Err(anyhow!("reset requires a session id."));
    }
    let session_id = &args[0];
    let (_index, sessions) = load_index_and_sessions(provider)?;
    let display = display_sessions(&sessions);
    let session = find_session(&display, session_id, Some(provider))
        .ok_or_else(|| anyhow!("Session not found: {session_id}"))?;
    let mut index = AliasIndex::load(provider)?;
    index.remove_alias(provider, &session.session.session_id);
    index.save(provider)?;
    println!("Reset alias for {}", session.session.session_id);
    Ok(())
}

fn delete_command(provider: ProviderKind, args: &[String]) -> Result<()> {
    if !provider.supports_delete() {
        return Err(anyhow!(
            "{} session delete is not supported.",
            provider.display_name()
        ));
    }
    let values = args
        .iter()
        .filter(|a| !matches!(a.as_str(), "--yes" | "--dry-run"))
        .cloned()
        .collect::<Vec<_>>();
    if values.is_empty() {
        return Err(anyhow!("delete requires at least one session id."));
    }
    let (_index, sessions) = load_index_and_sessions(provider)?;
    let display = display_sessions(&sessions);
    let selected = values
        .iter()
        .map(|value| {
            find_session(&display, value, Some(provider))
                .ok_or_else(|| anyhow!("No matching session found for {value}"))
        })
        .collect::<Result<Vec<_>>>()?;
    let target_refs = with_connected_agents(&display, &selected);
    let targets = target_refs
        .into_iter()
        .map(|entry| entry.session.clone())
        .collect::<Vec<_>>();
    if args.iter().any(|a| a == "--dry-run") {
        for session in targets {
            println!(
                "[would delete] {} {} {}",
                session.session_id,
                format_bytes(session.storage_bytes),
                session.display_title
            );
        }
        return Ok(());
    }
    if matches!(provider, ProviderKind::Claude | ProviderKind::Antigravity)
        && !args.iter().any(|a| a == "--yes")
    {
        anyhow::ensure!(
            io::stdin().is_terminal(),
            "delete requires confirmation; review with --dry-run, then use --yes or the interactive picker"
        );
        if !confirm_delete(&targets)? {
            println!("Deletion cancelled.");
            return Ok(());
        }
    }
    let mut index = AliasIndex::load(provider)?;
    delete_batch(provider, &targets, &mut index)?;
    for session in targets {
        println!("[deleted] {} Deleted from CLI history", session.session_id);
    }
    index.save(provider)
}

fn parse_cleanup_age(value: &str) -> Result<Duration> {
    let normalized = value.trim().to_ascii_lowercase();
    let (number, multiplier) = if let Some(value) = normalized.strip_suffix('h') {
        (value, 60 * 60)
    } else if let Some(value) = normalized.strip_suffix('w') {
        (value, 7 * 24 * 60 * 60)
    } else if let Some(value) = normalized.strip_suffix('d') {
        (value, 24 * 60 * 60)
    } else {
        (normalized.as_str(), 24 * 60 * 60)
    };
    let amount = number
        .parse::<i64>()
        .with_context(|| format!("invalid age: {value}"))?;
    if amount < 0 {
        return Err(anyhow!("cleanup age cannot be negative"));
    }
    amount
        .checked_mul(multiplier)
        .and_then(Duration::try_seconds)
        .filter(|age| Local::now().checked_sub_signed(*age).is_some())
        .ok_or_else(|| anyhow!("cleanup age is too large"))
}

fn cleanup_usage(provider: ProviderKind) {
    let launcher = provider.launcher_name();
    println!("{launcher} cleanup [options]");
    println!();
    println!("Options:");
    println!("  --dry-run            Print candidates without deleting");
    if provider == ProviderKind::Codex {
        println!("  --security-scans      Include generated security scans");
    }
    println!("  --agents              Include only old agents linked to retained chats");
    println!("  --orphans             Include only agents whose root chat is missing");
    println!("  --chats               Explicitly include old chats and their agents (default 90d)");
    println!("  --older-than <age>    Override retention age (examples: 12h, 7d, 4w)");
    println!();
    println!("Defaults: linked agents 30d, orphan agents 7d; Codex security scans 7d.");
    println!("Normal root chats are preserved unless --chats is explicitly supplied.");
}

fn parse_cleanup_options(args: &[String]) -> Result<(CleanupPolicy, bool)> {
    let category_filter = args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "--security-scans" | "--agents" | "--orphans" | "--chats"
        )
    });
    let mut policy = CleanupPolicy {
        include_security_scans: !category_filter,
        include_linked_agents: !category_filter,
        include_orphan_agents: !category_filter,
        ..CleanupPolicy::default()
    };
    let mut dry_run = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dry-run" => dry_run = true,
            "--security-scans" => policy.include_security_scans = true,
            "--agents" => policy.include_linked_agents = true,
            "--orphans" => policy.include_orphan_agents = true,
            "--chats" => policy.include_chats = true,
            "--older-than" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| anyhow!("--older-than requires an age"))?;
                let age = parse_cleanup_age(value)?;
                policy.security_scan_age = age;
                policy.linked_agent_age = age;
                policy.orphan_agent_age = age;
                policy.chat_age = age;
                index += 1;
            }
            "--help" | "-h" => {}
            unknown => return Err(anyhow!("unknown cleanup option: {unknown}")),
        }
        index += 1;
    }
    Ok((policy, dry_run))
}

fn print_cleanup_groups(groups: &[CleanupGroup]) {
    let total_sessions = groups
        .iter()
        .map(|group| group.session_ids.len())
        .sum::<usize>();
    let total_bytes = groups.iter().map(|group| group.storage_bytes).sum::<u64>();
    println!(
        "{} cleanup groups · {} sessions · {} in selected history (recovery/database overhead may remain)",
        groups.len(),
        total_sessions,
        format_bytes(total_bytes)
    );
    println!();
    let now = Local::now();
    for group in groups {
        println!(
            "{:<14} {:>9}  {:>5}d  {:>4} sessions  {}",
            group.kind.label(),
            format_bytes(group.storage_bytes),
            group.age_days(now),
            group.session_ids.len(),
            group.title
        );
        if !group.detail.trim().is_empty() {
            println!("  {}", group.detail);
        }
    }
}

fn confirm_cleanup(provider: ProviderKind, groups: &[&CleanupGroup]) -> Result<bool> {
    let sessions = groups
        .iter()
        .map(|group| group.session_ids.len())
        .sum::<usize>();
    let bytes = groups.iter().map(|group| group.storage_bytes).sum::<u64>();
    eprintln!();
    eprintln!(
        "Selected {} groups: {} session files, {}",
        groups.len(),
        sessions,
        format_bytes(bytes)
    );
    for group in groups.iter().take(8) {
        eprintln!(
            "  - {}: {} ({} sessions, {})",
            group.kind.label(),
            group.title,
            group.session_ids.len(),
            format_bytes(group.storage_bytes)
        );
    }
    if groups.len() > 8 {
        eprintln!("  - ... and {} more groups", groups.len() - 8);
    }
    eprintln!("Normal root chats are preserved by agent cleanup.");
    if groups
        .iter()
        .any(|g| g.kind == crate::cleanup::CleanupKind::Chats)
    {
        eprintln!("Explicitly selected old-chat groups remove the root chat and its agents.");
    }
    if provider == ProviderKind::Codex {
        eprintln!("Security-scan groups include their generated root and agent logs.");
    }
    if matches!(provider, ProviderKind::Claude | ProviderKind::Antigravity) {
        eprintln!(
            "Local CLI history only. A compressed recovery backup will remain until explicitly purged."
        );
    }
    eprint!("Type DELETE to remove the selected history: ");
    io::stderr().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim() == "DELETE")
}

fn cleanup_command(provider: ProviderKind, args: &[String]) -> Result<()> {
    anyhow::ensure!(
        provider == ProviderKind::Codex || !args.iter().any(|a| a == "--security-scans"),
        "security-scan grouping applies to Codex only"
    );
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        cleanup_usage(provider);
        return Ok(());
    }
    let (policy, dry_run) = parse_cleanup_options(args)?;
    let (_alias_index, sessions) = load_index_and_sessions(provider)?;
    let groups = cleanup_groups(&sessions, &policy, Local::now());
    if groups.is_empty() {
        println!("No cleanup candidates match the selected retention policy.");
        return Ok(());
    }
    if dry_run {
        print_cleanup_groups(&groups);
        return Ok(());
    }

    ensure_fzf()?;
    let selected_keys = select_cleanup_groups(&groups)?;
    if selected_keys.is_empty() {
        return Ok(());
    }
    let selected_key_set = selected_keys.into_iter().collect::<HashSet<_>>();
    let selected_groups = groups
        .iter()
        .filter(|group| selected_key_set.contains(&group.key))
        .collect::<Vec<_>>();
    if !confirm_cleanup(provider, &selected_groups)? {
        println!("Cleanup cancelled.");
        return Ok(());
    }

    // Selection and confirmation can take minutes. Recheck eligibility and the
    // group contents before deleting, so logs resumed in the meantime survive.
    let (mut alias_index, sessions) = load_index_and_sessions(provider)?;
    let current_groups = cleanup_groups(&sessions, &policy, Local::now());
    let unchanged_groups = selected_groups
        .iter()
        .filter(|group| {
            current_groups.iter().any(|current| {
                current.key == group.key
                    && current.latest_updated == group.latest_updated
                    && current.storage_bytes == group.storage_bytes
                    && current.session_ids.iter().collect::<HashSet<_>>()
                        == group.session_ids.iter().collect::<HashSet<_>>()
            })
        })
        .copied()
        .collect::<Vec<_>>();
    if unchanged_groups.len() != selected_groups.len() {
        eprintln!(
            "Skipped {} groups that changed while reviewing. Run cleanup again to review them.",
            selected_groups.len() - unchanged_groups.len()
        );
    }
    if unchanged_groups.is_empty() {
        println!("No unchanged cleanup candidates remain.");
        return Ok(());
    }
    let selected_ids = unchanged_groups
        .iter()
        .flat_map(|group| group.session_ids.iter().cloned())
        .collect::<HashSet<_>>();
    let mut targets = sessions
        .iter()
        .filter(|session| selected_ids.contains(&session.session_id))
        .collect::<Vec<_>>();
    targets.sort_by_key(|session| std::cmp::Reverse(session.agent_depth));
    if matches!(provider, ProviderKind::Claude | ProviderKind::Antigravity) {
        let targets = targets.into_iter().cloned().collect::<Vec<_>>();
        delete_batch(provider, &targets, &mut alias_index)?;
        alias_index.save(provider)?;
        println!(
            "Cleanup complete: {} sessions removed from local CLI history. Compressed recovery retained; purge it to reclaim that space.",
            targets.len()
        );
        return Ok(());
    }
    let mut deleted = 0usize;
    let mut reclaimed = 0u64;
    let mut failed = 0usize;
    for session in targets {
        match delete_session(provider, session, &mut alias_index) {
            Ok(()) => {
                deleted += 1;
                reclaimed += session.storage_bytes;
            }
            Err(error) => {
                failed += 1;
                eprintln!("[failed] {} {error}", session.session_id);
            }
        }
    }
    alias_index.save(provider)?;
    println!(
        "Cleanup complete: {deleted} sessions removed, {} {}{}.",
        format_bytes(reclaimed),
        if provider == ProviderKind::Opencode {
            "estimated payload removed (shared database may retain disk space)"
        } else {
            "of session files removed"
        },
        if failed == 0 {
            String::new()
        } else {
            format!(", {failed} failed")
        }
    );
    Ok(())
}

fn hidden_preview(provider: ProviderKind, args: &[String]) -> Result<()> {
    let (query, target) = if args.first().map(String::as_str) == Some("--query") {
        (args.get(1).map(String::as_str), args.get(2))
    } else {
        (None, args.first())
    };
    let (mut session_id, mut workspace_key, mut project_path) = if let Some(raw) = target {
        parse_row_target(raw)
    } else {
        (String::new(), String::new(), String::new())
    };
    let (_index, sessions) = load_index_and_sessions(provider)?;
    let display = display_sessions(&sessions);
    if let Some(query) = query {
        let filtered = filter_display_sessions(&sessions, query);
        // Older fzf releases may request a preview for the previous first row
        // while an asynchronous reload is replacing it. Resolve that row against
        // the current query, whose first visible row is a workspace header.
        let target_matches = filtered.iter().any(|entry| {
            (!session_id.is_empty() && entry.session.session_id == session_id)
                || (!workspace_key.is_empty() && entry.group_key == workspace_key)
                || (!project_path.is_empty() && entry.session.project_path == project_path)
        });
        if !target_matches {
            session_id.clear();
            project_path.clear();
            workspace_key = filtered
                .first()
                .map(|entry| entry.group_key.clone())
                .unwrap_or_default();
        }
    }
    write_preview(
        provider,
        &display,
        &session_id,
        &workspace_key,
        &project_path,
    )
}

fn hidden_query(provider: ProviderKind, args: &[String]) -> Result<()> {
    let (expanded_roots, query_args) = if args.first().map(String::as_str) == Some("--expanded") {
        let expanded = args
            .get(1)
            .filter(|value| value.as_str() != "-")
            .map(|value| {
                value
                    .split(',')
                    .filter(|id| !id.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        (expanded, &args[2.min(args.len())..])
    } else {
        (BTreeSet::new(), args)
    };
    let query = if !query_args.is_empty() {
        query_args.join(" ")
    } else {
        env::var("FZF_QUERY").unwrap_or_default()
    };
    let (_index, sessions) = load_index_and_sessions(provider)?;
    let display = display_sessions(&sessions);
    let filtered = filter_display_sessions(&sessions, &query);
    let stdout = io::stdout();
    let mut output = io::BufWriter::new(stdout.lock());
    for row in fzf_rows(
        &filtered,
        &display,
        &expanded_roots,
        !query.trim().is_empty(),
    ) {
        if let Err(error) = writeln!(output, "{row}") {
            if error.kind() == io::ErrorKind::BrokenPipe {
                return Ok(());
            }
            return Err(error.into());
        }
    }
    if let Err(error) = output.flush()
        && error.kind() != io::ErrorKind::BrokenPipe
    {
        return Err(error.into());
    }
    Ok(())
}

fn hidden_select(provider: ProviderKind, args: &[String]) -> Result<()> {
    let _notice = crate::update::Notice::start();
    ensure_fzf()?;
    let exe = current_exe()?;
    let query = args.join(" ");
    let mut expanded_roots = BTreeSet::new();

    loop {
        let (_index, sessions) = load_index_and_sessions(provider)?;
        let display = display_sessions(&sessions);
        let filtered = filter_display_sessions(&sessions, &query);
        let rows = fzf_rows(
            &filtered,
            &display,
            &expanded_roots,
            !query.trim().is_empty(),
        );
        if rows.is_empty() {
            return Ok(());
        }
        let Some(result) = run_fzf(provider, &query, &rows, &exe, &expanded_roots)? else {
            return Ok(());
        };
        let selected = resolve_selected_sessions(&display, &result.session_ids);
        if selected.is_empty() {
            continue;
        }

        match result.action.as_str() {
            "enter" => {
                if selected.len() > 1 && !is_single_workspace_selection(&result.session_ids) {
                    return Err(anyhow!(
                        "Resume only supports one session at a time. Clear multi-select or choose a single row."
                    ));
                }
                let selected = connected_chat(&display, selected[0]);
                anyhow::ensure!(
                    !(selected.session.provider == ProviderKind::Claude
                        && selected.session.is_agent),
                    "the parent Claude chat is missing; agent logs can be previewed or cleaned but not resumed alone"
                );
                println!(
                    "{}	{}",
                    selected.session.project_path, selected.session.session_id
                );
                return Ok(());
            }
            "ctrl-a" => toggle_agents(&selected, &result.session_ids, &mut expanded_roots),
            "ctrl-d" => {
                let targets = with_connected_agents(&display, &selected);
                interactive_delete(provider, &targets)?
            }
            "ctrl-e" => interactive_rename(provider, selected[0])?,
            "ctrl-r" => interactive_reset(provider, selected[0])?,
            _ => return Err(anyhow!("Unsupported browser action: {}", result.action)),
        }
    }
}

fn resume_session(session: &crate::session::SessionRecord) -> Result<()> {
    anyhow::ensure!(
        !(session.provider == ProviderKind::Claude && session.is_agent),
        "the parent Claude conversation is missing; this agent cannot be resumed alone"
    );
    resume_provider(
        session.provider,
        &session.session_id,
        if session.project_path.is_empty() {
            None
        } else {
            Some(&session.project_path)
        },
    )
}

fn resume_provider(
    provider: ProviderKind,
    session_id: &str,
    project_path: Option<&str>,
) -> Result<()> {
    let provider_binary = which::which(provider.binary_name())
        .with_context(|| format!("{} was not found in PATH.", provider.binary_name()))?;
    match provider {
        ProviderKind::Codex => {
            let mut command = Command::new(&provider_binary);
            command.arg("resume");
            if let Some(project_path) = project_path.filter(|value| !value.trim().is_empty()) {
                command.arg("--cd").arg(project_path);
            }
            command.arg(session_id);
            let status = command.status().context("run codex")?;
            if !status.success() {
                return Err(anyhow!("codex resume failed"));
            }
        }
        ProviderKind::Claude => {
            let mut command = Command::new(&provider_binary);
            command.arg("--resume").arg(session_id);
            if let Some(project_path) = project_path.filter(|value| !value.trim().is_empty()) {
                command.current_dir(project_path);
            }
            let status = command.status().context("run claude")?;
            if !status.success() {
                return Err(anyhow!("claude resume failed"));
            }
        }
        ProviderKind::Opencode => {
            let mut command = Command::new(&provider_binary);
            command.arg("--session").arg(session_id);
            if let Some(project_path) = project_path.filter(|value| !value.trim().is_empty()) {
                command.current_dir(project_path);
            }
            let status = command.status().context("run opencode")?;
            if !status.success() {
                return Err(anyhow!("opencode resume failed"));
            }
        }
        ProviderKind::Antigravity => {
            anyhow::ensure!(
                crate::antigravity::valid_id(session_id),
                "invalid Antigravity conversation ID"
            );
            let mut command = Command::new(&provider_binary);
            command.arg("--conversation").arg(session_id);
            if let Some(project_path) = project_path.filter(|value| !value.trim().is_empty()) {
                command.current_dir(project_path);
            }
            let status = command.status().context("run agy")?;
            if !status.success() {
                return Err(anyhow!("Antigravity CLI resume failed"));
            }
        }
    }
    Ok(())
}

fn install_shell_command(_provider: ProviderKind) -> Result<()> {
    let exe = current_exe()?;
    if cfg!(windows) {
        let launcher_root = install_cmd_launchers(&exe)?;
        let profile_path = install_powershell_shell_integration(&exe)?;
        println!("Launchers installed at {}", launcher_root.display());
        println!("Shell integration installed at {}", profile_path.display());
        println!("Reload your shell with: . $PROFILE");
        return Ok(());
    }

    let shell = env::var("SHELL").unwrap_or_default();
    if shell.ends_with("/pwsh") || shell.ends_with("/powershell") {
        let profile_path = install_powershell_shell_integration(&exe)?;
        println!("Shell integration installed at {}", profile_path.display());
        println!("Reload your shell with: . $PROFILE");
    } else {
        let result = install_posix_shell_integration(None)?;
        println!("Launchers installed at {}", result.launcher_root.display());
        println!(
            "Shell integration installed at {}",
            result.profile_path.display()
        );
        println!(
            "Reload your shell with: source {}",
            result.profile_path.display()
        );
    }
    Ok(())
}

fn uninstall_shell_command(_provider: ProviderKind) -> Result<()> {
    if cfg!(windows) {
        let profile_path = uninstall_powershell_shell_integration()?;
        let launcher_root = crate::paths::launcher_root();
        for name in ["csx.cmd", "clx.cmd", "opx.cmd", "agx.cmd", "sessionhub.cmd"] {
            let path = launcher_root.join(name);
            if path.exists() {
                let _ = fs::remove_file(path);
            }
        }
        println!("Shell integration removed from {}", profile_path.display());
        return Ok(());
    }

    let shell = env::var("SHELL").unwrap_or_default();
    if shell.ends_with("/pwsh") || shell.ends_with("/powershell") {
        let profile_path = uninstall_powershell_shell_integration()?;
        println!("Shell integration removed from {}", profile_path.display());
    } else {
        let profile_path = uninstall_posix_shell_integration()?;
        println!("Shell integration removed from {}", profile_path.display());
    }
    Ok(())
}

fn doctor_command(provider: ProviderKind) -> Result<()> {
    let binary_available = which::which(provider.binary_name()).is_ok();
    let fzf_available = which::which("fzf").is_ok();
    let config_root = config_root();
    let session_root = provider_session_root(provider);
    let launcher_root = crate::paths::launcher_root();
    let profile_path = if cfg!(windows) {
        powershell_profile_path()
    } else {
        crate::paths::detect_posix_profile()
    };
    let profile_installed = profile_path.exists()
        && fs::read_to_string(&profile_path)
            .unwrap_or_default()
            .contains("Agent Session Hub");

    println!("Provider: {}", provider.name());
    println!("ProviderName: {}", provider.display_name());
    println!("SessionRoot: {}", session_root.display());
    println!("SessionRootExists: {}", session_root.exists());
    println!("ConfigRoot: {}", config_root.display());
    println!("CacheRoot: {}", crate::cache::cache_root().display());
    println!(
        "CacheEnabled: {}",
        std::env::var("CODEX_SESSION_HUB_DISABLE_CACHE").as_deref() != Ok("1")
    );
    println!("FzfAvailable: {fzf_available}");
    println!("BinaryName: {}", provider.binary_name());
    println!("CommandAvailable: {binary_available}");
    if matches!(provider, ProviderKind::Claude | ProviderKind::Antigravity) {
        println!("HistoryDeletion: offline local CLI history with compressed recovery");
        println!(
            "RecoveryRoot: {}",
            crate::local_store::recovery_root(provider).display()
        );
    }
    println!("LauncherRoot: {}", launcher_root.display());
    println!("ProfilePath: {}", profile_path.display());
    println!("ProfileInstalled: {profile_installed}");
    Ok(())
}

fn usage(provider: ProviderKind) -> Result<()> {
    println!("{} version | --version | -V | -v", provider.launcher_name());
    println!("{} check-update | update [--yes]", provider.launcher_name());
    println!("{} [query]", provider.launcher_name());
    println!("{} browse [query]", provider.launcher_name());
    println!(
        "{} rename <session-id> --name <alias>",
        provider.launcher_name()
    );
    println!("{} reset <session-id>", provider.launcher_name());
    if provider.supports_delete() {
        println!(
            "{} delete <session-id...> [--dry-run] [--yes]",
            provider.launcher_name()
        );
    }
    println!("{} cleanup [options]", provider.launcher_name());
    if matches!(provider, ProviderKind::Claude | ProviderKind::Antigravity) {
        println!(
            "{} recovery list | restore <id> | purge <id> [--yes]",
            provider.launcher_name()
        );
    }
    println!("{} doctor", provider.launcher_name());
    println!("{} install-shell", provider.launcher_name());
    println!("{} uninstall-shell", provider.launcher_name());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv0_provider_aliases_work() {
        assert_eq!(ProviderKind::alias_from_argv0("csx"), ProviderKind::Codex);
        assert_eq!(ProviderKind::alias_from_argv0("clx"), ProviderKind::Claude);
        assert_eq!(
            ProviderKind::alias_from_argv0("opx"),
            ProviderKind::Opencode
        );
    }

    #[test]
    fn sessionhub_entrypoint_is_generic() {
        assert!(is_generic_entrypoint("sessionhub"));
        assert!(is_generic_entrypoint("/tmp/sessionhub"));
        assert!(is_generic_entrypoint("target/debug/agent-session-hub"));
        assert!(!is_generic_entrypoint("csx"));
    }

    #[test]
    fn generic_provider_aliases_work() {
        assert_eq!(generic_provider_from_arg("csx"), Some(ProviderKind::Codex));
        assert_eq!(
            generic_provider_from_arg("claude"),
            Some(ProviderKind::Claude)
        );
        assert_eq!(
            generic_provider_from_arg("opx"),
            Some(ProviderKind::Opencode)
        );
        assert_eq!(generic_provider_from_arg("unknown"), None);
    }

    #[test]
    fn one_workspace_header_is_a_single_resume_selection() {
        assert!(is_single_workspace_selection(&[
            "W:codex:repo|main".to_string()
        ]));
        assert!(!is_single_workspace_selection(&[
            "W:codex:repo|main".to_string(),
            "S:codex:abc123".to_string(),
        ]));
        assert!(!is_single_workspace_selection(&[
            "S:codex:abc123".to_string()
        ]));
    }

    #[test]
    fn cleanup_ages_accept_hours_days_weeks_and_bare_days() {
        assert_eq!(parse_cleanup_age("12h").unwrap(), Duration::hours(12));
        assert_eq!(parse_cleanup_age("7d").unwrap(), Duration::days(7));
        assert_eq!(parse_cleanup_age("4w").unwrap(), Duration::weeks(4));
        assert_eq!(parse_cleanup_age("30").unwrap(), Duration::days(30));
        assert!(parse_cleanup_age("-1d").is_err());
        assert!(parse_cleanup_age("tomorrow").is_err());
        assert!(parse_cleanup_age("9223372036854775807h").is_err());
        assert!(parse_cleanup_age("100000000000d").is_err());
    }

    #[test]
    fn cleanup_category_flags_select_only_requested_types() {
        let args = vec![
            "--agents".to_string(),
            "--older-than".to_string(),
            "90d".to_string(),
            "--dry-run".to_string(),
        ];
        let (policy, dry_run) = parse_cleanup_options(&args).unwrap();
        assert!(dry_run);
        assert!(!policy.include_security_scans);
        assert!(policy.include_linked_agents);
        assert!(!policy.include_orphan_agents);
        assert_eq!(policy.linked_agent_age, Duration::days(90));
        assert!(!policy.include_chats);
        let (policy, _) =
            parse_cleanup_options(&["--chats".into(), "--older-than".into(), "180d".into()])
                .unwrap();
        assert!(policy.include_chats);
        assert!(!policy.include_linked_agents);
        assert!(!policy.include_security_scans);
        assert_eq!(policy.chat_age, Duration::days(180));
    }
}
