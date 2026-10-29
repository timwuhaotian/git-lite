use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "gl", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Init {
        #[arg(long)]
        server: Option<String>,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long, default_value = "main")]
        branch: String,
        #[arg(long)]
        author: Option<String>,
    },
    Config {
        key: Option<String>,
        value: Option<String>,
        #[arg(long)]
        list: bool,
    },
    Save {
        #[arg(short, long)]
        message: String,
        #[arg(long)]
        author: Option<String>,
    },
    Log {
        #[arg(short = 'n', long, default_value_t = 20)]
        number: usize,
        #[arg(long)]
        branch: Option<String>,
    },
    Branch {
        name: Option<String>,
    },
    Switch {
        name: String,
    },
    Status,
    Worktree {
        #[command(subcommand)]
        cmd: WorktreeCmd,
    },
    Push,
    Pull,
}

#[derive(Subcommand)]
enum WorktreeCmd {
    Add { path: PathBuf, branch: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    server: String,
    repo: String,
    branch: String,
    author: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TreeEntry {
    hash: String,
    size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Commit {
    id: String,
    parents: Vec<String>,
    message: String,
    author: String,
    time: u64,
    tree: BTreeMap<String, TreeEntry>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn default_author() -> String {
    env::var("USER")
        .or_else(|_| env::var("USERNAME"))
        .unwrap_or_else(|_| "local".to_string())
}

fn find_root() -> Result<PathBuf> {
    let mut dir = env::current_dir()?;
    loop {
        if dir.join(".gl").is_dir() {
            return Ok(dir);
        }
        if !dir.pop() {
            bail!("not a gl repo (no .gl found)");
        }
    }
}

fn gl_dir(root: &Path) -> PathBuf {
    root.join(".gl")
}

fn load_config(root: &Path) -> Result<Config> {
    let raw = fs::read(gl_dir(root).join("config.json")).context("cannot read .gl/config.json")?;
    Ok(serde_json::from_slice(&raw)?)
}

fn save_config(root: &Path, cfg: &Config) -> Result<()> {
    let p = gl_dir(root).join("config.json");
    fs::write(p, serde_json::to_string_pretty(cfg)?)?;
    Ok(())
}

fn current_branch(root: &Path, cfg: &Config) -> Result<String> {
    let p = gl_dir(root).join("HEAD");
    if p.is_file() {
        let raw = fs::read_to_string(&p)?.trim().to_string();
        let b = raw
            .strip_prefix("ref: refs/heads/")
            .unwrap_or(&raw)
            .to_string();
        if !b.is_empty() {
            return Ok(b);
        }
    }
    Ok(cfg.branch.clone())
}

fn write_head(root: &Path, branch: &str) -> Result<()> {
    fs::write(gl_dir(root).join("HEAD"), branch)?;
    Ok(())
}

fn validate_relative_path(path: &str) -> Result<()> {
    if path.is_empty()
        || !Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
    {
        bail!("invalid relative path {path:?}");
    }
    Ok(())
}

fn ref_path(root: &Path, branch: &str) -> Result<PathBuf> {
    validate_relative_path(branch)?;
    Ok(gl_dir(root).join("refs").join("heads").join(branch))
}

fn read_tip(root: &Path, branch: &str) -> Result<Option<String>> {
    let p = ref_path(root, branch)?;
    if !p.is_file() {
        return Ok(None);
    }
    let id = fs::read_to_string(&p)?.trim().to_string();
    Ok(if id.is_empty() { None } else { Some(id) })
}

fn write_tip(root: &Path, branch: &str, id: &str) -> Result<()> {
    let p = ref_path(root, branch)?;
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(p, id)?;
    Ok(())
}

fn list_branches(root: &Path) -> Result<Vec<String>> {
    let d = gl_dir(root).join("refs").join("heads");
    let mut out = Vec::new();
    if d.is_dir() {
        for e in fs::read_dir(d)? {
            let e = e?;
            if e.file_type()?.is_file() {
                out.push(e.file_name().to_string_lossy().into_owned());
            }
        }
    }
    out.sort();
    Ok(out)
}

fn object_path(root: &Path, hash: &str) -> PathBuf {
    gl_dir(root)
        .join("objects")
        .join(&hash[..2.min(hash.len())])
        .join(hash)
}

fn store_object(root: &Path, bytes: &[u8]) -> Result<String> {
    let hash = blake3::hash(bytes).to_hex().to_string();
    let p = object_path(root, &hash);
    if !p.is_file() {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = p.with_extension("tmp");
        fs::write(&tmp, bytes)?;
        fs::rename(&tmp, &p)?;
    }
    Ok(hash)
}

fn load_commit(root: &Path, id: &str) -> Result<Commit> {
    let p = gl_dir(root).join("commits").join(format!("{id}.json"));
    let raw = fs::read(&p).with_context(|| format!("missing commit {id}"))?;
    Ok(serde_json::from_slice(&raw)?)
}

fn store_commit(root: &Path, c: &Commit) -> Result<()> {
    let d = gl_dir(root).join("commits");
    fs::create_dir_all(&d)?;
    fs::write(
        d.join(format!("{}.json", c.id)),
        serde_json::to_string_pretty(c)?,
    )?;
    Ok(())
}

fn commit_id(
    parents: &[String],
    message: &str,
    author: &str,
    time: u64,
    tree: &BTreeMap<String, TreeEntry>,
) -> String {
    let v = serde_json::json!({
        "parents": parents,
        "message": message,
        "author": author,
        "time": time,
        "tree": tree,
    });
    blake3::hash(serde_json::to_string(&v).unwrap_or_default().as_bytes())
        .to_hex()
        .to_string()
}

fn rel_key(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()?
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
        .into()
}

fn scan_tree(root: &Path) -> Result<BTreeMap<String, TreeEntry>> {
    let mut tree = BTreeMap::new();
    for entry in WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if p.components().any(|c| c.as_os_str() == ".gl") {
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let bytes = fs::read(p)?;
        let hash = store_object(root, &bytes)?;
        if let Some(key) = rel_key(root, p) {
            tree.insert(
                key,
                TreeEntry {
                    hash,
                    size: bytes.len() as u64,
                },
            );
        }
    }
    Ok(tree)
}

fn ancestors_local(root: &Path, tip: &str, stop: &BTreeSet<String>) -> Result<Vec<Commit>> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let mut stack = vec![tip.to_string()];
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) || stop.contains(&id) {
            continue;
        }
        let c = load_commit(root, &id)?;
        stack.extend(c.parents.iter().cloned());
        out.push(c);
    }
    Ok(out)
}

fn checkout(
    root: &Path,
    old: &BTreeMap<String, TreeEntry>,
    new: &BTreeMap<String, TreeEntry>,
) -> Result<()> {
    for key in old.keys().chain(new.keys()) {
        validate_relative_path(key)?;
    }

    let mut removed = Vec::new();
    for (key, old_entry) in old {
        let changed = match new.get(key) {
            None => {
                removed.push(key);
                true
            }
            Some(new_entry) => old_entry.hash != new_entry.hash || old_entry.size != new_entry.size,
        };
        if changed {
            let bytes = fs::read(root.join(key))
                .with_context(|| format!("local changes would be overwritten: {key}"))?;
            if bytes.len() as u64 != old_entry.size
                || blake3::hash(&bytes).to_hex().to_string() != old_entry.hash
            {
                bail!("local changes would be overwritten: {key}");
            }
        }
    }
    for key in new.keys() {
        if old.contains_key(key) {
            continue;
        }
        match fs::symlink_metadata(root.join(key)) {
            Ok(_) => bail!("path already exists: {key}"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }

    let mut writes = Vec::new();
    for (key, new_entry) in new {
        let changed = !old.get(key).is_some_and(|old_entry| {
            old_entry.hash == new_entry.hash && old_entry.size == new_entry.size
        });
        if changed {
            let bytes = fs::read(object_path(root, &new_entry.hash))
                .with_context(|| format!("missing object {}", new_entry.hash))?;
            writes.push((key, bytes));
        }
    }

    for key in removed {
        fs::remove_file(root.join(key))?;
    }
    for (key, bytes) in writes {
        let p = root.join(key);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(p, bytes)?;
    }
    Ok(())
}

fn tip_tree(root: &Path, branch: &str) -> Result<BTreeMap<String, TreeEntry>> {
    match read_tip(root, branch)? {
        Some(id) => Ok(load_commit(root, &id)?.tree),
        None => Ok(BTreeMap::new()),
    }
}

fn api(cfg: &Config, path: &str) -> String {
    format!("{}{}", cfg.server.trim_end_matches('/'), path)
}

fn http() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?)
}

fn get_refs(cfg: &Config) -> Result<BTreeMap<String, String>> {
    let url = api(cfg, &format!("/api/repos/{}/refs", cfg.repo));
    let resp = http()?.get(&url).send().context("GET refs failed")?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(BTreeMap::new());
    }
    if !resp.status().is_success() {
        bail!("GET refs failed: {}", resp.status());
    }
    let v: serde_json::Value = resp.json()?;
    let map = if let Some(o) = v.get("refs").and_then(|x| x.as_object()) {
        o.clone()
    } else if let Some(o) = v.get("branches").and_then(|x| x.as_object()) {
        o.clone()
    } else if let Some(o) = v.as_object() {
        o.clone()
    } else {
        bail!("bad refs response");
    };
    let mut out = BTreeMap::new();
    for (k, val) in map {
        if let Some(s) = val.as_str() {
            out.insert(k, s.to_string());
        } else if let Some(id) = val.get("id").and_then(|x| x.as_str()) {
            out.insert(k, id.to_string());
        }
    }
    Ok(out)
}

