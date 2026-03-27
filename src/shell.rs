use crate::formatting::escape_sh_single_quotes;
use crate::paths::{detect_posix_profile, ensure_parent, launcher_root, powershell_profile_path};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

const MARKER_START: &str = "# >>> Agent Session Hub >>>";
const MARKER_END: &str = "# <<< Agent Session Hub <<<";

#[derive(Clone, Debug)]
pub struct ShellInstallResult {
    pub launcher_root: PathBuf,
    pub profile_path: PathBuf,
}

fn replace_marked_block(path: &Path, block: &str) -> Result<()> {
    ensure_parent(path)?;
    let content = fs::read_to_string(path).unwrap_or_default();
    let updated = if let Some(start) = content.find(MARKER_START) {
        if let Some(end_relative) = content[start..].find(MARKER_END) {
            let end = start + end_relative + MARKER_END.len();
            let mut merged = String::new();
            merged.push_str(&content[..start]);
            if !merged.trim_end().is_empty() {
                merged.push('\n');
                merged.push('\n');
            }
            merged.push_str(block);
            if !content[end..].trim().is_empty() {
                merged.push('\n');
                merged.push('\n');
                merged.push_str(content[end..].trim());
                merged.push('\n');
            } else {
                merged.push('\n');
            }
            merged
        } else {
            block.to_string()
        }
    } else if content.trim().is_empty() {
        format!("{block}\n")
    } else {
        format!("{}\n\n{block}\n", content.trim_end())
    };
    fs::write(path, updated).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn remove_marked_block(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let content = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let Some(start) = content.find(MARKER_START) else {
        return Ok(false);
    };
    let Some(end_relative) = content[start..].find(MARKER_END) else {
        return Ok(false);
    };
    let end = start + end_relative + MARKER_END.len();
    let updated = format!("{}{}", &content[..start], &content[end..])
        .trim()
        .to_string();
    if updated.is_empty() {
        fs::remove_file(path).with_context(|| format!("remove {}", path.display()))?;
    } else {
        fs::write(path, format!("{updated}\n"))
            .with_context(|| format!("write {}", path.display()))?;
    }
    Ok(true)
}

fn posix_function(name: &str, binary_name: &str, passthrough: &[&str]) -> String {
    let passthrough_cases = passthrough
        .iter()
        .map(|value| format!("    {value})\n      shift\n      ;;"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"{name}() {{
  case "${{1-}}" in
{passthrough_cases}
    doctor|rename|reset|delete|help|install-shell|uninstall-shell|__*)
      command {name} "$@"
      return $?
      ;;
  esac

  local _ash_result
  _ash_result="$(command {name} __select "$@")" || return $?
  [ -z "$_ash_result" ] && return 0

  local _ash_project="${{_ash_result%%	*}}"
  local _ash_session="${{_ash_result#*	}}"

  if [ -n "$_ash_project" ] && [ -d "$_ash_project" ]; then
    cd "$_ash_project" || return $?
  fi

  {binary_name} --resume "$_ash_session"
}}"#
    )
}

fn fish_function(name: &str, binary_name: &str) -> String {
    format!(
        r#"function {name}
    set first $argv[1]
    switch "$first"
        case browse
            set argv $argv[2..-1]
        case doctor rename reset delete help install-shell uninstall-shell '__*'
            command {name} $argv
            return $status
    end

    set ash_result (command {name} __select $argv)
    if test -z "$ash_result"
        return 0
    end

    set ash_parts (string split \t -- "$ash_result")
    if test -n "$ash_parts[1]" -a -d "$ash_parts[1]"
        cd "$ash_parts[1]"
    end

    {binary_name} --resume "$ash_parts[2]"
end"#
    )
}

fn powershell_function(name: &str, provider: &str, binary_name: &str, exe: &str) -> String {
    format!(
        r#"function {name} {{
    $commandName = if ($args.Count -gt 0) {{ [string]$args[0] }} else {{ '' }}
    if ($commandName -eq 'browse') {{
        $args = @($args | Select-Object -Skip 1)
        $commandName = if ($args.Count -gt 0) {{ [string]$args[0] }} else {{ '' }}
    }}

    if ($commandName -in @('doctor', 'rename', 'reset', 'delete', 'help', 'install-shell', 'uninstall-shell') -or $commandName.StartsWith('__')) {{
        & '{exe}' --provider {provider} @args
        return
    }}

    $result = & '{exe}' --provider {provider} __select @args
    if (-not $result) {{ return }}
    $parts = $result -split "`t", 2
    if ($parts.Length -ge 1 -and -not [string]::IsNullOrWhiteSpace($parts[0]) -and (Test-Path $parts[0])) {{
        Set-Location $parts[0]
    }}
    if ($parts.Length -ge 2 -and -not [string]::IsNullOrWhiteSpace($parts[1])) {{
        & {binary_name} --resume $parts[1]
    }}
}}"#
    )
}

