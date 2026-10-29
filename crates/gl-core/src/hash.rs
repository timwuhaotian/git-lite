use anyhow::Result;
use std::path::Path;

pub fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

pub fn hash_file(path: &Path) -> Result<(String, u64)> {
    let bytes = std::fs::read(path)?;
    let size = bytes.len() as u64;
    Ok((hash_bytes(&bytes), size))
}
