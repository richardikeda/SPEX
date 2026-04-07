use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use spex_core::{
    hash::{hash_ctap2_cbor_value, HashId},
    pow::{self, PowParams},
    sign,
    types::GrantToken,
};
use spex_transport::{
    ingest_validation_correlation_id, validate_p2p_grant_payload, validate_p2p_puzzle_payload,
    P2pGrantPayload, P2pPuzzlePayload, PowParamsPayload, TransportError,
};

/// Builds a deterministic signing key for P2P ingest validation tests.
fn test_signing_key() -> ed25519_dalek::SigningKey {
    let seed = [3u8; 32];
    sign::ed25519_signing_key_from_seed(&seed).expect("seed should be 32 bytes")
}

/// Builds a signed grant payload for P2P ingestion validation.
fn build_grant_payload(expires_at: Option<u64>) -> P2pGrantPayload {
    let signing_key = test_signing_key();
    let verifying_key = sign::ed25519_verify_key(&signing_key);
    let grant = GrantToken {
        user_id: b"user".to_vec(),
        role: 1,
        flags: None,
        expires_at,
        extensions: Default::default(),
    };
    let hash = hash_ctap2_cbor_value(HashId::Sha256, &grant).expect("grant hash");
    let signature = sign::ed25519_sign_hash(&signing_key, &hash);
    P2pGrantPayload {
        user_id: BASE64_STANDARD.encode(&grant.user_id),
        role: grant.role,
        flags: grant.flags,
        expires_at: grant.expires_at,
        verifying_key: BASE64_STANDARD.encode(verifying_key.to_bytes()),
        signature: BASE64_STANDARD.encode(signature.to_bytes()),
    }
}

/// Builds a P2P puzzle payload with the provided PoW parameters.
fn build_puzzle_payload(params: PowParams) -> P2pPuzzlePayload {
    let recipient_key = b"recipient";
    let puzzle_input = b"puzzle-input";
    let puzzle_output =
        pow::generate_puzzle_output(recipient_key, puzzle_input, params).expect("puzzle output");
    P2pPuzzlePayload {
        recipient_key: BASE64_STANDARD.encode(recipient_key),
        puzzle_input: BASE64_STANDARD.encode(puzzle_input),
        puzzle_output: BASE64_STANDARD.encode(puzzle_output),
        params: Some(PowParamsPayload {
            memory_kib: params.memory_kib,
            iterations: params.iterations,
            parallelism: params.parallelism,
            output_len: params.output_len,
        }),
    }
}

/// Ensures invalid grant signatures are rejected during P2P ingestion validation.
#[test]
fn rejects_invalid_grant_signature_in_p2p_ingest() {
    let mut payload = build_grant_payload(Some(1_700_000_050));
    payload.signature = BASE64_STANDARD.encode([9u8; 64]);
    let result = validate_p2p_grant_payload(1_700_000_000, &payload);
    assert!(matches!(result, Err(TransportError::GrantInvalid)));
}

/// Ensures expired grants are rejected during P2P ingestion validation.
#[test]
fn rejects_expired_grant_in_p2p_ingest() {
    let payload = build_grant_payload(Some(1_699_999_999));
    let result = validate_p2p_grant_payload(1_700_000_000, &payload);
    assert!(matches!(result, Err(TransportError::GrantExpired)));
}

/// Ensures weak PoW parameters are rejected during P2P ingestion validation.
#[test]
fn rejects_weak_pow_in_p2p_ingest() {
    let weak_params = PowParams {
        memory_kib: 8 * 1024,
        iterations: 1,
        parallelism: 1,
        output_len: 32,
    };
    let payload = build_puzzle_payload(weak_params);
    let result = validate_p2p_puzzle_payload(&payload);
    assert!(matches!(result, Err(TransportError::PowTooWeak)));
}

/// Ensures malformed base64 payloads return explicit deterministic invalid-payload errors.
#[test]
fn rejects_malformed_base64_payload_with_explicit_error() {
    let payload = P2pGrantPayload {
        user_id: "@@not-base64@@".to_string(),
        role: 1,
        flags: None,
        expires_at: None,
        verifying_key: "@@not-base64@@".to_string(),
        signature: "@@not-base64@@".to_string(),
    };
    let result = validate_p2p_grant_payload(1_700_000_000, &payload);
    assert!(matches!(result, Err(TransportError::InvalidPayload(_))));
}

/// Ensures ingest correlation fallback is deterministic when contextual bytes are unavailable.
#[test]
fn ingest_correlation_fallback_is_deterministic() {
    let missing = ingest_validation_correlation_id(None);
    let empty = ingest_validation_correlation_id(Some(&[]));
    let with_hint = ingest_validation_correlation_id(Some(b"grant"));

    assert_eq!(missing, empty);
    assert_ne!(missing, with_hint);
}
