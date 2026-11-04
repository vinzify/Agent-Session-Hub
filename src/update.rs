//! Public-release discovery and user-local, verified self-updates. Never touches session data.
use anyhow::{Context, Result, anyhow, bail, ensure};
use reqwest::blocking::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
#[cfg(not(windows))]
use std::path::Component;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const REPO: &str = "vinzify/Agent-Session-Hub";
const LATEST: &str = "https://api.github.com/repos/vinzify/Agent-Session-Hub/releases/latest";
const SIDECAR: &str = ".agent-session-hub-install.json";
const MAX_ARCHIVE: u64 = 64 * 1024 * 1024;
const MAX_BINARY: u64 = 128 * 1024 * 1024;
const DAY: u64 = 24 * 60 * 60;

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

fn release_version(tag: &str) -> Result<Version> {
    let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag))?;
    ensure!(
        version.pre.is_empty() && version.build.is_empty(),
        "not a stable release"
    );
    Ok(version)
}

fn client(timeout: u64) -> Result<Client> {
    Client::builder()
        .user_agent(concat!("Agent-Session-Hub/", env!("CARGO_PKG_VERSION")))
        .https_only(true)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(timeout))
        .build()
        .context("initialize update client")
}

fn limited_read(reader: impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "update response exceeds size limit"
    );
    Ok(bytes)
}

fn latest() -> Result<Release> {
    let response = client(8)?
        .get(LATEST)
        .header("Accept", "application/vnd.github+json")
        .send()
        .context("check GitHub releases")?
        .error_for_status()?;
    let release: Release = serde_json::from_slice(&limited_read(response, 2 * 1024 * 1024)?)?;
    ensure!(
        !release.draft && !release.prerelease,
        "GitHub returned an unpublished or prerelease version"
    );
    release_version(&release.tag_name)?;
    Ok(release)
}

fn asset_url(release: &Release, name: &str) -> Result<String> {
    let expected = format!(
        "https://github.com/{REPO}/releases/download/{}/{name}",
        release.tag_name
    );
    let matches: Vec<_> = release
        .assets
        .iter()
        .filter(|asset| asset.name == name)
        .collect();
    ensure!(
        matches.len() == 1 && matches[0].browser_download_url == expected,
        "missing or unexpected release asset: {name}"
    );
    Ok(expected)
}

fn target() -> Result<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-gnu"),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin"),
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        ("windows", "x86_64") => Ok("x86_64-pc-windows-msvc"),
        _ => bail!("No prebuilt update for this platform; update from source instead."),
    }
}

fn checksum(text: &str, name: &str) -> Result<String> {
    let mut found = Vec::new();
    for line in text.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() == 2 && fields[1].trim_start_matches('*') == name {
            ensure!(
                fields[0].len() == 64 && fields[0].bytes().all(|c| c.is_ascii_hexdigit()),
                "invalid SHA256SUMS entry"
            );
            found.push(fields[0].to_ascii_lowercase());
        }
    }
    ensure!(
        found.len() == 1,
        "SHA256SUMS must contain exactly one entry for {name}"
    );
    Ok(found.remove(0))
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn verify(bytes: &[u8], expected: &str) -> Result<()> {
    ensure!(
        hash(bytes) == expected,
        "update checksum mismatch; nothing was installed"
    );
    Ok(())
}

#[cfg(not(windows))]
fn extract_binary(bytes: &[u8]) -> Result<Vec<u8>> {
    let decoder = flate2::read::GzDecoder::new(bytes).take(MAX_BINARY + 16 * 1024 * 1024);
    let mut archive = tar::Archive::new(decoder);
    let mut binary = None;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let components: Vec<_> = path
            .components()
            .filter(|part| *part != Component::CurDir)
            .collect();
        if components == [Component::Normal("agent-session-hub".as_ref())] {
            ensure!(
                entry.header().entry_type().is_file() && binary.is_none(),
                "invalid or duplicate executable in update archive"
            );
            binary = Some(limited_read(&mut entry, MAX_BINARY)?);
        }
    }
    binary.context("release archive has no root agent-session-hub executable")
}