fn parse_commit_list(v: &serde_json::Value) -> Result<Vec<Commit>> {
    let arr = if let Some(a) = v.as_array() {
        a.clone()
    } else if let Some(a) = v.get("commits").and_then(|x| x.as_array()) {
        a.clone()
    } else if let Some(a) = v.get("log").and_then(|x| x.as_array()) {
        a.clone()
    } else if let Some(a) = v.get("history").and_then(|x| x.as_array()) {
        a.clone()
    } else {
        bail!("bad log response");
    };
    let mut out = Vec::new();
    for item in arr {
        out.push(serde_json::from_value(item)?);
    }
    Ok(out)
}

fn get_log(cfg: &Config, branch: &str, n: usize) -> Result<Vec<Commit>> {
    let url = api(
        cfg,
        &format!("/api/repos/{}/log?branch={}&n={}", cfg.repo, branch, n),
    );
    let resp = http()?.get(&url).send().context("GET log failed")?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(Vec::new());
    }
    if !resp.status().is_success() {
        bail!("GET log failed: {}", resp.status());
    }
    parse_commit_list(&resp.json()?)
}

fn get_object(cfg: &Config, hash: &str) -> Result<Vec<u8>> {
    let url = api(cfg, &format!("/api/repos/{}/objects/{}", cfg.repo, hash));
    let resp = http()?.get(&url).send().context("GET object failed")?;
    if !resp.status().is_success() {
        bail!("GET object {hash} failed: {}", resp.status());
    }
    Ok(resp.bytes()?.to_vec())
}

