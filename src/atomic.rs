use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::Path;

/// Stage beside the destination so replacement never crosses filesystems.
/// A failed write/flush leaves the old destination intact.
pub fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .context("destination has no parent directory")?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("stage {}", path.display()))?;
    staged.write_all(bytes).context("write staged file")?;
    if let Ok(metadata) = fs::metadata(path) {
        staged.as_file().set_permissions(metadata.permissions())?;
    }
    staged.as_file().sync_all().context("flush staged file")?;
    staged
        .persist(path)
        .with_context(|| format!("replace {}", path.display()))?;
    #[cfg(unix)]
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}
