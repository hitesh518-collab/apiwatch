use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

fn temporary(path: &Path) -> Result<tempfile::NamedTempFile> {
    let parent = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create lockfile directory {}", parent.display()))?;
    tempfile::NamedTempFile::new_in(parent).context("failed to create temporary lockfile")
}

fn fill(mut file: tempfile::NamedTempFile, bytes: &[u8]) -> Result<tempfile::NamedTempFile> {
    file.write_all(bytes)
        .and_then(|_| file.flush())
        .and_then(|_| file.as_file().sync_all())
        .context("failed to write temporary lockfile")?;
    Ok(file)
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    fill(temporary(path)?, bytes)?
        .persist_noclobber(path)
        .map_err(|error| anyhow!("failed to create lockfile: {}", error.error))?;
    Ok(())
}

pub(super) fn replace(path: &Path, bytes: &[u8]) -> Result<()> {
    fill(temporary(path)?, bytes)?
        .persist(path)
        .map_err(|error| anyhow!("failed to replace lockfile: {}", error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_new_refuses_to_replace_existing_bytes() {
        let directory = tempfile::tempdir().expect("temp directory should create");
        let path = directory.path().join("api.lock");
        fs::write(&path, "preserve").expect("fixture should write");

        assert!(write_new(&path, b"replacement").is_err());
        assert_eq!(
            fs::read_to_string(path).expect("fixture should remain readable"),
            "preserve"
        );
    }

    #[test]
    fn replace_atomically_updates_existing_bytes() {
        let directory = tempfile::tempdir().expect("temp directory should create");
        let path = directory.path().join("api.lock");
        fs::write(&path, "old").expect("fixture should write");

        replace(&path, b"new").expect("replacement should succeed");

        assert_eq!(
            fs::read_to_string(path).expect("replacement should be readable"),
            "new"
        );
    }
}
