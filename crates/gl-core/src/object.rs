use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn object_path(root: &Path, hash: &str) -> Result<PathBuf> {
    if hash.len() < 2 {
        anyhow::bail!("invalid object hash: {hash:?}");
    }
    Ok(crate::gl_dir(root)
        .join("objects")
        .join(&hash[..2])
        .join(hash))
}

pub fn store_object(root: &Path, hash: &str, bytes: &[u8]) -> Result<()> {
    let path = object_path(root, hash)?;
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn load_object(root: &Path, hash: &str) -> Result<Vec<u8>> {
    let path = object_path(root, hash)?;
    let bytes = std::fs::read(&path)
        .with_context(|| format!("object {hash} missing at {}", path.display()))?;
    let actual = crate::hash_bytes(&bytes);
    if actual != hash {
        anyhow::bail!("object {hash} corrupt (recomputed {actual})");
    }
    Ok(bytes)
}
