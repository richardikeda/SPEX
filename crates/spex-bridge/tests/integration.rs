use axum::{body::Body, http::Request, http::StatusCode};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rusqlite::{params, Connection};
use serde_json::json;
use spex_bridge::{app, init_state_with_clock, Clock};
use spex_core::{hash, pow, pow::PowParams, sign, types::GrantToken};
use spex_core::hash::HashId;
use std::sync::Arc;
use tempfile::tempdir;
use tower::ServiceExt;

struct FixedClock {
    now: u64,
}

impl Clock for FixedClock {
    /// Returns the fixed timestamp for deterministic tests.
    fn now(&self) -> u64 {
        self.now
    }
}

/// Creates a deterministic Ed25519 signing key for tests.
fn test_signing_key() -> ed25519_dalek::SigningKey {
    let seed = [5u8; 32];
    sign::ed25519_signing_key_from_seed(&seed).expect("seed should be 32 bytes")
}

/// Builds a signed grant payload for bridge storage requests.
fn build_grant_payload(expires_at: u64) -> serde_json::Value {
    let signing_key = test_signing_key();
    let verify_key = sign::ed25519_verify_key(&signing_key);
    let grant = GrantToken {
        user_id: b"user".to_vec(),
        role: 1,
        flags: None,
        expires_at: Some(expires_at),
        extensions: Default::default(),
    };
    let hash =
        hash::hash_ctap2_cbor_value(HashId::Sha256, &grant).expect("grant hash");
    let signature = sign::ed25519_sign_hash(&signing_key, &hash);
    json!({
        "user_id": BASE64.encode(&grant.user_id),
        "role": grant.role,
        "flags": grant.flags,
        "expires_at": grant.expires_at,
        "verifying_key": BASE64.encode(verify_key.to_bytes()),
        "signature": BASE64.encode(signature.to_bytes())
    })
}

/// Builds a valid storage request payload with PoW and grant data.
fn build_payload(now: u64, data: &[u8]) -> serde_json::Value {
    let recipient_key = b"recipient-key";
    let puzzle_input = b"puzzle-input";
    let params = PowParams::default();
    let puzzle_output = pow::generate_puzzle_output(recipient_key, puzzle_input, params)
        .expect("puzzle output");

    json!({
        "data": BASE64.encode(data),
        "grant": build_grant_payload(now + 60),
        "puzzle": {
            "recipient_key": BASE64.encode(recipient_key),
            "puzzle_input": BASE64.encode(puzzle_input),
            "puzzle_output": BASE64.encode(puzzle_output),
            "params": {
                "memory_kib": params.memory_kib,
                "iterations": params.iterations,
                "parallelism": params.parallelism,
                "output_len": params.output_len
            }
        }
    })
}

/// Builds a storage request payload with custom PoW inputs and parameters.
fn build_payload_with_puzzle(
    now: u64,
    data: &[u8],
    recipient_key: &[u8],
    puzzle_input: &[u8],
    puzzle_output: &[u8],
    params: PowParams,
) -> serde_json::Value {
    json!({
        "data": BASE64.encode(data),
        "grant": build_grant_payload(now + 60),
        "puzzle": {
            "recipient_key": BASE64.encode(recipient_key),
            "puzzle_input": BASE64.encode(puzzle_input),
            "puzzle_output": BASE64.encode(puzzle_output),
            "params": {
                "memory_kib": params.memory_kib,
                "iterations": params.iterations,
                "parallelism": params.parallelism,
                "output_len": params.output_len
            }
        }
    })
}

/// Inserts inbox keys and items directly into the bridge database for testing.
fn seed_inbox(db_path: &std::path::Path, inbox_key: &str, items: &[&[u8]]) {
    seed_inbox_with_expiry(
        db_path,
        inbox_key,
        &items
            .iter()
            .map(|item| InboxSeedItem {
                payload: *item,
                expires_at: None,
            })
            .collect::<Vec<_>>(),
    );
}

/// Inserts inbox keys and items directly into the bridge database for testing.
fn seed_inbox_with_expiry(
    db_path: &std::path::Path,
    inbox_key: &str,
    items: &[InboxSeedItem<'_>],
) {
    let conn = Connection::open(db_path).expect("open inbox db");
    conn.execute(
        "INSERT OR IGNORE INTO inbox_keys (inbox_key) VALUES (?1)",
        params![inbox_key],
    )
    .expect("insert inbox key");
    for item in items {
        conn.execute(
            "INSERT INTO inbox_items (inbox_key, item, expires_at) VALUES (?1, ?2, ?3)",
            params![inbox_key, item.payload, item.expires_at],
        )
        .expect("insert inbox item");
    }
}

/// Carries inbox payload data and optional expiration timestamps for tests.
struct InboxSeedItem<'a> {
    payload: &'a [u8],
    expires_at: Option<u64>,
}

