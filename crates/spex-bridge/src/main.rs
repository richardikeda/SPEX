// SPDX-License-Identifier: MPL-2.0
use spex_bridge::init_state;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

const DEFAULT_BRIDGE_ADDR: &str = "127.0.0.1:3000";

/// Parses an optional bridge bind address and defaults to a loopback-only listener.
fn bridge_addr_from_value(value: Option<String>) -> Result<SocketAddr, std::net::AddrParseError> {
    value
        .unwrap_or_else(|| DEFAULT_BRIDGE_ADDR.to_string())
        .parse()
}

/// Boots the bridge HTTP server with Axum and Tokio.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let db_path = std::env::var("SPEX_BRIDGE_DB").unwrap_or_else(|_| "spex-bridge.db".to_string());
    let state = init_state(db_path).map_err(|err| format!("{err}"))?;
    let app = spex_bridge::app(state);

    let addr = bridge_addr_from_value(std::env::var("SPEX_BRIDGE_ADDR").ok())?;
    tracing::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uses the loopback address when no explicit bridge bind address is configured.
    #[test]
    fn bridge_addr_defaults_to_loopback() {
        assert_eq!(
            bridge_addr_from_value(None).expect("default address"),
            "127.0.0.1:3000".parse().expect("expected address")
        );
    }

    /// Accepts an explicit operator-selected bridge bind address.
    #[test]
    fn bridge_addr_accepts_explicit_override() {
        assert_eq!(
            bridge_addr_from_value(Some("127.0.0.1:4100".to_string())).expect("override address"),
            "127.0.0.1:4100".parse().expect("expected address")
        );
    }

    /// Rejects malformed bridge bind addresses instead of silently using another value.
    #[test]
    fn bridge_addr_rejects_malformed_override() {
        assert!(bridge_addr_from_value(Some("not-an-address".to_string())).is_err());
    }
}
