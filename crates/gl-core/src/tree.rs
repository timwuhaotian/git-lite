use crate::{FileEntry, Tree};
use anyhow::Result;
use std::path::Path;

pub fn snapshot_workdir(root: &Path) -> Result<Tree> {
    let mut tree = Tree::new();
    let walker = ignore::WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .ignore(true)
        .parents(true)
        .require_git(false)
        .filter_entry(|e| {
            e.file_name()
                .to_str()
                .map(|n| n != ".gl")
                .unwrap_or(true)
        })
        .build();
    for entry in walker {
        let entry = entry?;
        if !matches!(entry.file_type(), Some(ft) if ft.is_file()) {
            continue;
        }
        let bytes = std::fs::read(entry.path())?;
        let hash = crate::hash_bytes(&bytes);
        crate::store_object(root, &hash, &bytes)?;
        let rel = entry
            .path()
            .strip_prefix(root)
            .expect("walk entry under root")
            .to_string_lossy()
            .replace('\\', "/");
        tree.insert(
            rel,
            FileEntry {
                hash,
                size: bytes.len() as u64,
            },
        );
    }
    Ok(tree)
}