#[cfg(windows)]
fn extract_binary(bytes: &[u8]) -> Result<Vec<u8>> {
    extract_zip_binary(bytes)
}

#[cfg(any(windows, test))]
fn extract_zip_binary(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(io::Cursor::new(bytes))?;
    ensure!(archive.len() <= 1024, "too many archive entries");
    ensure!(
        archive
            .file_names()
            .filter_map(|name| name.ok())
            .filter(|name| name.as_ref() == "agent-session-hub.exe")
            .count()
            == 1,
        "release archive has no unique root agent-session-hub.exe executable"
    );
    let mut entry = archive.by_name("agent-session-hub.exe")?;
    ensure!(entry.is_file(), "update executable is not a file");
    limited_read(&mut entry, MAX_BINARY)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Installation {
    format: u32,
    install_root: PathBuf,
    bin_root: PathBuf,
}

impl Installation {
    fn binary_paths(&self) -> Vec<PathBuf> {
        let exe = if cfg!(windows) {
            "agent-session-hub.exe"
        } else {
            "agent-session-hub"
        };
        let mut paths = vec![self.install_root.join("bin").join(exe)];
        if cfg!(windows) {
            paths.push(self.bin_root.join(exe));
        } else {
            for name in ["csx", "clx", "opx", "sessionhub"] {
                paths.push(self.bin_root.join(name));
            }
        }
        paths.sort();
        paths.dedup();
        paths
    }

    fn validate(&self, current: &Path) -> Result<()> {
        ensure!(
            self.format == 1 && self.install_root.is_absolute() && self.bin_root.is_absolute(),
            "invalid installation record"
        );
        let paths = self.binary_paths();
        ensure!(
            paths.iter().any(|path| path == current),
            "this executable is not part of the recorded installation"
        );
        let expected = hash(&fs::read(current)?);
        for path in paths {
            ensure!(
                fs::symlink_metadata(&path)?.file_type().is_file(),
                "refusing to overwrite a symlink/non-file: {}",
                path.display()
            );
            ensure!(
                hash(&fs::read(&path)?) == expected,
                "installed launchers differ at {}; rerun the installer to repair them first",
                path.display()
            );
        }
        Ok(())
    }
}

pub fn record_install(install_root: &Path, bin_root: &Path) -> Result<()> {
    let record = Installation {
        format: 1,
        install_root: install_root.canonicalize()?,
        bin_root: bin_root.canonicalize()?,
    };
    record.validate(&std::env::current_exe()?.canonicalize()?)?;
    let bytes = serde_json::to_vec_pretty(&record)?;
    crate::atomic::write(&record.install_root.join("installation.json"), &bytes)?;
    crate::atomic::write(&record.bin_root.join(SIDECAR), &bytes)?;
    Ok(())
}

fn installation() -> Result<Installation> {
    let current = std::env::current_exe()?.canonicalize()?;
    let parent = current.parent().context("executable has no directory")?;
    let candidates = [
        parent.join(SIDECAR),
        parent.parent().unwrap_or(parent).join("installation.json"),
    ];
    for path in candidates {
        if path.try_exists()? {
            let record: Installation = serde_json::from_slice(&fs::read(&path)?)
                .with_context(|| format!("read installation record {}", path.display()))?;
            record.validate(&current)?;
            return Ok(record);
        }
    }
    bail!(
        "This is not a recorded installer-managed installation. Run the current install.sh/install.ps1 once to enable self-update. Source and package-manager builds should use their own update method."
    )
}

#[derive(Clone, Serialize, Deserialize)]
struct CachedCheck {
    checked_at: u64,
    tag: Option<String>,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn cache_path() -> PathBuf {
    crate::cache::cache_root().join("update-check.json")
}

fn save_cache(cache: &CachedCheck) -> Result<()> {
    let path = cache_path();
    let mut directory = fs::DirBuilder::new();
    directory.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directory.mode(0o700);
    }
    directory.create(path.parent().context("cache has no directory")?)?;
    crate::atomic::write(&path, &serde_json::to_vec(cache)?)
}
fn newer(tag: &str) -> bool {
    release_version(tag)
        .is_ok_and(|version| version > Version::parse(env!("CARGO_PKG_VERSION")).unwrap())
}
fn notification(tag: Option<&str>) -> Option<String> {
    tag.filter(|tag| newer(tag)).map(|tag| {
        format!(
            "Agent Session Hub update available: {} -> {}. Run: sessionhub update",
            env!("CARGO_PKG_VERSION"),
            release_version(tag).unwrap()
        )
    })
}

/// Only instantiated by the interactive browser, never by query/preview subprocesses.
pub struct Notice {
    worker: Option<std::thread::JoinHandle<Option<String>>>,
    shown: Option<String>,
}
impl Notice {
    pub fn start() -> Self {
        let mut notice = Self {
            worker: None,
            shown: None,
        };
        if !io::stdin().is_terminal()
            || !io::stderr().is_terminal()
            || std::env::var("AGENT_SESSION_HUB_NO_UPDATE_CHECK").as_deref() == Ok("1")
        {
            return notice;
        }
        let cached = fs::read(cache_path())
            .ok()
            .and_then(|bytes| serde_json::from_slice::<CachedCheck>(&bytes).ok());
        notice.shown = notification(cached.as_ref().and_then(|cache| cache.tag.as_deref()));
        if let Some(message) = &notice.shown {
            eprintln!("{message}");
        }
        if cached
            .as_ref()
            .is_some_and(|cache| now().saturating_sub(cache.checked_at) < DAY)
        {
            return notice;
        }
        let pending = CachedCheck {
            checked_at: now(),
            tag: cached.and_then(|cache| cache.tag),
        };
        // Reserve the daily attempt before networking; failures keep the previous known release.
        if save_cache(&pending).is_err() {
            return notice;
        }
        notice.worker = Some(std::thread::spawn(move || {
            let release = latest().ok()?;
            let cache = CachedCheck {
                checked_at: now(),
                tag: Some(release.tag_name),
            };
            let _ = save_cache(&cache);
            notification(cache.tag.as_deref())
        }));
        notice
    }
}
impl Drop for Notice {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take().filter(|worker| worker.is_finished())
            && let Ok(Some(message)) = worker.join()
            && self.shown.as_ref() != Some(&message)
        {
            eprintln!("{message}");
        }
    }
}

