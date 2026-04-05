use crate::browser::{
    confirm_delete, ensure_fzf, fzf_rows, parse_row_target, resolve_selected_sessions, run_fzf,
    write_preview,
};
use crate::config::AliasIndex;
use crate::paths::{config_root, current_exe, powershell_profile_path, provider_session_root};
use crate::provider::ProviderKind;
use crate::session::{display_sessions, filter_display_sessions, find_session, load_sessions};
use crate::shell::{
    install_cmd_launchers, install_posix_shell_integration, install_powershell_shell_integration,
    uninstall_posix_shell_integration, uninstall_powershell_shell_integration,
};
use anyhow::{Context, Result, anyhow};
use std::env;
use std::fs;
use std::io::{self, Write};
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
    }
}

fn generic_provider_from_arg(value: &str) -> Option<ProviderKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        "codex" | "csx" => Some(ProviderKind::Codex),
        "claude" | "clx" => Some(ProviderKind::Claude),
        "opencode" | "opx" => Some(ProviderKind::Opencode),
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
        eprint!("Choice [1-3,q]: ");
        io::stderr().flush()?;
        let mut choice = String::new();
        io::stdin().read_line(&mut choice)?;
        let normalized = choice.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "" | "q" | "quit" | "exit" => return Ok(None),
            "1" | "csx" | "codex" => return Ok(Some(ProviderKind::Codex)),
            "2" | "clx" | "claude" => return Ok(Some(ProviderKind::Claude)),
            "3" | "opx" | "opencode" => return Ok(Some(ProviderKind::Opencode)),
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
    println!("sessionhub <codex|claude|opencode> [query]");
    println!("sessionhub <csx|clx|opx> [query]");
    println!("sessionhub install-shell");
    println!("sessionhub uninstall-shell");
    println!();
    generic_providers();
    println!();
    println!("Use `csx help`, `clx help`, or `opx help` for provider-specific commands.");
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
                "Use `csx doctor`, `clx doctor`, or `opx doctor` for provider-specific diagnostics."
            );
            Ok(())
        }
        "install-shell" => install_shell_command(ProviderKind::Codex),
        "uninstall-shell" => uninstall_shell_command(ProviderKind::Codex),
        _ => {
            let explicit = generic_provider_from_arg(&args[0]);
            let Some(provider) = explicit.or(prompt_for_provider()?) else {
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
    match provider {
        ProviderKind::Codex => {
            fs::remove_file(&session.file_path)
                .with_context(|| format!("remove {}", session.file_path.display()))?;
        }
        ProviderKind::Claude => {
            return Err(anyhow!("Claude session delete is not supported."));
        }
        ProviderKind::Opencode => {
            let status = Command::new("opencode")
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
    for session in targets {
        if let Err(err) = delete_session(provider, &session, &mut index) {
            eprintln!("[failed] {} {}", session.session_id, err);
        }
    }
    index.save(provider)
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

fn browse_command(provider: ProviderKind, query: &str) -> Result<()> {
    ensure_fzf()?;
    let exe = current_exe()?;
    let query = query.to_string();

    loop {
        let (_index, sessions) = load_index_and_sessions(provider)?;
        let display = display_sessions(&sessions);
        let rows = fzf_rows(&filter_display_sessions(&sessions, &query));
        if rows.is_empty() {
            return Ok(());
        }
        let Some(result) = run_fzf(provider, &query, &rows, &exe)? else {
            return Ok(());
        };
        let selected = resolve_selected_sessions(&display, &result.session_ids);
        if selected.is_empty() {
            continue;
        }

        match result.action.as_str() {
            "enter" => {
                if selected.len() > 1 {
                    return Err(anyhow!(
                        "Resume only supports one session at a time. Clear multi-select or choose a single row."
                    ));
                }
                return resume_session(&selected[0].session);
            }
            "ctrl-d" => interactive_delete(provider, &selected)?,
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
    if args.is_empty() {
        return Err(anyhow!("delete requires at least one session id."));
    }
    let (_index, sessions) = load_index_and_sessions(provider)?;
    let display = display_sessions(&sessions);
    let targets = args
        .iter()
        .map(|value| {
            find_session(&display, value, Some(provider))
                .map(|entry| entry.session.clone())
                .ok_or_else(|| anyhow!("No matching session found for {value}"))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut index = AliasIndex::load(provider)?;
    for session in targets {
        match delete_session(provider, &session, &mut index) {
            Ok(_) => println!("[deleted] {} Deleted", session.session_id),
            Err(err) => println!("[failed] {} {}", session.session_id, err),
        }
    }
    index.save(provider)
}

fn hidden_preview(provider: ProviderKind, args: &[String]) -> Result<()> {
    let (session_id, workspace_key, project_path) = if let Some(raw) = args.first() {
        parse_row_target(raw)
    } else {
        (String::new(), String::new(), String::new())
    };
    let (_index, sessions) = load_index_and_sessions(provider)?;
    let display = display_sessions(&sessions);
    write_preview(
        provider,
        &display,
        &session_id,
        &workspace_key,
        &project_path,
    )
}

fn hidden_query(provider: ProviderKind, args: &[String]) -> Result<()> {
    let query = if !args.is_empty() {
        args.join(" ")
    } else {
        env::var("FZF_QUERY").unwrap_or_default()
    };
    let (_index, sessions) = load_index_and_sessions(provider)?;
    for row in fzf_rows(&filter_display_sessions(&sessions, &query)) {
        println!("{row}");
    }
    Ok(())
}

fn hidden_select(provider: ProviderKind, args: &[String]) -> Result<()> {
    ensure_fzf()?;
    let exe = current_exe()?;
    let query = args.join(" ");

    loop {
        let (_index, sessions) = load_index_and_sessions(provider)?;
        let display = display_sessions(&sessions);
        let rows = fzf_rows(&filter_display_sessions(&sessions, &query));
        if rows.is_empty() {
            return Ok(());
        }
        let Some(result) = run_fzf(provider, &query, &rows, &exe)? else {
            return Ok(());
        };
        let selected = resolve_selected_sessions(&display, &result.session_ids);
        if selected.is_empty() {
            continue;
        }

        match result.action.as_str() {
            "enter" => {
                if selected.len() > 1 {
                    return Err(anyhow!(
                        "Resume only supports one session at a time. Clear multi-select or choose a single row."
                    ));
                }
                println!(
                    "{}	{}",
                    selected[0].session.project_path, selected[0].session.session_id
                );
                return Ok(());
            }
            "ctrl-d" => interactive_delete(provider, &selected)?,
            "ctrl-e" => interactive_rename(provider, selected[0])?,
            "ctrl-r" => interactive_reset(provider, selected[0])?,
            _ => return Err(anyhow!("Unsupported browser action: {}", result.action)),
        }
    }
}

fn resume_session(session: &crate::session::SessionRecord) -> Result<()> {
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
    which::which(provider.binary_name())
        .with_context(|| format!("{} was not found in PATH.", provider.binary_name()))?;
    match provider {
        ProviderKind::Codex => {
            let mut command = Command::new("codex");
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
            let mut command = Command::new("claude");
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
            let mut command = Command::new("opencode");
            command.arg("--session").arg(session_id);
            if let Some(project_path) = project_path.filter(|value| !value.trim().is_empty()) {
                command.current_dir(project_path);
            }
            let status = command.status().context("run opencode")?;
            if !status.success() {
                return Err(anyhow!("opencode resume failed"));
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
        for name in ["csx.cmd", "clx.cmd", "opx.cmd", "sessionhub.cmd"] {
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
    println!("FzfAvailable: {fzf_available}");
    println!("BinaryName: {}", provider.binary_name());
    println!("CommandAvailable: {binary_available}");
    println!("LauncherRoot: {}", launcher_root.display());
    println!("ProfilePath: {}", profile_path.display());
    println!("ProfileInstalled: {profile_installed}");
    Ok(())
}

fn usage(provider: ProviderKind) -> Result<()> {
    println!("{} [query]", provider.launcher_name());
    println!("{} browse [query]", provider.launcher_name());
    println!(
        "{} rename <session-id> --name <alias>",
        provider.launcher_name()
    );
    println!("{} reset <session-id>", provider.launcher_name());
    if provider.supports_delete() {
        println!("{} delete <session-id...>", provider.launcher_name());
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
}
