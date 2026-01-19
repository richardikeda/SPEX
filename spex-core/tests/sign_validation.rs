use ed25519_dalek::Signature;
use spex_core::{hash, sign, types};
use spex_core::hash::HashId;

/// Build a deterministic Ed25519 signing key for tests.
fn test_signing_key() -> ed25519_dalek::SigningKey {
    let seed = [7u8; 32];
    sign::ed25519_signing_key_from_seed(&seed).expect("seed should be 32 bytes")
}

/// Sign and verify a ContactCard, then confirm validation fails after mutation.
#[test]
fn contact_card_signature_validation() {
    let signing_key = test_signing_key();
    let verify_key = sign::ed25519_verify_key(&signing_key);

    let mut card = types::ContactCard {
        user_id: vec![1, 2, 3, 4],
        verifying_key: verify_key.to_bytes().to_vec(),
        device_id: vec![9, 9, 9],
        device_nonce: vec![7, 7, 7],
        issued_at: 1_700_000_000,
        invite: None,
        signature: None,
        extensions: Default::default(),
    };

    let hash = hash::hash_ctap2_cbor_value(HashId::Sha256, &card)
        .expect("hashing ContactCard should succeed");
    let signature = sign::ed25519_sign_hash(&signing_key, &hash);
    let signature_bytes = signature.to_bytes();
    let signature = Signature::from_bytes(&signature_bytes);

    sign::ed25519_verify_hash(&verify_key, &hash, &signature)
        .expect("ContactCard signature should verify");

    card.issued_at = 1_700_000_001;
    let mutated_hash = hash::hash_ctap2_cbor_value(HashId::Sha256, &card)
        .expect("hashing mutated ContactCard should succeed");
    let validation = sign::ed25519_verify_hash(&verify_key, &mutated_hash, &signature);
    assert!(validation.is_err(), "mutated ContactCard should fail validation");
}

/// Sign and verify a GrantToken, then confirm validation fails after mutation.
#[test]
fn grant_token_signature_validation() {
    let signing_key = test_signing_key();
    let verify_key = sign::ed25519_verify_key(&signing_key);

    let mut grant = types::GrantToken {
        user_id: vec![8, 8, 8, 8],
        role: 4,
        flags: Some(2),
        expires_at: Some(1_700_001_234),
        extensions: Default::default(),
    };

    let hash = hash::hash_ctap2_cbor_value(HashId::Sha256, &grant)
        .expect("hashing GrantToken should succeed");
    let signature = sign::ed25519_sign_hash(&signing_key, &hash);
    let signature_bytes = signature.to_bytes();
    let signature = Signature::from_bytes(&signature_bytes);

    sign::ed25519_verify_hash(&verify_key, &hash, &signature)
        .expect("GrantToken signature should verify");

    grant.role = 5;
    let mutated_hash = hash::hash_ctap2_cbor_value(HashId::Sha256, &grant)
        .expect("hashing mutated GrantToken should succeed");
    let validation = sign::ed25519_verify_hash(&verify_key, &mutated_hash, &signature);
    assert!(validation.is_err(), "mutated GrantToken should fail validation");
}