pub fn cmd_launcher_content(exe_name: &str, provider: &str, binary_name: &str) -> String {
    format!(
        "@echo off\r\n\
setlocal\r\n\
set \"ASH_EXE=%~dp0{exe_name}\"\r\n\
set \"ASH_COMMAND=%~1\"\r\n\
\r\n\
if /I \"%ASH_COMMAND%\"==\"browse\" (\r\n\
  shift\r\n\
  set \"ASH_COMMAND=%~1\"\r\n\
)\r\n\
\r\n\
if /I \"%ASH_COMMAND%\"==\"doctor\" goto passthrough\r\n\
if /I \"%ASH_COMMAND%\"==\"rename\" goto passthrough\r\n\
if /I \"%ASH_COMMAND%\"==\"reset\" goto passthrough\r\n\
if /I \"%ASH_COMMAND%\"==\"delete\" goto passthrough\r\n\
if /I \"%ASH_COMMAND%\"==\"help\" goto passthrough\r\n\
if /I \"%ASH_COMMAND%\"==\"install-shell\" goto passthrough\r\n\
if /I \"%ASH_COMMAND%\"==\"uninstall-shell\" goto passthrough\r\n\
if \"%ASH_COMMAND:~0,2%\"==\"__\" goto passthrough\r\n\
\r\n\
:select\r\n\
for /f \"usebackq tokens=1,* delims=\t\" %%A in (`\"%ASH_EXE%\" --provider {provider} __select %*`) do (\r\n\
  set \"ASH_PROJECT=%%A\"\r\n\
  set \"ASH_SESSION=%%B\"\r\n\
)\r\n\
\r\n\
if not defined ASH_SESSION exit /b 0\r\n\
if defined ASH_PROJECT if exist \"%ASH_PROJECT%\" cd /d \"%ASH_PROJECT%\"\r\n\
{binary_name} --resume \"%ASH_SESSION%\"\r\n\
exit /b %ERRORLEVEL%\r\n\
\r\n\
:passthrough\r\n\
\"%ASH_EXE%\" --provider {provider} %*\r\n\
exit /b %ERRORLEVEL%\r\n"
    )
}

pub fn install_cmd_launchers(exe_path: &Path) -> Result<PathBuf> {
    let launcher_root = exe_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(launcher_root);
    fs::create_dir_all(&launcher_root)
        .with_context(|| format!("create {}", launcher_root.display()))?;
    let exe_name = exe_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("agent-session-hub.exe");
    let launchers = [
        ("csx.cmd", cmd_launcher_content(exe_name, "codex", "codex")),
        (
            "clx.cmd",
            cmd_launcher_content(exe_name, "claude", "claude"),
        ),
        ("cxs.cmd", "@echo off\r\ncsx %*\r\n".to_string()),
    ];
    for (name, content) in launchers {
        let path = launcher_root.join(name);
        fs::write(&path, content).with_context(|| format!("write {}", path.display()))?;
    }
    Ok(launcher_root)
}

pub fn install_posix_shell_integration(shell_name: Option<&str>) -> Result<ShellInstallResult> {
    let launcher_root = launcher_root();
    fs::create_dir_all(&launcher_root)
        .with_context(|| format!("create {}", launcher_root.display()))?;
    let profile_path = detect_posix_profile();

    let block = if profile_path.ends_with("config.fish") || shell_name == Some("fish") {
        format!(
            "{MARKER_START}\nset -gx PATH \"{}\" $PATH\n{}\n{}\nfunction cxs\n    csx $argv\nend\n{MARKER_END}",
            launcher_root.display(),
            fish_function("csx", "codex"),
            fish_function("clx", "claude"),
        )
    } else {
        format!(
            "{MARKER_START}\nexport PATH='{}':$PATH\n{}\n{}\ncxs() {{ csx \"$@\"; }}\n{MARKER_END}",
            escape_sh_single_quotes(&launcher_root.to_string_lossy()),
            posix_function("csx", "codex", &["browse"]),
            posix_function("clx", "claude", &["browse"]),
        )
    };
    replace_marked_block(&profile_path, &block)?;
    Ok(ShellInstallResult {
        launcher_root,
        profile_path,
    })
}

pub fn uninstall_posix_shell_integration() -> Result<PathBuf> {
    let profile_path = detect_posix_profile();
    let _ = remove_marked_block(&profile_path)?;
    Ok(profile_path)
}

pub fn powershell_profile_block(exe_path: &Path) -> String {
    let exe = exe_path.display().to_string();
    format!(
        r#"{MARKER_START}
{}
{}
Set-Alias cxs csx
{MARKER_END}"#,
        powershell_function("csx", "codex", "codex", &exe),
        powershell_function("clx", "claude", "claude", &exe),
    )
}

pub fn install_powershell_shell_integration(exe_path: &Path) -> Result<PathBuf> {
    let profile_path = powershell_profile_path();
    replace_marked_block(&profile_path, &powershell_profile_block(exe_path))?;
    Ok(profile_path)
}

pub fn uninstall_powershell_shell_integration() -> Result<PathBuf> {
    let profile_path = powershell_profile_path();
    let _ = remove_marked_block(&profile_path)?;
    Ok(profile_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powershell_block_passes_through_management_commands() {
        let block = powershell_profile_block(Path::new("/tmp/agent-session-hub"));
        assert!(block.contains("--provider codex @args"));
        assert!(block.contains("__select @args"));
        assert!(block.contains("$commandName -eq 'browse'"));
    }

    #[test]
    fn cmd_block_selects_then_resumes() {
        let block = cmd_launcher_content("agent-session-hub.exe", "claude", "claude");
        assert!(block.contains("--provider claude __select"));
        assert!(block.contains("claude --resume"));
        assert!(block.contains("goto passthrough"));
    }
}
