use std::collections::BTreeMap;

pub mod commit;
pub mod hash;
pub mod layout;
pub mod object;
pub mod refs;
pub mod tree;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileEntry {
    pub hash: String,
    pub size: u64,
}

pub type Tree = BTreeMap<String, FileEntry>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Commit {
    pub id: String,
    pub parents: Vec<String>,
    pub message: String,
    pub author: String,
    pub time: i64,
    pub tree: Tree,
}

pub use commit::{commit_id, new_commit, read_commit, write_commit};
pub use hash::{hash_bytes, hash_file};
pub use layout::{current_branch, find_root, gl_dir, init_dir, read_head};
pub use object::{load_object, object_path, store_object};
pub use refs::{list_branches, log, normalize_branch, read_ref, update_ref};
pub use tree::snapshot_workdir;