fn post_object(cfg: &Config, hash: &str, bytes: &[u8]) -> Result<()> {
    let url = api(cfg, &format!("/api/repos/{}/objects/{}", cfg.repo, hash));
    let resp = http()?
        .post(&url)
        .header("Content-Type", "application/octet-stream")
        .body(bytes.to_vec())
        .send()
        .context("POST object failed")?;
    if !resp.status().is_success() {
        bail!("POST object {hash} failed: {}", resp.status());
    }
    Ok(())
}

fn post_push(cfg: &Config, branch: &str, tip_id: &str, commits: &[Commit]) -> Result<()> {
    let url = api(cfg, &format!("/api/repos/{}/push", cfg.repo));
    let body = serde_json::json!({"refs": {branch: tip_id}, "commits": commits});
    let resp = http()?
        .post(&url)
        .json(&body)
        .send()
        .context("POST push failed")?;
    if !resp.status().is_success() {
        bail!("POST push failed: {}", resp.status());
    }
    Ok(())
}

fn has_commit(root: &Path, id: &str) -> bool {
    gl_dir(root)
        .join("commits")
        .join(format!("{id}.json"))
        .is_file()
}

fn short(id: &str) -> &str {
    &id[..8.min(id.len())]
}

fn cmd_init(
    server: Option<String>,
    repo: Option<String>,
    branch: &str,
    author: Option<String>,
) -> Result<()> {
    let root = env::current_dir()?;
    if root.join(".gl").is_dir() {
        bail!("already initialized");
    }
    let repo = repo.unwrap_or_else(|| {
        root.file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "repo".to_string())
    });
    let cfg = Config {
        server: server.unwrap_or_else(|| "http://127.0.0.1:3000".to_string()),
        repo,
        branch: branch.to_string(),
        author: author.unwrap_or_else(default_author),
    };
    fs::create_dir_all(gl_dir(&root).join("refs").join("heads"))?;
    fs::create_dir_all(gl_dir(&root).join("objects"))?;
    fs::create_dir_all(gl_dir(&root).join("commits"))?;
    save_config(&root, &cfg)?;
    write_head(&root, &cfg.branch)?;
    println!("init {} branch={}", cfg.repo, cfg.branch);
    Ok(())
}

