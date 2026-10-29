mod routes;
mod store;

use store::Store;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let store = Store::from_env();
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(7331);
    eprintln!("gl-server: data={} port={port}", store.root.display());
    let app = routes::router(store);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
