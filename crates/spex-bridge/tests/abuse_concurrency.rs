use axum::{body::Body, http::Request, http::StatusCode};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rusqlite::{params, Connection};
use serde_json::json;
use spex_bridge::{app, init_state_with_clock, Clock};
use spex_core::{
    hash::{self, HashId},
    pow,
    pow::PowParams,
    sign,
    types::GrantToken,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::task::JoinSet;
use tower::ServiceExt;

struct FixedClock {
    now: u64,
}

impl Clock for FixedClock {
    /// Returns a fixed timestamp so concurrent tests remain deterministic.
    fn now(&self) -> u64 {
        self.now
    }
}

/// Stores deterministic identity material used to sign grants in tests.
struct TestIdentity {
    user_id: Vec<u8>,
    signing_key: ed25519_dalek::SigningKey,
}

impl TestIdentity {
    /// Creates a deterministic identity from a seed byte.
    fn from_seed(seed: u8) -> Self {
        let signing_key = sign::ed25519_signing_key_from_seed(&[seed; 32]).expect("seed");
        let user_id = hash::hash_bytes(HashId::Sha256, &[seed; 16]);
        Self {
            user_id,
            signing_key,
        }
    }

    /// Returns the hex identity key used by bridge rate-limiting tables.
    fn identity_hex(&self) -> String {
        hex::encode(&self.user_id)
    }
}

/// Builds a signed grant payload for the provided identity.
fn build_grant_payload(identity: &TestIdentity, expires_at: u64) -> serde_json::Value {
    let verify_key = sign::ed25519_verify_key(&identity.signing_key);
    let grant = GrantToken {
        user_id: identity.user_id.clone(),
        role: 1,
        flags: None,
        expires_at: Some(expires_at),
        extensions: BTreeMap::new(),
    };
    let grant_hash = hash::hash_ctap2_cbor_value(HashId::Sha256, &grant).expect("grant hash");
    let signature = sign::ed25519_sign_hash(&identity.signing_key, &grant_hash);
    json!({
        "user_id": BASE64.encode(&grant.user_id),
        "role": grant.role,
        "flags": grant.flags,
        "expires_at": grant.expires_at,
        "verifying_key": BASE64.encode(verify_key.to_bytes()),
        "signature": BASE64.encode(signature.to_bytes())
    })
}

/// Builds a PUT /inbox payload with custom PoW parameters and puzzle values.
fn build_inbox_payload(
    identity: &TestIdentity,
    now: u64,
    data: &[u8],
    params: PowParams,
    recipient_key: &[u8],
    puzzle_input: &[u8],
) -> serde_json::Value {
    let puzzle_output =
        pow::generate_puzzle_output(recipient_key, puzzle_input, params).expect("puzzle output");
    json!({
        "data": BASE64.encode(data),
        "ttl_seconds": 60,
        "grant": build_grant_payload(identity, now + 120),
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

/// Builds a PUT /slot payload with custom PoW parameters and puzzle values.
fn build_slot_payload(
    identity: &TestIdentity,
    now: u64,
    data: &[u8],
    params: PowParams,
    recipient_key: &[u8],
    puzzle_input: &[u8],
) -> serde_json::Value {
    let puzzle_output =
        pow::generate_puzzle_output(recipient_key, puzzle_input, params).expect("puzzle output");
    json!({
        "data": BASE64.encode(data),
        "grant": build_grant_payload(identity, now + 120),
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

/// Sends a slot request and returns the resulting HTTP status.
async fn send_slot_request(
    app: axum::Router,
    identity: &TestIdentity,
    now: u64,
    data: Vec<u8>,
    params: PowParams,
) -> StatusCode {
    let slot_id = hex::encode(hash::hash_bytes(HashId::Sha256, &data));
    let payload = build_slot_payload(
        identity,
        now,
        &data,
        params,
        b"slot-recipient",
        b"slot-puzzle-input",
    );
    app.oneshot(
        Request::builder()
            .method("PUT")
            .uri(format!("/slot/{slot_id}"))
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("slot request"),
    )
    .await
    .expect("slot response")
    .status()
}

/// Sends an inbox request and returns the resulting HTTP status.
async fn send_inbox_request(
    app: axum::Router,
    identity: &TestIdentity,
    now: u64,
    inbox_key: &str,
    data: Vec<u8>,
    params: PowParams,
) -> StatusCode {
    let payload = build_inbox_payload(
        identity,
        now,
        &data,
        params,
        b"inbox-recipient",
        b"inbox-puzzle-input",
    );
    app.oneshot(
        Request::builder()
            .method("PUT")
            .uri(format!("/inbox/{inbox_key}"))
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("inbox request"),
    )
    .await
    .expect("inbox response")
    .status()
}

/// Counts request log outcomes for an identity.
fn load_request_outcome_counts(db_path: &std::path::Path, identity_hex: &str) -> (u64, u64) {
    let conn = Connection::open(db_path).expect("open db");
    let accepted: u64 = conn
        .query_row(
            "SELECT COUNT(*) FROM request_logs WHERE identity = ?1 AND outcome = 'accepted'",
            params![identity_hex],
            |row| row.get::<_, i64>(0),
        )
        .expect("accepted count") as u64;
    let rejected: u64 = conn
        .query_row(
            "SELECT COUNT(*) FROM request_logs WHERE identity = ?1 AND outcome = 'rejected'",
            params![identity_hex],
            |row| row.get::<_, i64>(0),
        )
        .expect("rejected count") as u64;
    (accepted, rejected)
}

/// Loads current identity reputation score from SQLite.
fn load_reputation_score(db_path: &std::path::Path, identity_hex: &str) -> i32 {
    let conn = Connection::open(db_path).expect("open db");
    conn.query_row(
        "SELECT score FROM identity_reputation WHERE identity = ?1",
        params![identity_hex],
        |row| row.get::<_, i32>(0),
    )
    .expect("reputation score")
}

/// Inserts synthetic accepted request logs to force higher required PoW for one identity.
fn seed_rate_limit_pressure(
    db_path: &std::path::Path,
    identity_hex: &str,
    now: u64,
    rows: usize,
    bytes_per_row: usize,
) {
    let conn = Connection::open(db_path).expect("open db");
    for index in 0..rows {
        conn.execute(
            "INSERT INTO request_logs (timestamp, identity, ip, slot_id, bytes, request_kind, outcome) \
             VALUES (?1, ?2, 'unknown', ?3, ?4, 'slot', 'accepted')",
            params![now as i64, identity_hex, format!("seeded-{index}"), bytes_per_row as i64],
        )
        .expect("seed request log");
    }
}

/// Exercises concurrent mixed endpoint abuse for one identity and verifies adaptive PoW + rejection.
#[tokio::test]
async fn concurrent_same_identity_escalates_pow_and_rejects_stale_params() {
    let tmp = tempdir().expect("tempdir");
    let db_path = tmp.path().join("bridge.db");
    let now = 1_700_000_000;
    let clock = Arc::new(FixedClock { now });
    let state = init_state_with_clock(db_path.clone(), clock).expect("state");
    let app = app(state);

    let identity = Arc::new(TestIdentity::from_seed(11));
    let base_params = PowParams::default();

    let initial_status = send_inbox_request(
        app.clone(),
        identity.as_ref(),
        now,
        "initial-accept",
        b"seed-request".to_vec(),
        base_params,
    )
    .await;
    assert_eq!(initial_status, StatusCode::NO_CONTENT);

    let mut tasks = JoinSet::new();
    for index in 0..10usize {
        let app_clone = app.clone();
        let identity_clone = Arc::clone(&identity);
        tasks.spawn(async move {
            if index % 2 == 0 {
                send_inbox_request(
                    app_clone,
                    identity_clone.as_ref(),
                    now,
                    &format!("abuse-{index}"),
                    vec![b'i'; 512],
                    base_params,
                )
                .await
            } else {
                send_slot_request(
                    app_clone,
                    identity_clone.as_ref(),
                    now,
                    vec![b's'; 256 + index],
                    base_params,
                )
                .await
            }
        });
    }

    let mut accepted = 0u64;
    while let Some(result) = tasks.join_next().await {
        let status = result.expect("task result");
        if status == StatusCode::NO_CONTENT {
            accepted += 1;
        }
    }
    assert!(accepted >= 8, "accepted={accepted}");

    seed_rate_limit_pressure(&db_path, &identity.identity_hex(), now, 24, 262_144);

    let stale_pow_status = send_slot_request(
        app.clone(),
        identity.as_ref(),
        now,
        b"stale-pow-after-burst".to_vec(),
        base_params,
    )
    .await;
    assert!(
        stale_pow_status == StatusCode::UNAUTHORIZED
            || stale_pow_status == StatusCode::TOO_MANY_REQUESTS,
        "status={stale_pow_status}"
    );

    let identity_hex = identity.identity_hex();
    let (logged_accepted, logged_rejected) = load_request_outcome_counts(&db_path, &identity_hex);
    assert!(
        logged_accepted > accepted,
        "logged_accepted={logged_accepted}"
    );
    assert!(logged_rejected >= 1, "logged_rejected={logged_rejected}");

    let reputation_score = load_reputation_score(&db_path, &identity_hex);
    assert!((-20..=100).contains(&reputation_score));
}

/// Verifies per-identity isolation under concurrent load with alternating payload sizes.
#[tokio::test]
async fn concurrent_identities_keep_pow_and_reputation_isolated() {
    let tmp = tempdir().expect("tempdir");
    let db_path = tmp.path().join("bridge.db");
    let now = 1_700_000_000;
    let clock = Arc::new(FixedClock { now });
    let state = init_state_with_clock(db_path.clone(), clock).expect("state");
    let app = app(state);

    let identity_a = Arc::new(TestIdentity::from_seed(21));
    let identity_b = Arc::new(TestIdentity::from_seed(22));
    let base_params = PowParams::default();

    let mut ramp_tasks = JoinSet::new();
    for index in 0..10usize {
        let app_clone = app.clone();
        let identity_clone = Arc::clone(&identity_a);
        ramp_tasks.spawn(async move {
            if index % 2 == 0 {
                send_inbox_request(
                    app_clone,
                    identity_clone.as_ref(),
                    now,
                    &format!("iso-a-{index}"),
                    vec![b'a'; 1_024 + index],
                    base_params,
                )
                .await
            } else {
                send_slot_request(
                    app_clone,
                    identity_clone.as_ref(),
                    now,
                    vec![b'b'; 4_096 + index],
                    base_params,
                )
                .await
            }
        });
    }
    while let Some(result) = ramp_tasks.join_next().await {
        result.expect("identity A task");
    }

    seed_rate_limit_pressure(&db_path, &identity_a.identity_hex(), now, 24, 262_144);

    let identity_a_stale_pow = send_inbox_request(
        app.clone(),
        identity_a.as_ref(),
        now,
        "iso-a-stale-pow",
        vec![b'a'; 2048],
        base_params,
    )
    .await;
    assert!(
        identity_a_stale_pow == StatusCode::UNAUTHORIZED
            || identity_a_stale_pow == StatusCode::TOO_MANY_REQUESTS
    );

    let identity_b_default_pow = send_slot_request(
        app.clone(),
        identity_b.as_ref(),
        now,
        b"identity-b-default-pow".to_vec(),
        base_params,
    )
    .await;
    assert_eq!(identity_b_default_pow, StatusCode::NO_CONTENT);

    let (a_accepted, a_rejected) =
        load_request_outcome_counts(&db_path, &identity_a.identity_hex());
    let (b_accepted, b_rejected) =
        load_request_outcome_counts(&db_path, &identity_b.identity_hex());
    assert!(a_accepted >= 8, "a_accepted={a_accepted}");
    assert!(a_rejected >= 1, "a_rejected={a_rejected}");
    assert_eq!(b_accepted, 1);
    assert_eq!(b_rejected, 0);

    let reputation_a = load_reputation_score(&db_path, &identity_a.identity_hex());
    let reputation_b = load_reputation_score(&db_path, &identity_b.identity_hex());
    assert_ne!(reputation_a, 0);
    assert_eq!(reputation_b, 1);
}