fn cmd_config(root: &Path, key: Option<&str>, value: Option<&str>, list: bool) -> Result<()> {
    let mut cfg = load_config(root)?;
    if list || key.is_none() {
        println!("server={}", cfg.server);
        println!("repo={}", cfg.repo);
        println!("branch={}", cfg.branch);
        println!("author={}", cfg.author);
        return Ok(());
    }
    let key = key.unwrap();
    match value {
        None => match key {
            "server" => println!("{}", cfg.server),
            "repo" => println!("{}", cfg.repo),
            "branch" => println!("{}", cfg.branch),
            "author" => println!("{}", cfg.author),
            _ => bail!("unknown key {key}"),
        },
        Some(v) => {
            match key {
                "server" => cfg.server = v.trim_end_matches('/').to_string(),
                "repo" => cfg.repo = v.to_string(),
                "branch" => cfg.branch = v.to_string(),
                "author" => cfg.author = v.to_string(),
                _ => bail!("unknown key {key}"),
            }
            save_config(root, &cfg)?;
        }
    }
    Ok(())
}

fn cmd_save(root: &Path, message: &str, author: Option<&str>) -> Result<()> {
    let cfg = load_config(root)?;
    let branch = current_branch(root, &cfg)?;
    let tree = scan_tree(root)?;
    let parent = read_tip(root, &branch)?;
    let parents = parent.into_iter().collect::<Vec<_>>();
    let author = author.unwrap_or(&cfg.author).to_string();
    let time = now_secs();
    let id = commit_id(&parents, message, &author, time, &tree);
    let c = Commit {
        id: id.clone(),
        parents,
        message: message.to_string(),
        author,
        time,
        tree,
    };
    store_commit(root, &c)?;
    write_tip(root, &branch, &id)?;
    println!("{} {}", short(&id), message);
    Ok(())
}

fn cmd_log(root: &Path, n: usize, branch: Option<&str>) -> Result<()> {
    let cfg = load_config(root)?;
    let branch = branch
        .map(|s| s.to_string())
        .unwrap_or(current_branch(root, &cfg)?);
    let Some(mut id) = read_tip(root, &branch)? else {
        return Ok(());
    };
    let mut count = 0;
    let mut seen = BTreeSet::new();
    while count < n && seen.insert(id.clone()) {
        let c = load_commit(root, &id)?;
        println!("{} {} {} {}", short(&c.id), c.author, c.time, c.message);
        match c.parents.first() {
            Some(p) => id = p.clone(),
            None => break,
        }
        count += 1;
    }
    Ok(())
}

fn cmd_branch(root: &Path, name: Option<&str>) -> Result<()> {
    let cfg = load_config(root)?;
    let cur = current_branch(root, &cfg)?;
    match name {
        None => {
            for b in list_branches(root)? {
                if b == cur {
                    println!("* {b}");
                } else {
                    println!("  {b}");
                }
            }
            Ok(())
        }
        Some(n) => {
            let tip = read_tip(root, &cur)?.ok_or_else(|| anyhow!("no commits yet"))?;
            write_tip(root, n, &tip)?;
            println!("branch {n} at {}", short(&tip));
            Ok(())
        }
    }
}