/// Loads the most recent request log outcome for assertions.
fn load_latest_request_outcome(db_path: &std::path::Path) -> String {
    let conn = Connection::open(db_path).expect("open request log db");
    conn.query_row(
        "SELECT outcome FROM request_logs ORDER BY id DESC LIMIT 1",
        [],
        |row| row.get::<_, String>(0),
    )
    .expect("load request outcome")
}

/// Verifies storing and retrieving a card succeeds with valid grant/puzzle data.
#[tokio::test]
async fn put_get_card_roundtrip() {
    let tmp = tempdir().expect("tempdir");
    let clock = Arc::new(FixedClock { now: 1_700_000_000 });
    let state = init_state_with_clock(tmp.path().join("bridge.db"), clock).unwrap();
    let app = app(state);

    let data = b"card-bytes";
    let card_hash = hex::encode(hash::hash_bytes(hash::HashId::Sha256, data));
    let payload = build_payload(1_700_000_000, data);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/cards/{card_hash}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/cards/{card_hash}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(response_json["data"], BASE64.encode(data));
}

/// Verifies storing and retrieving a slot succeeds with valid grant/puzzle data.
#[tokio::test]
async fn put_get_slot_roundtrip() {
    let tmp = tempdir().expect("tempdir");
    let clock = Arc::new(FixedClock { now: 1_700_000_000 });
    let state = init_state_with_clock(tmp.path().join("bridge.db"), clock).unwrap();
    let app = app(state);

    let data = b"slot-bytes";
    let slot_hash = hex::encode(hash::hash_bytes(HashId::Sha256, data));
    let payload = build_payload(1_700_000_000, data);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/slot/{slot_hash}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/slot/{slot_hash}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(response_json["data"], BASE64.encode(data));
}

