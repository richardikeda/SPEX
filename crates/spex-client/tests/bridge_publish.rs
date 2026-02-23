use axum::{
    extract::{Path, State},
    routing::put,
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use spex_client::{
    create_identity, publish_via_bridge,
};
use spex_core::types::{Envelope, Ctap2Cbor};
use spex_transport::inbox::{BridgeClient, BridgePublishRequest};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

#[derive(Clone)]
struct MockState {
    received_requests: Arc<Mutex<Vec<(String, BridgePublishRequest)>>>,
}

async fn handle_put_inbox(
    State(state): State<MockState>,
    Path(inbox_key): Path<String>,
    Json(payload): Json<BridgePublishRequest>,
) -> axum::http::StatusCode {
    state
        .received_requests
        .lock()
        .unwrap()
        .push((inbox_key, payload));
    axum::http::StatusCode::OK
}

#[tokio::test]
async fn test_publish_via_bridge_success() {
    // 1. Setup Mock Server
    let state = MockState {
        received_requests: Arc::new(Mutex::new(Vec::new())),
    };
    let app = Router::new()
        .route("/inbox/:key", put(handle_put_inbox))
        .with_state(state.clone());

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // 2. Setup Client
    let base_url = format!("http://{}", addr);
    let bridge = BridgeClient::new(base_url);
    let identity = create_identity();
    let recipient_user_id = spex_client::random_bytes(32); // Mock recipient

    // Create a dummy envelope
    let envelope = Envelope {
        thread_id: spex_client::random_bytes(32),
        epoch: 1,
        seq: 1,
        sender_user_id: hex::decode(&identity.user_id_hex).unwrap(),
        ciphertext: b"encrypted".to_vec(),
        signature: None,
        extensions: Default::default(),
    };

    // 3. Publish
    let result = publish_via_bridge(
        &identity,
        &recipient_user_id,
        &envelope,
        &bridge,
        Some(3600),
    )
    .await;

    assert!(result.is_ok(), "publish failed: {:?}", result.err());

    // 4. Verify Request
    let requests = state.received_requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let (key, payload) = &requests[0];

    // Verify key derived correctly (SHA256 of recipient ID)
    let expected_key_bytes = spex_core::hash::hash_bytes(spex_core::hash::HashId::Sha256, &recipient_user_id);
    assert_eq!(*key, hex::encode(expected_key_bytes));

    // Verify payload fields
    assert!(payload.ttl_seconds == Some(3600));

    // Verify grant user_id matches sender
    let grant_user_bytes = BASE64.decode(&payload.grant.user_id).unwrap();
    assert_eq!(hex::encode(grant_user_bytes), identity.user_id_hex);

    // Verify puzzle recipient key matches our seed
    let puzzle_recipient = BASE64.decode(&payload.puzzle.recipient_key).unwrap();
    assert_eq!(puzzle_recipient, recipient_user_id);

    // Verify data matches envelope
    let envelope_bytes = envelope.to_ctap2_canonical_bytes().unwrap();
    let payload_bytes = BASE64.decode(&payload.data).unwrap();
    assert_eq!(payload_bytes, envelope_bytes);
}
