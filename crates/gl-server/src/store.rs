use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Store {
    pub root: PathBuf,
}

impl Store {
    pub fn from_env() -> Self {
        let base = std::env::var("GL_DATA").unwrap_or_else(|_| "./data".to_string());
        Self {
            root: PathBuf::from(base).join("repos"),
        }
    }

    pub fn repo_dir(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn refs_path(&self, name: &str) -> PathBuf {
        self.repo_dir(name).join("refs.json")
    }

    fn commit_path(&self, name: &str, id: &str) -> PathBuf {
        self.repo_dir(name).join("commits").join(format!("{id}.json"))
    }

    fn object_path(&self, name: &str, hash: &str) -> PathBuf {
        let prefix = if hash.len() >= 2 { &hash[..2] } else { "xx" };
        self.repo_dir(name).join("objects").join(prefix).join(hash)
    }

    pub fn valid_name(name: &str) -> bool {
        !name.is_empty()
            && name.len() <= 128
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            && name != "."
            && name != ".."
    }

    pub fn valid_id(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= 256
            && !id.contains('/')
            && !id.contains('\\')
            && id != "."
            && id != ".."
    }

    pub fn list_repos(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&self.root) {
            for e in rd.flatten() {
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    if let Some(n) = e.file_name().to_str() {
                        out.push(n.to_string());
                    }
                }
            }
        }
        out.sort();
        out
    }

    pub fn read_refs(&self, name: &str) -> BTreeMap<String, String> {
        std::fs::read_to_string(self.refs_path(name))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn write_refs(&self, name: &str, refs: &BTreeMap<String, String>) -> anyhow::Result<()> {
        let p = self.refs_path(name);
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(refs)?)?;
        std::fs::rename(&tmp, &p)?;
        Ok(())
    }

    pub fn read_commit(&self, name: &str, id: &str) -> Option<serde_json::Value> {
        std::fs::read_to_string(self.commit_path(name, id))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
    }

    pub fn store_commit(
        &self,
        name: &str,
        id: &str,
        v: &serde_json::Value,
    ) -> anyhow::Result<bool> {
        let p = self.commit_path(name, id);
        if p.exists() {
            return Ok(false);
        }
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string(v)?)?;
        std::fs::rename(&tmp, &p)?;
        Ok(true)
    }

    pub fn read_object(&self, name: &str, hash: &str) -> Option<Vec<u8>> {
        std::fs::read(self.object_path(name, hash)).ok()
    }

    pub fn store_object(&self, name: &str, hash: &str, data: &[u8]) -> anyhow::Result<bool> {
        let p = self.object_path(name, hash);
        if p.exists() {
            return Ok(false);
        }
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = p.with_extension("tmp");
        std::fs::write(&tmp, data)?;
        std::fs::rename(&tmp, &p)?;
        Ok(true)
    }

    pub fn is_ancestor(&self, name: &str, old: &str, new: &str) -> bool {
        if old == new {
            return true;
        }
        let mut seen = HashSet::new();
        let mut stack = vec![new.to_string()];
        while let Some(id) = stack.pop() {
            if id == old {
                return true;
            }
            if !seen.insert(id.clone()) {
                continue;
            }
            if seen.len() > 100_000 {
                return false;
            }
            if let Some(v) = self.read_commit(name, &id) {
                if let Some(parents) = v.get("parents").and_then(|p| p.as_array()) {
                    for p in parents {
                        if let Some(s) = p.as_str() {
                            stack.push(s.to_string());
                        }
                    }
                }
            }
        }
        false
    }

    pub fn log(&self, name: &str, branch: Option<&str>, n: usize) -> Vec<serde_json::Value> {
        let refs = self.read_refs(name);
        let head = match branch {
            Some(b) => refs.get(b).cloned(),
            None => refs
                .get("main")
                .or_else(|| refs.get("master"))
                .or_else(|| refs.values().next())
                .cloned(),
        };
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut stack = head.into_iter().collect::<Vec<_>>();
        while let Some(id) = stack.pop() {
            if out.len() >= n || !seen.insert(id.clone()) {
                continue;
            }
            if let Some(v) = self.read_commit(name, &id) {
                if let Some(parents) = v.get("parents").and_then(|p| p.as_array()) {
                    for p in parents {
                        if let Some(s) = p.as_str() {
                            stack.push(s.to_string());
                        }
                    }
                }
                out.push(v);
            }
        }
        out
    }
}
