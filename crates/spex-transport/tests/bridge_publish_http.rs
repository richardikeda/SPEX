// SPDX-License-Identifier: MPL-2.0
use axum::{extract::Path, routing::put, Json, Router};
use serde_json::json;
use spex_transport::{
    inbox::{build_bridge_publish_request, BridgeClient, BridgeEnvelopePublishInput},
    TransportError,
};

/// Returns bridge-like validation errors based on the inbox key under test.
async fn reject_for_key(
    Path(inbox_key): Path<String>,
) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    if inbox_key == hex::encode(b"grant") {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "grant signature invalid" })),
        );
    }
    if inbox_key == hex::encode(b"pow") {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "puzzle validation failed" })),
        );
    }
    (
        axum::http::StatusCode::BAD_REQUEST,
        Json(json!({ "error": "invalid request: invalid inbox ttl" })),
    )
}

/// Ensures bridge error payloads are mapped to explicit transport errors.
#[tokio::test]
async fn maps_bridge_validation_errors() {
    let app = Router::new().route("/inbox/{key}", put(reject_for_key));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    let client = BridgeClient::new(format!("http://{addr}"));
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[1u8; 32]);
    let request = build_bridge_publish_request(
        &signing_key,
        &BridgeEnvelopePublishInput {
            sender_user_id: vec![5u8; 32],
            recipient_key_seed: vec![6u8; 32],
            envelope: vec![1, 2, 3],
            role: 1,
            flags: None,
            now_unix: 10,
            grant_ttl_seconds: 20,
            inbox_ttl_seconds: Some(30),
            pow_params: spex_core::pow::PowParams::default(),
        },
    )
    .expect("request");

    let grant_err = client
        .publish_to_inbox(b"grant", &request)
        .await
        .expect_err("grant");
    assert!(matches!(grant_err, TransportError::GrantInvalid));

    let pow_err = client
        .publish_to_inbox(b"pow", &request)
        .await
        .expect_err("pow");
    assert!(matches!(pow_err, TransportError::PowInvalid));

    let ttl_err = client
        .publish_to_inbox(b"ttl", &request)
        .await
        .expect_err("ttl");
    assert!(matches!(ttl_err, TransportError::InvalidTtl));
}