pub fn dispatch(args: &[String]) -> Option<Result<()>> {
    let command = args.first()?.as_str();
    match command {
        "version" | "--version" | "-V" => Some(if args.len() == 1 {
            println!("Agent Session Hub {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        } else {
            Err(anyhow!("version accepts no arguments"))
        }),
        "check-update" => Some(check_command(&args[1..])),
        "update" => Some(update_command(&args[1..])),
        "__record-install" => Some(if args.len() == 3 {
            record_install(Path::new(&args[1]), Path::new(&args[2]))
        } else {
            Err(anyhow!(
                "internal installation record requires install and launcher directories"
            ))
        }),
        _ => None,
    }
}

fn help(args: &[String]) -> bool {
    args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h")
}
fn check_command(args: &[String]) -> Result<()> {
    if help(args) {
        println!(
            "sessionhub check-update\nCheck the latest stable release without installing. No login/token required."
        );
        return Ok(());
    }
    ensure!(args.is_empty(), "check-update accepts only --help");
    let release = latest()?;
    let cache = CachedCheck {
        checked_at: now(),
        tag: Some(release.tag_name.clone()),
    };
    let _ = save_cache(&cache);
    println!(
        "{}",
        notification(Some(&release.tag_name)).unwrap_or_else(|| format!(
            "Agent Session Hub {} is up to date.",
            env!("CARGO_PKG_VERSION")
        ))
    );
    Ok(())
}

fn update_command(args: &[String]) -> Result<()> {
    if help(args) {
        println!(
            "sessionhub update [--yes]\nDownload the latest stable release, verify SHA-256, and update installer-managed launchers.\nConfirmation is required unless --yes/-y is supplied. No session data or shell profiles are changed."
        );
        return Ok(());
    }
    ensure!(
        args.iter()
            .all(|arg| matches!(arg.as_str(), "--yes" | "-y")),
        "unknown update option; use sessionhub update --help"
    );
    let record = installation()?;
    let target = target()?;
    let release = latest()?;
    if !newer(&release.tag_name) {
        println!(
            "Agent Session Hub {} is up to date.",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    let kind = if cfg!(windows) { "zip" } else { "tar.gz" };
    let archive_name = format!("agent-session-hub-{target}.{kind}");
    let url = asset_url(&release, &archive_name)?;
    let sums_url = asset_url(&release, "SHA256SUMS")?;
    println!(
        "Update {} -> {}\nInstall: {}\nLaunchers: {}",
        env!("CARGO_PKG_VERSION"),
        release_version(&release.tag_name)?,
        record.install_root.display(),
        record.bin_root.display()
    );
    if !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--yes" | "-y"))
    {
        ensure!(
            io::stdin().is_terminal() && io::stderr().is_terminal(),
            "confirmation needs a terminal; use --yes for unattended updates"
        );
        eprint!("Download and install this update? [y/N] ");
        io::stderr().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            println!("Update cancelled. Nothing was changed.");
            return Ok(());
        }
    }
    let client = client(120)?;
    let sums = limited_read(
        client.get(sums_url).send()?.error_for_status()?,
        1024 * 1024,
    )?;
    let expected = checksum(std::str::from_utf8(&sums)?, &archive_name)?;
    println!("Downloading {archive_name}…");
    let bytes = limited_read(client.get(url).send()?.error_for_status()?, MAX_ARCHIVE)?;
    verify(&bytes, &expected)?;
    let binary = extract_binary(&bytes)?;
    ensure!(!binary.is_empty(), "empty update executable");
    let stage = tempfile::Builder::new()
        .prefix("agent-session-hub-update-")
        .tempdir()?;
    let source = stage.path().join(if cfg!(windows) {
        "agent-session-hub.exe"
    } else {
        "agent-session-hub"
    });
    fs::write(&source, &binary)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&source, fs::Permissions::from_mode(0o755))?;
    }
    let probe = Command::new(&source)
        .arg("--version")
        .output()
        .context("test downloaded executable")?;
    let wanted = format!("Agent Session Hub {}", release_version(&release.tag_name)?);
    ensure!(
        probe.status.success() && String::from_utf8_lossy(&probe.stdout).trim() == wanted,
        "downloaded binary does not match the release version; nothing was installed"
    );
    record.validate(&std::env::current_exe()?.canonicalize()?)?;
    #[cfg(not(windows))]
    {
        replace_all(&binary, &record.binary_paths())?;
        println!(
            "Updated to {}. All launchers are ready; sessions/settings were untouched.",
            release_version(&release.tag_name)?
        );
    }
    #[cfg(windows)]
    schedule_windows(stage, &source, &record)?;
    Ok(())
}

