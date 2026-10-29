use crate::{Commit, gl_dir};
use anyhow::{Context, Result};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub fn normalize_branch(branch: &str) -> &str {
    branch.strip_prefix("refs/heads/").unwrap_or(branch)
}

fn ref_path(root: &Path, branch: &str) -> Result<PathBuf> {
    let name = normalize_branch(branch);
    if name.is_empty() || name.contains("..") {
        anyhow::bail!("invalid branch name: {branch:?}");
    }
    Ok(gl_dir(root).join("refs/heads").join(name))
}

pub fn update_ref(root: &Path, branch: &str, commit_id: &str) -> Result<()> {
    let path = ref_path(root, branch)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, format!("{commit_id}\n"))
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn read_ref(root: &Path, branch: &str) -> Result<Option<String>> {
    let path = ref_path(root, branch)?;
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let id = content.trim().to_string();
            Ok(if id.is_empty() { None } else { Some(id) })
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("read {}", path.display())),
    }
}

pub fn list_branches(root: &Path) -> Result<Vec<String>> {
    let base = gl_dir(root).join("refs/heads");
    let mut out = Vec::new();
    if !base.is_dir() {
        return Ok(out);
    }
    for entry in walkdir::WalkDir::new(&base)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let rel = entry
                .path()
                .strip_prefix(&base)
                .expect("walkdir path under base")
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
        }
    }
    out.sort();
    Ok(out)
}

pub fn log(root: &Path, branch: &str, n: usize) -> Result<Vec<Commit>> {
    let mut out = Vec::new();
    let mut current = read_ref(root, branch)?;
    while let Some(id) = current {
        if out.len() >= n {
            break;
        }
        let commit = crate::read_commit(root, &id)?;
        current = commit.parents.first().cloned();
        out.push(commit);
    }
    Ok(out)
}
