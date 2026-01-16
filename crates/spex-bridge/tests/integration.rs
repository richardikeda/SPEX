use axum::{body::Body, http::Request, http::StatusCode};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::json;
use spex_bridge::{app, init_state_with_clock, Clock};
use spex_core::{hash, pow, pow::PowParams};
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

/// Builds a valid storage request payload with PoW and grant data.
fn build_payload(now: u64, data: &[u8]) -> serde_json::Value {
    let recipient_key = b"recipient-key";
    let puzzle_input = b"puzzle-input";
    let params = PowParams::default();
    let puzzle_output = pow::generate_puzzle_output(recipient_key, puzzle_input, params)
        .expect("puzzle output");

    json!({
        "data": BASE64.encode(data),
        "grant": {
            "user_id": BASE64.encode(b"user"),
            "role": 1,
            "flags": null,
            "expires_at": now + 60
        },
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
    let payload = build_payload(1_700_000_000, data);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/slot/slot-123")
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
                .uri("/slot/slot-123")
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

/// Ensures expired grants are rejected by the slot endpoint.
#[tokio::test]
async fn rejects_expired_grant() {
    let tmp = tempdir().expect("tempdir");
    let clock = Arc::new(FixedClock { now: 1_700_000_000 });
    let state = init_state_with_clock(tmp.path().join("bridge.db"), clock).unwrap();
    let app = app(state);

    let data = b"slot";
    let payload = json!({
        "data": BASE64.encode(data),
        "grant": {
            "user_id": BASE64.encode(b"user"),
            "role": 1,
            "flags": null,
            "expires_at": 1_699_999_999
        },
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
                .uri("/slot/slot-1")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// Ensures invalid puzzle output is rejected by the slot endpoint.
#[tokio::test]
async fn rejects_invalid_puzzle() {
    let tmp = tempdir().expect("tempdir");
    let clock = Arc::new(FixedClock { now: 1_700_000_000 });
    let state = init_state_with_clock(tmp.path().join("bridge.db"), clock).unwrap();
    let app = app(state);

    let payload = json!({
        "data": BASE64.encode(b"slot"),
        "grant": {
            "user_id": BASE64.encode(b"user"),
            "role": 1,
            "flags": null,
            "expires_at": 1_700_000_100
        },
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
                .uri("/slot/slot-2")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