fn cmd_switch(root: &Path, name: &str) -> Result<()> {
    let mut cfg = load_config(root)?;
    let cur = current_branch(root, &cfg)?;
    if cur == name {
        return Ok(());
    }
    let target_tip = read_tip(root, name)?.ok_or_else(|| anyhow!("unknown branch {name}"))?;
    let old = tip_tree(root, &cur)?;
    let new = load_commit(root, &target_tip)?.tree;
    checkout(root, &old, &new)?;
    write_head(root, name)?;
    cfg.branch = name.to_string();
    save_config(root, &cfg)?;
    println!("switch {name}");
    Ok(())
}

fn cmd_status(root: &Path) -> Result<()> {
    let cfg = load_config(root)?;
    let branch = current_branch(root, &cfg)?;
    let tip = read_tip(root, &branch)?;
    println!("branch {branch}");
    let committed = match &tip {
        Some(id) => load_commit(root, id)?.tree,
        None => BTreeMap::new(),
    };
    let mut work_hashes: BTreeMap<String, String> = BTreeMap::new();
    for entry in WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if p.components().any(|c| c.as_os_str() == ".gl") {
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        if let Some(key) = rel_key(root, p) {
            let bytes = fs::read(p)?;
            work_hashes.insert(key, blake3::hash(&bytes).to_hex().to_string());
        }
    }
    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut deleted = Vec::new();
    for (k, h) in &work_hashes {
        match committed.get(k) {
            None => added.push(k),
            Some(e) if &e.hash != h => modified.push(k),
            _ => {}
        }
    }
    for k in committed.keys() {
        if !work_hashes.contains_key(k) {
            deleted.push(k);
        }
    }
    println!("tip {}", tip.as_deref().unwrap_or("(none)"));
    for k in added {
        println!("A {k}");
    }
    for k in modified {
        println!("M {k}");
    }
    for k in deleted {
        println!("D {k}");
    }
    Ok(())
}