/// Ensures slot uploads are rejected when the slot_id hash does not match the payload.
#[tokio::test]
async fn rejects_slot_hash_mismatch() {
    let tmp = tempdir().expect("tempdir");
    let clock = Arc::new(FixedClock { now: 1_700_000_000 });
    let state = init_state_with_clock(tmp.path().join("bridge.db"), clock).unwrap();
    let app = app(state);

    let data = b"slot-bytes";
    let slot_hash = hex::encode(hash::hash_bytes(HashId::Sha256, b"other-slot"));
    let payload = build_payload(1_700_000_000, data);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/slot/{slot_hash}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/slot/{slot_hash}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Ensures expired grants are rejected by the slot endpoint.
#[tokio::test]
async fn rejects_expired_grant() {
    let tmp = tempdir().expect("tempdir");
    let clock = Arc::new(FixedClock { now: 1_700_000_000 });
    let state = init_state_with_clock(tmp.path().join("bridge.db"), clock).unwrap();
    let app = app(state);

    let data = b"slot";
    let slot_hash = hex::encode(hash::hash_bytes(HashId::Sha256, data));
    let payload = json!({
        "data": BASE64.encode(data),
        "grant": build_grant_payload(1_699_999_999),
        "puzzle": {
            "recipient_key": BASE64.encode(b"recipient"),
            "puzzle_input": BASE64.encode(b"input"),
            "puzzle_output": BASE64.encode(
                pow::generate_puzzle_output(b"recipient", b"input", PowParams::default())
                    .unwrap()
            )
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/slot/{slot_hash}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// Ensures invalid grant tokens are rejected by the slot endpoint.
#[tokio::test]
async fn rejects_invalid_grant() {
    let tmp = tempdir().expect("tempdir");
    let clock = Arc::new(FixedClock { now: 1_700_000_000 });
    let state = init_state_with_clock(tmp.path().join("bridge.db"), clock).unwrap();
    let app = app(state);

    let slot_hash = hex::encode(hash::hash_bytes(HashId::Sha256, b"slot"));
    let mut payload = build_payload(1_700_000_000, b"slot");
    payload["grant"]["user_id"] = json!("not-base64");

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/slot/{slot_hash}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// Ensures invalid grant signatures are rejected by the slot endpoint and logged.
#[tokio::test]
async fn rejects_invalid_grant_signature() {
    let tmp = tempdir().expect("tempdir");
    let db_path = tmp.path().join("bridge.db");
    let clock = Arc::new(FixedClock { now: 1_700_000_000 });
    let state = init_state_with_clock(db_path.clone(), clock).unwrap();
    let app = app(state);

    let slot_hash = hex::encode(hash::hash_bytes(HashId::Sha256, b"slot"));
    let mut payload = build_payload(1_700_000_000, b"slot");
    payload["grant"]["signature"] = json!(BASE64.encode([9u8; 64]));

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/slot/{slot_hash}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(load_latest_request_outcome(&db_path), "rejected");
}

/// Ensures invalid puzzle output is rejected by the slot endpoint.
#[tokio::test]
async fn rejects_invalid_puzzle() {
    let tmp = tempdir().expect("tempdir");
    let clock = Arc::new(FixedClock { now: 1_700_000_000 });
    let state = init_state_with_clock(tmp.path().join("bridge.db"), clock).unwrap();
    let app = app(state);

    let slot_hash = hex::encode(hash::hash_bytes(HashId::Sha256, b"slot"));
    let payload = json!({
        "data": BASE64.encode(b"slot"),
        "grant": build_grant_payload(1_700_000_100),
        "puzzle": {
            "recipient_key": BASE64.encode(b"recipient"),
            "puzzle_input": BASE64.encode(b"input"),
            "puzzle_output": BASE64.encode(b"wrong")
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/slot/{slot_hash}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// Ensures weak PoW parameters are rejected when stronger validation is requested.
#[tokio::test]
async fn rejects_weak_pow() {
    let tmp = tempdir().expect("tempdir");
    let clock = Arc::new(FixedClock { now: 1_700_000_000 });
    let state = init_state_with_clock(tmp.path().join("bridge.db"), clock).unwrap();
    let app = app(state);

    let recipient_key = b"recipient";
    let puzzle_input = b"input";
    let weak_params = PowParams {
        memory_kib: 8 * 1024,
        iterations: 1,
        parallelism: 1,
        output_len: 32,
    };
    let puzzle_output =
        pow::generate_puzzle_output(recipient_key, puzzle_input, weak_params).unwrap();
    let payload = build_payload_with_puzzle(
        1_700_000_000,
        b"slot",
        recipient_key,
        puzzle_input,
        &puzzle_output,
        weak_params,
    );

    let slot_hash = hex::encode(hash::hash_bytes(HashId::Sha256, b"slot"));
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/slot/{slot_hash}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// Ensures puzzles derived from a different recipient key are rejected.
#[tokio::test]
async fn rejects_incorrect_pow_salt() {
    let tmp = tempdir().expect("tempdir");
    let clock = Arc::new(FixedClock { now: 1_700_000_000 });
    let state = init_state_with_clock(tmp.path().join("bridge.db"), clock).unwrap();
    let app = app(state);

    let puzzle_input = b"input";
    let puzzle_output =
        pow::generate_puzzle_output(b"correct-recipient", puzzle_input, PowParams::default())
            .unwrap();
    let payload = build_payload_with_puzzle(
        1_700_000_000,
        b"slot",
        b"incorrect-recipient",
        puzzle_input,
        &puzzle_output,
        PowParams::default(),
    );

    let slot_hash = hex::encode(hash::hash_bytes(HashId::Sha256, b"slot"));
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/slot/{slot_hash}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// Verifies inbox retrieval returns stored items for a known inbox key.
#[tokio::test]
async fn get_inbox_with_items() {
    let tmp = tempdir().expect("tempdir");
    let db_path = tmp.path().join("bridge.db");
    let clock = Arc::new(FixedClock { now: 1_700_000_000 });
    let state = init_state_with_clock(db_path.clone(), clock).unwrap();
    let app = app(state);

    let inbox_key = "deadbeef";
    seed_inbox(&db_path, inbox_key, &[b"item-1", b"item-2"]);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/inbox/{inbox_key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        response_json["items"],
        json!([BASE64.encode(b"item-1"), BASE64.encode(b"item-2")])
    );
    assert!(response_json["next_cursor"].is_null());
}

/// Verifies inbox retrieval returns an empty list when no items are present.
#[tokio::test]
async fn get_inbox_without_items() {
    let tmp = tempdir().expect("tempdir");
    let db_path = tmp.path().join("bridge.db");
    let clock = Arc::new(FixedClock { now: 1_700_000_000 });
    let state = init_state_with_clock(db_path.clone(), clock).unwrap();
    let app = app(state);

    let inbox_key = "empty-box";
    seed_inbox(&db_path, inbox_key, &[]);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/inbox/{inbox_key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(response_json["items"], json!([]));
    assert!(response_json["next_cursor"].is_null());
}

/// Verifies inbox retrieval skips expired items for a known inbox key.
#[tokio::test]
async fn get_inbox_filters_expired_items() {
    let tmp = tempdir().expect("tempdir");
    let db_path = tmp.path().join("bridge.db");
    let clock = Arc::new(FixedClock { now: 1_700_000_000 });
    let state = init_state_with_clock(db_path.clone(), clock).unwrap();
    let app = app(state);

    let inbox_key = "expired-box";
    seed_inbox_with_expiry(
        &db_path,
        inbox_key,
        &[
            InboxSeedItem {
                payload: b"expired-item",
                expires_at: Some(1_699_999_000),
            },
            InboxSeedItem {
                payload: b"fresh-item",
                expires_at: Some(1_700_000_100),
            },
        ],
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/inbox/{inbox_key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(response_json["items"], json!([BASE64.encode(b"fresh-item")]));
    assert!(response_json["next_cursor"].is_null());
}

/// Captures a manual TLS validation checklist for the bridge server.
///
/// Checklist:
/// - Start the bridge with HTTPS and a known certificate.
/// - Confirm the client rejects invalid, expired, or self-signed certificates.
/// - Verify the TLS handshake negotiates the expected protocol and cipher suite.
#[test]
#[ignore]
fn tls_validation_checklist() {
    assert!(true, "Run bridge with TLS and verify certificate trust.");
}
