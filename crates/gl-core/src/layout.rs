use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn gl_dir(root: &Path) -> PathBuf {
    root.join(".gl")
}

pub fn init_dir(path: &Path) -> Result<()> {
    let gl = gl_dir(path);
    std::fs::create_dir_all(gl.join("refs/heads"))
        .with_context(|| format!("create {}", gl.join("refs/heads").display()))?;
    std::fs::create_dir_all(gl.join("objects"))?;
    std::fs::create_dir_all(gl.join("commits"))?;
    let config = gl.join("config.json");
    if !config.exists() {
        std::fs::write(&config, "{\"version\":1}\n")
            .with_context(|| format!("write {}", config.display()))?;
    }
    let head = gl.join("HEAD");
    if !head.exists() {
        std::fs::write(&head, "ref: refs/heads/main\n")
            .with_context(|| format!("write {}", head.display()))?;
    }
    Ok(())
}

pub fn find_root(cwd: &Path) -> Result<PathBuf> {
    let mut dir = if cwd.is_absolute() {
        cwd.to_path_buf()
    } else {
        std::env::current_dir()?.join(cwd)
    };
    if let Ok(canonical) = dir.canonicalize() {
        dir = canonical;
    }
    if dir.is_file() {
        dir = dir
            .parent()
            .map(Path::to_path_buf)
            .context("file has no parent directory")?;
    }
    for ancestor in dir.ancestors() {
        if ancestor.join(".gl").is_dir() {
            return Ok(ancestor.to_path_buf());
        }
    }
    anyhow::bail!("not inside a gl repo (no .gl found above {})", dir.display())
}

pub fn read_head(root: &Path) -> Result<String> {
    let head = gl_dir(root).join("HEAD");
    Ok(std::fs::read_to_string(&head)
        .with_context(|| format!("read {}", head.display()))?
        .trim()
        .to_string())
}

pub fn current_branch(root: &Path) -> Result<Option<String>> {
    let head = read_head(root)?;
    Ok(head
        .strip_prefix("ref:")
        .map(str::trim)
        .and_then(|r| r.strip_prefix("refs/heads/"))
        .map(str::to_string))
}
