use std::collections::BTreeMap;
use std::path::PathBuf;

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::store::Store;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
}

pub fn router(store: Store) -> Router {
    let state = AppState { store };
    Router::new()
        .route("/api/health", get(health))
        .route("/api/repos", get(list_repos))
        .route("/api/repos/{name}/refs", get(get_refs))
        .route("/api/repos/{name}/log", get(get_log))
        .route("/api/repos/{name}/commits/{id}", get(get_commit))
        .route(
            "/api/repos/{name}/objects/{hash}",
            get(get_object).post(put_object),
        )
        .route("/api/repos/{name}/push", post(push))
        .route("/", get(index))
        .route("/index.html", get(index))
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(state)
}

fn err(code: StatusCode, msg: &str) -> Response {
    (code, Json(serde_json::json!({"error": msg}))).into_response()
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok"}))
}

async fn list_repos(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.store.list_repos())
}

async fn get_refs(State(s): State<AppState>, Path(name): Path<String>) -> Response {
    if !Store::valid_name(&name) {
        return err(StatusCode::BAD_REQUEST, "invalid repo name");
    }
    Json(s.store.read_refs(&name)).into_response()
}

#[derive(Deserialize)]
struct LogQuery {
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    n: Option<usize>,
}

async fn get_log(
    State(s): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<LogQuery>,
) -> Response {
    if !Store::valid_name(&name) {
        return err(StatusCode::BAD_REQUEST, "invalid repo name");
    }
    let n = q.n.unwrap_or(50).clamp(1, 500);
    Json(s.store.log(&name, q.branch.as_deref(), n)).into_response()
}

async fn get_commit(
    State(s): State<AppState>,
    Path((name, id)): Path<(String, String)>,
) -> Response {
    if !Store::valid_name(&name) || !Store::valid_id(&id) {
        return err(StatusCode::BAD_REQUEST, "invalid name or id");
    }
    match s.store.read_commit(&name, &id) {
        Some(v) => Json(v).into_response(),
        None => err(StatusCode::NOT_FOUND, "commit not found"),
    }
}

async fn get_object(
    State(s): State<AppState>,
    Path((name, hash)): Path<(String, String)>,
) -> Response {
    if !Store::valid_name(&name) || !Store::valid_id(&hash) {
        return err(StatusCode::BAD_REQUEST, "invalid name or hash");
    }
    match s.store.read_object(&name, &hash) {
        Some(data) => ([(header::CONTENT_TYPE, "application/octet-stream")], data).into_response(),
        None => err(StatusCode::NOT_FOUND, "object not found"),
    }
}

async fn put_object(
    State(s): State<AppState>,
    Path((name, hash)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    if !Store::valid_name(&name) || !Store::valid_id(&hash) {
        return err(StatusCode::BAD_REQUEST, "invalid name or hash");
    }
    match s.store.store_object(&name, &hash, &body) {
        Ok(stored) => Json(serde_json::json!({"ok": true, "stored": stored})).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn push(
    State(s): State<AppState>,
    Path(name): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    if !Store::valid_name(&name) {
        return err(StatusCode::BAD_REQUEST, "invalid repo name");
    }
    // Accept server-native {refs, commits} and gl-client {branch, id, commit[, commits]}.
    let mut refs = BTreeMap::<String, String>::new();
    let mut commits: Vec<serde_json::Value> = Vec::new();
    if let Some(o) = body.get("refs").and_then(|x| x.as_object()) {
        for (b, v) in o {
            if let Some(id) = v.as_str() {
                refs.insert(b.clone(), id.to_string());
            }
        }
    }
    if let Some(arr) = body.get("commits").and_then(|x| x.as_array()) {
        commits.extend(arr.iter().cloned());
    }
    if let (Some(b), Some(id)) = (
        body.get("branch").and_then(|x| x.as_str()),
        body.get("id").and_then(|x| x.as_str()),
    ) {
        refs.insert(b.to_string(), id.to_string());
    }
    if let Some(c) = body.get("commit") {
        commits.push(c.clone());
    }
    if refs.is_empty() {
        return err(StatusCode::BAD_REQUEST, "push missing refs");
    }
    for v in &commits {
        match v.get("id").and_then(|i| i.as_str()) {
            Some(id) if Store::valid_id(id) => {}
            _ => return err(StatusCode::BAD_REQUEST, "commit missing valid id"),
        }
    }
    for (b, c) in &refs {
        if !Store::valid_id(b) || !Store::valid_id(c) {
            return err(StatusCode::BAD_REQUEST, "invalid ref");
        }
    }
    for v in &commits {
        let id = v.get("id").and_then(|i| i.as_str()).unwrap();
        if let Err(e) = s.store.store_commit(&name, id, v) {
            return err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string());
        }
    }
    let mut current = s.store.read_refs(&name);
    for (b, c) in &refs {
        match current.get(b) {
            None => {}
            Some(old) if *old == *c => continue,
            Some(old) => {
                if !s.store.is_ancestor(&name, old, c) {
                    return (
                        StatusCode::CONFLICT,
                        Json(serde_json::json!({
                            "error": "non-fast-forward",
                            "branch": b,
                            "have": old,
                            "want": c,
                        })),
                    )
                        .into_response();
                }
            }
        }
    }
    for (b, c) in &refs {
        current.insert(b.clone(), c.clone());
    }
    if let Err(e) = s.store.write_refs(&name, &current) {
        return err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string());
    }
    Json(serde_json::json!({"ok": true, "refs": current})).into_response()
}

async fn index() -> impl IntoResponse {
    let html = index_html().unwrap_or_else(|| "<!doctype html><title>gl-server</title><h1>gl-server</h1>".to_string());
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html)
}

fn index_html() -> Option<String> {
    for c in candidates() {
        if let Ok(s) = std::fs::read_to_string(&c) {
            return Some(s);
        }
    }
    None
}

fn candidates() -> Vec<PathBuf> {
    let mut v = vec![
        PathBuf::from("web/index.html"),
        PathBuf::from("../web/index.html"),
        PathBuf::from("../../web/index.html"),
    ];
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..5 {
            match dir {
                Some(d) => {
                    v.push(d.join("web/index.html"));
                    dir = d.parent().map(|p| p.to_path_buf());
                }
                None => break,
            }
        }
    }
    v
}