fn worktree_root(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn cmd_worktree_add(root: &Path, path: &Path, branch: &str) -> Result<()> {
    let cfg = load_config(root)?;
    let cur = current_branch(root, &cfg)?;
    let nroot = worktree_root(&env::current_dir()?, path);
    if nroot.exists() {
        bail!("path already exists");
    }
    let base_tip = read_tip(root, branch)?.or(read_tip(root, &cur)?);
    fs::create_dir_all(&nroot)?;
    fs::create_dir_all(gl_dir(&nroot).join("refs").join("heads"))?;
    fs::create_dir_all(gl_dir(&nroot).join("objects"))?;
    fs::create_dir_all(gl_dir(&nroot).join("commits"))?;
    let ncfg = Config {
        server: cfg.server.clone(),
        repo: cfg.repo.clone(),
        branch: branch.to_string(),
        author: cfg.author.clone(),
    };
    save_config(&nroot, &ncfg)?;
    write_head(&nroot, branch)?;
    for id in walk_all_ids(root)? {
        let src = gl_dir(root).join("commits").join(format!("{id}.json"));
        let dst = gl_dir(&nroot).join("commits").join(format!("{id}.json"));
        fs::copy(src, dst)?;
    }
    let empty = BTreeMap::new();
    if let Some(tip) = base_tip {
        write_tip(&nroot, branch, &tip)?;
        let tree = load_commit(root, &tip)?.tree;
        for e in tree.values() {
            let src = object_path(root, &e.hash);
            let dst = object_path(&nroot, &e.hash);
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            if !dst.is_file() {
                fs::copy(src, &dst)?;
            }
        }
        checkout(&nroot, &empty, &tree)?;
    }
    println!("worktree {} branch={branch}", nroot.display());
    Ok(())
}

fn walk_all_ids(root: &Path) -> Result<Vec<String>> {
    let d = gl_dir(root).join("commits");
    let mut out = Vec::new();
    if d.is_dir() {
        for e in fs::read_dir(d)? {
            let e = e?;
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some(id) = name.strip_suffix(".json") {
                out.push(id.to_string());
            }
        }
    }
    Ok(out)
}

fn cmd_push(root: &Path) -> Result<()> {
    let cfg = load_config(root)?;
    let branch = current_branch(root, &cfg)?;
    let tip_id = read_tip(root, &branch)?.ok_or_else(|| anyhow!("nothing to push"))?;
    let remote = get_refs(&cfg)?;
    let mut known = BTreeSet::new();
    if let Some(rid) = remote.get(&branch) {
        known.insert(rid.clone());
        if let Ok(cs) = ancestors_local(root, rid, &BTreeSet::new()) {
            known.extend(cs.into_iter().map(|c| c.id));
        }
    }
    let missing = ancestors_local(root, &tip_id, &known)?;
    let mut hashes = BTreeSet::new();
    for c in &missing {
        for e in c.tree.values() {
            hashes.insert(e.hash.clone());
        }
    }
    for h in &hashes {
        let bytes =
            fs::read(object_path(root, h)).with_context(|| format!("missing local object {h}"))?;
        post_object(&cfg, h, &bytes)?;
    }
    post_push(&cfg, &branch, &tip_id, &missing)?;
    println!(
        "push {branch} {} ({} commits, {} objects)",
        short(&tip_id),
        missing.len(),
        hashes.len()
    );
    Ok(())
}

fn cmd_pull(root: &Path) -> Result<()> {
    let cfg = load_config(root)?;
    let branch = current_branch(root, &cfg)?;
    let remote = get_refs(&cfg)?;
    let rid = remote
        .get(&branch)
        .ok_or_else(|| anyhow!("no remote branch {branch}"))?;
    let local_tip = read_tip(root, &branch)?;
    if local_tip.as_deref() == Some(rid.as_str()) {
        println!("up to date");
        return Ok(());
    }
    let remote_commits = get_log(&cfg, &branch, 1_000_000)?;
    let mut by_id = BTreeMap::new();
    for c in remote_commits {
        by_id.insert(c.id.clone(), c);
    }
    if !by_id.contains_key(rid) {
        bail!("remote tip not in log");
    }
    let mut need: Vec<Commit> = Vec::new();
    let mut seen = BTreeSet::new();
    let mut stack = vec![rid.clone()];
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if has_commit(root, &id) {
            let cs = ancestors_local(root, &id, &BTreeSet::new()).unwrap_or_default();
            seen.extend(cs.into_iter().map(|c| c.id));
            continue;
        }
        let c = by_id
            .get(&id)
            .ok_or_else(|| anyhow!("remote commit {id} missing from log"))?
            .clone();
        stack.extend(c.parents.iter().cloned());
        need.push(c);
    }
    for c in &need {
        store_commit(root, c)?;
    }
    let mut objs = BTreeSet::new();
    for c in &need {
        for e in c.tree.values() {
            objs.insert(e.hash.clone());
        }
    }
    for h in &objs {
        if object_path(root, h).is_file() {
            continue;
        }
        let bytes = get_object(&cfg, h)?;
        let got = blake3::hash(&bytes).to_hex().to_string();
        if &got != h {
            bail!("object {h} checksum mismatch");
        }
        let p = object_path(root, h);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(p, bytes)?;
    }
    let ok = match &local_tip {
        None => true,
        Some(lt) => {
            let mut s = BTreeSet::new();
            let mut st = vec![rid.clone()];
            let mut found = false;
            while let Some(id) = st.pop() {
                if !s.insert(id.clone()) {
                    continue;
                }
                if &id == lt {
                    found = true;
                    break;
                }
                let c = load_commit(root, &id)?;
                st.extend(c.parents.iter().cloned());
            }
            found
        }
    };
    if !ok {
        bail!("diverged: local and remote both moved, manual fix needed");
    }
    let old = tip_tree(root, &branch)?;
    let new = load_commit(root, rid)?.tree;
    checkout(root, &old, &new)?;
    write_tip(root, &branch, rid)?;
    println!("pull {branch} {}", short(rid));
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Init {
            server,
            repo,
            branch,
            author,
        } => cmd_init(server, repo, &branch, author),
        Cmd::Config { key, value, list } => {
            let root = find_root()?;
            cmd_config(&root, key.as_deref(), value.as_deref(), list)
        }
        Cmd::Save { message, author } => {
            let root = find_root()?;
            cmd_save(&root, &message, author.as_deref())
        }
        Cmd::Log { number, branch } => {
            let root = find_root()?;
            cmd_log(&root, number, branch.as_deref())
        }
        Cmd::Branch { name } => {
            let root = find_root()?;
            cmd_branch(&root, name.as_deref())
        }
        Cmd::Switch { name } => {
            let root = find_root()?;
            cmd_switch(&root, &name)
        }
        Cmd::Status => {
            let root = find_root()?;
            cmd_status(&root)
        }
        Cmd::Worktree { cmd } => {
            let WorktreeCmd::Add { path, branch } = cmd;
            let root = find_root()?;
            cmd_worktree_add(&root, &path, &branch)
        }
        Cmd::Push => {
            let root = find_root()?;
            cmd_push(&root)
        }
        Cmd::Pull => {
            let root = find_root()?;
            cmd_pull(&root)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Result<Self> {
            let path = env::temp_dir().join(format!(
                "gl-client-test-{}-{}",
                std::process::id(),
                NEXT_DIR.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path)?;
            Ok(Self(path))
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn entry(root: &Path, bytes: &[u8]) -> Result<TreeEntry> {
        Ok(TreeEntry {
            hash: store_object(root, bytes)?,
            size: bytes.len() as u64,
        })
    }

    #[test]
    fn checkout_rejects_path_escape_before_changes() -> Result<()> {
        let dir = TestDir::new()?;
        let root = &dir.0;
        fs::write(root.join("tracked"), b"old")?;
        let old = BTreeMap::from([("tracked".to_string(), entry(root, b"old")?)]);
        let new = BTreeMap::from([("../escape".to_string(), entry(root, b"new")?)]);

        assert!(checkout(root, &old, &new).is_err());
        assert_eq!(fs::read(root.join("tracked"))?, b"old");
        assert!(!root.parent().unwrap().join("escape").exists());

        for path in ["", "/absolute", "safe/../../escape", "."] {
            assert!(validate_relative_path(path).is_err(), "accepted {path:?}");
        }
        assert!(ref_path(root, "../escape").is_err());
        assert!(ref_path(root, "/absolute").is_err());
        Ok(())
    }

    #[test]
    fn checkout_preserves_unsaved_changes() -> Result<()> {
        let dir = TestDir::new()?;
        let root = &dir.0;
        let old = BTreeMap::from([("file".to_string(), entry(root, b"old")?)]);
        let new = BTreeMap::from([("file".to_string(), entry(root, b"new")?)]);
        fs::write(root.join("file"), b"dirty")?;

        assert!(checkout(root, &old, &new).is_err());
        assert_eq!(fs::read(root.join("file"))?, b"dirty");

        let added = BTreeMap::from([("added".to_string(), entry(root, b"new")?)]);
        fs::write(root.join("added"), b"existing")?;
        assert!(checkout(root, &BTreeMap::new(), &added).is_err());
        assert_eq!(fs::read(root.join("added"))?, b"existing");
        Ok(())
    }

    #[test]
    fn checkout_clean_switch_skips_unchanged_entries() -> Result<()> {
        let dir = TestDir::new()?;
        let root = &dir.0;
        let unchanged = entry(root, b"committed")?;
        let old = BTreeMap::from([
            ("changed".to_string(), entry(root, b"old")?),
            ("removed".to_string(), entry(root, b"removed")?),
            ("unchanged".to_string(), unchanged.clone()),
        ]);
        let new = BTreeMap::from([
            ("changed".to_string(), entry(root, b"new")?),
            ("added".to_string(), entry(root, b"added")?),
            ("unchanged".to_string(), unchanged),
        ]);
        fs::write(root.join("changed"), b"old")?;
        fs::write(root.join("removed"), b"removed")?;
        fs::write(root.join("unchanged"), b"dirty")?;

        checkout(root, &old, &new)?;
        checkout(root, &BTreeMap::new(), &BTreeMap::new())?;
        assert_eq!(fs::read(root.join("changed"))?, b"new");
        assert_eq!(fs::read(root.join("added"))?, b"added");
        assert_eq!(fs::read(root.join("unchanged"))?, b"dirty");
        assert!(!root.join("removed").exists());
        Ok(())
    }

    #[test]
    fn worktree_relative_path_uses_command_cwd() {
        let cwd = env::temp_dir().join("command-cwd");
        assert_eq!(
            worktree_root(&cwd, Path::new("nested/worktree")),
            cwd.join("nested/worktree")
        );
        let absolute = env::temp_dir().join("absolute-worktree");
        assert_eq!(worktree_root(&cwd, &absolute), absolute);
    }
}