#[cfg(not(windows))]
fn replace_all(binary: &[u8], paths: &[PathBuf]) -> Result<()> {
    let originals: Vec<_> = paths.iter().map(fs::read).collect::<io::Result<_>>()?;
    for (index, path) in paths.iter().enumerate() {
        if let Err(error) = crate::atomic::write(path, binary) {
            let mut rollback_errors = Vec::new();
            for previous in (0..=index).rev() {
                if let Err(rollback) = crate::atomic::write(&paths[previous], &originals[previous])
                {
                    rollback_errors.push(format!("{}: {rollback:#}", paths[previous].display()));
                }
            }
            if !rollback_errors.is_empty() {
                bail!(
                    "update failed: {error:#}; recovery required: {}. Rerun the installer",
                    rollback_errors.join("; ")
                );
            }
            return Err(error).context("update failed; original launchers restored");
        }
    }
    Ok(())
}

#[cfg(windows)]
fn schedule_windows(stage: tempfile::TempDir, source: &Path, record: &Installation) -> Result<()> {
    let mut files = Vec::new();
    for destination in record.binary_paths() {
        files.push(serde_json::json!({"destination":destination,"source":source,"sha256":hash(&fs::read(&destination)?)}));
    }
    // The helper replaces binaries only. Existing .cmd and PowerShell routing stays valid.
    let plan = stage.path().join("plan.json");
    fs::write(
        &plan,
        serde_json::to_vec(&serde_json::json!({"files":files,"stage":stage.path()}))?,
    )?;
    let script = stage.path().join("apply.ps1");
    fs::write(&script, include_str!("update-windows.ps1"))?;
    Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&script)
        .arg("-PlanPath")
        .arg(&plan)
        .arg("-AshParentId")
        .arg(std::process::id().to_string())
        .stdin(std::process::Stdio::null())
        .spawn()
        .context("launch Windows update helper")?;
    let _ = stage.keep();
    println!(
        "Update verified. The helper will apply it after this command exits. Close other running hub processes if replacement is blocked."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_semver_comparison_and_safe_notices() {
        assert!(newer("v99.0.0"));
        assert!(!newer("v0.1.0"));
        assert!(!newer("v99.0.0-beta.1"));
        assert!(!newer("\u{1b}[31m99.0.0"));
        assert!(
            notification(Some("v99.0.0"))
                .unwrap()
                .contains("sessionhub update")
        );
        assert!(notification(Some(concat!("v", env!("CARGO_PKG_VERSION")))).is_none());
    }
    #[test]
    fn checksum_rejects_missing_duplicate_malformed_and_tampered_data() {
        let digest = hash(b"verified data");
        assert_eq!(
            checksum(&format!("{digest}  archive.tar.gz\n"), "archive.tar.gz").unwrap(),
            digest
        );
        assert!(checksum("bad archive.tar.gz", "archive.tar.gz").is_err());
        assert!(checksum(&format!("{digest} a\n{digest} a"), "a").is_err());
        assert!(checksum(&format!("{digest} a"), "b").is_err());
        assert!(verify(b"modified data", &digest).is_err());
        assert!(limited_read(&b"too big"[..], 3).is_err());
    }
    #[test]
    fn assets_are_pinned_to_the_expected_repository_release() {
        let mut release = Release {
            tag_name: "v1.0.0".into(),
            draft: false,
            prerelease: false,
            assets: vec![Asset {
                name: "SHA256SUMS".into(),
                browser_download_url: format!(
                    "https://github.com/{REPO}/releases/download/v1.0.0/SHA256SUMS"
                ),
            }],
        };
        assert!(asset_url(&release, "SHA256SUMS").is_ok());
        release.assets[0].browser_download_url = "https://example.com/payload".into();
        assert!(asset_url(&release, "SHA256SUMS").is_err());
    }

    #[test]
    fn managed_layout_refuses_unrelated_launchers_and_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        let record = Installation {
            format: 1,
            install_root: temp.path().join("install"),
            bin_root: temp.path().join("launchers"),
        };
        for file in record.binary_paths() {
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, b"same executable").unwrap();
        }
        let current = record.binary_paths()[0].clone();
        assert!(record.validate(&current).is_ok());
        assert!(record.validate(&temp.path().join("unrelated")).is_err());
        let changed = record.binary_paths().last().unwrap().clone();
        fs::write(&changed, b"another program").unwrap();
        assert!(record.validate(&current).is_err());
        #[cfg(unix)]
        {
            fs::remove_file(&changed).unwrap();
            std::os::unix::fs::symlink(&current, &changed).unwrap();
            assert!(record.validate(&current).is_err());
        }
    }

    #[test]
    fn windows_zip_extraction_ignores_other_paths_and_requires_root_binary() {
        for name in ["agent-session-hub.exe", "nested/agent-session-hub.exe"] {
            let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"test executable").unwrap();
            let bytes = writer.finish().unwrap().into_inner();
            if name == "agent-session-hub.exe" {
                assert_eq!(extract_zip_binary(&bytes).unwrap(), b"test executable");
            } else {
                assert!(extract_zip_binary(&bytes).is_err());
            }
        }
        assert!(extract_zip_binary(b"not a ZIP file").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_helper_applies_and_rolls_back_with_unicode_paths() {
        use std::os::windows::fs::OpenOptionsExt;
        for locked in [false, true] {
            let stage = tempfile::Builder::new()
                .prefix("agent-session-hub-update-")
                .tempdir()
                .unwrap();
            let targets = tempfile::tempdir().unwrap();
            // Real installation records use canonical Windows extended paths.
            let canonical = targets.path().canonicalize().unwrap();
            let a = canonical.join("first è.exe");
            let b = canonical.join("second.exe");
            fs::write(&a, b"old a").unwrap();
            fs::write(&b, b"old b").unwrap();
            let source = stage.path().join("payload");
            fs::write(&source, b"new").unwrap();
            let files = [&a,&b].iter().map(|path|serde_json::json!({"destination":path,"source":source,"sha256":hash(&fs::read(path).unwrap())})).collect::<Vec<_>>();
            let plan = stage.path().join("plan.json");
            fs::write(
                &plan,
                serde_json::to_vec(&serde_json::json!({"stage":stage.path(),"files":files}))
                    .unwrap(),
            )
            .unwrap();
            let script = stage.path().join("apply.ps1");
            fs::write(&script, include_str!("update-windows.ps1")).unwrap();
            let _lock = locked.then(|| {
                fs::OpenOptions::new()
                    .read(true)
                    .share_mode(1)
                    .open(&b)
                    .unwrap()
            });
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
                !locked,
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                fs::read(&a).unwrap(),
                if locked {
                    b"old a".as_slice()
                } else {
                    b"new".as_slice()
                }
            );
            assert_eq!(
                fs::read(&b).unwrap(),
                if locked {
                    b"old b".as_slice()
                } else {
                    b"new".as_slice()
                }
            );
        }
    }
    #[cfg(not(windows))]
    #[test]
    fn archive_reads_only_the_root_executable_and_rejects_links() {
        for link in [false, true] {
            let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            let mut archive = tar::Builder::new(encoder);
            let mut header = tar::Header::new_gnu();
            header.set_mode(0o755);
            if link {
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_link_name("/etc/passwd").unwrap();
                header.set_size(0);
            } else {
                header.set_size(7);
            }
            header.set_cksum();
            archive
                .append_data(
                    &mut header,
                    "./agent-session-hub",
                    if link { &b""[..] } else { &b"payload"[..] },
                )
                .unwrap();
            let bytes = archive.into_inner().unwrap().finish().unwrap();
            if link {
                assert!(extract_binary(&bytes).is_err());
            } else {
                assert_eq!(extract_binary(&bytes).unwrap(), b"payload");
            }
        }
    }
    #[cfg(not(windows))]
    #[test]
    fn launcher_transaction_preserves_modes_and_rolls_back() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let a = temp.path().join("csx");
        let b = temp.path().join("clx");
        fs::write(&a, b"old a").unwrap();
        fs::write(&b, b"old b").unwrap();
        fs::set_permissions(&a, fs::Permissions::from_mode(0o755)).unwrap();
        replace_all(b"new", &[a.clone(), b.clone()]).unwrap();
        assert_eq!(fs::read(&a).unwrap(), b"new");
        assert_eq!(
            fs::metadata(&a).unwrap().permissions().mode() & 0o777,
            0o755
        );
        // A destination in /proc cannot be staged; the earlier successful write must roll back.
        #[cfg(target_os = "linux")]
        {
            let blocked = PathBuf::from("/proc/version");
            assert!(replace_all(b"broken", &[a.clone(), blocked]).is_err());
            assert_eq!(fs::read(&a).unwrap(), b"new");
        }
    }
}
