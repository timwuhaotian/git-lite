use crate::{Commit, Tree};
use anyhow::{Context, Result};
use std::path::Path;

pub fn commit_id(
    parents: &[String],
    message: &str,
    author: &str,
    time: i64,
    tree: &Tree,
) -> String {
    let payload = serde_json::json!({
        "parents": parents,
        "message": message,
        "author": author,
        "time": time,
        "tree": tree,
    });
    blake3::hash(&serde_json::to_vec(&payload).expect("commit payload serializes"))
        .to_hex()
        .to_string()
}

pub fn new_commit(
    parents: Vec<String>,
    message: String,
    author: String,
    time: i64,
    tree: Tree,
) -> Commit {
    let id = commit_id(&parents, &message, &author, time, &tree);
    Commit {
        id,
        parents,
        message,
        author,
        time,
        tree,
    }
}

pub fn write_commit(root: &Path, commit: &Commit) -> Result<()> {
    let dir = crate::gl_dir(root).join("commits");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", commit.id));
    let data = serde_json::to_vec(commit)?;
    std::fs::write(&path, data).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn read_commit(root: &Path, id: &str) -> Result<Commit> {
    let path = crate::gl_dir(root)
        .join("commits")
        .join(format!("{id}.json"));
    let data =
        std::fs::read(&path).with_context(|| format!("commit {id} missing"))?;
    Ok(serde_json::from_slice(&data)?)
}
