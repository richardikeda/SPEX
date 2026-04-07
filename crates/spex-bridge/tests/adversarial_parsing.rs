use proptest::prelude::*;
use spex_bridge::{parse_inbox_store_request_bytes, parse_storage_request_bytes};

/// Returns a deterministic baseline-valid payload used for adversarial parser mutations.
fn valid_payload_json() -> String {
    serde_json::json!({
        "data": "AQI=",
        "grant": {
            "user_id": "AQI=",
            "role": 1,
            "flags": null,
            "expires_at": null,
            "verifying_key": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            "signature": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=="
        },
        "puzzle": {
            "recipient_key": "AQI=",
            "puzzle_input": "AQI=",
            "puzzle_output": "AQI=",
            "params": {
                "memory_kib": 65536,
                "iterations": 3,
                "parallelism": 1,
                "output_len": 32
            }
        },
        "ttl_seconds": 60
    })
    .to_string()
}

/// Ensures truncation on JSON boundaries is rejected with explicit parser errors.
#[test]
fn rejects_truncated_payloads() {
    let payload = valid_payload_json();
    let truncated = &payload.as_bytes()[..payload.len() / 2];
    assert!(parse_storage_request_bytes(truncated).is_err());
    assert!(parse_inbox_store_request_bytes(truncated).is_err());
}

/// Ensures unexpected JSON types are rejected instead of being silently coerced.
#[test]
fn rejects_unexpected_json_types() {
    let payload = serde_json::json!({
        "data": 17,
        "grant": "not-an-object",
        "puzzle": [],
        "ttl_seconds": "sixty"
    });
    let encoded = serde_json::to_vec(&payload).expect("json encoding should succeed");
    assert!(parse_storage_request_bytes(&encoded).is_err());
    assert!(parse_inbox_store_request_bytes(&encoded).is_err());
}

/// Ensures invalid base64 in required decoded fields is rejected by strict parsing logic.
#[test]
fn rejects_invalid_base64_required_field() {
    let mut payload: serde_json::Value =
        serde_json::from_str(&valid_payload_json()).expect("baseline payload should parse");
    payload["grant"]["user_id"] = serde_json::Value::String("***not-base64***".to_string());
    let encoded = serde_json::to_vec(&payload).expect("json encoding should succeed");
    assert!(parse_storage_request_bytes(&encoded).is_err());
    assert!(parse_inbox_store_request_bytes(&encoded).is_err());
}

/// Ensures invalid payload parsing remains deterministic for repeated decoding attempts.
#[test]
fn invalid_payload_error_is_deterministic() {
    let mut payload: serde_json::Value =
        serde_json::from_str(&valid_payload_json()).expect("baseline payload should parse");
    payload["puzzle"]["recipient_key"] = serde_json::Value::String("###invalid###".to_string());
    let encoded = serde_json::to_vec(&payload).expect("json encoding should succeed");

    let first = parse_storage_request_bytes(&encoded)
        .expect_err("invalid payload must fail")
        .to_string();
    let second = parse_storage_request_bytes(&encoded)
        .expect_err("invalid payload must fail")
        .to_string();
    assert_eq!(first, second);
}

proptest! {
    /// Ensures arbitrary untrusted bytes never panic bridge storage parser boundaries.
    #[test]
    fn storage_parser_never_panics_on_untrusted_bytes(input in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let _ = parse_storage_request_bytes(&input);
    }

    /// Ensures arbitrary untrusted bytes never panic bridge inbox parser boundaries.
    #[test]
    fn inbox_parser_never_panics_on_untrusted_bytes(input in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let _ = parse_inbox_store_request_bytes(&input);
    }
}
