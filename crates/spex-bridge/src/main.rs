use spex_bridge::init_state;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

/// Boots the bridge HTTP server with Axum and Tokio.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let db_path = std::env::var("SPEX_BRIDGE_DB").unwrap_or_else(|_| "spex-bridge.db".to_string());
    let state = init_state(db_path)?;
    let app = spex_bridge::app(state);

    let addr: SocketAddr = "0.0.0.0:3000".parse()?;
    tracing::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
