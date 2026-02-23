use ed25519_dalek::{Signature, SigningKey};
use rand_core::OsRng;
use spex_core::{
    hash::{hash_bytes, HashId},
    sign::{ed25519_sign_hash, ed25519_verify_hash, ed25519_verify_key},
};

#[test]
fn test_hashing_empty_input() {
    let input = b"";
    let hash = hash_bytes(HashId::Sha256, input);
    assert_eq!(hash.len(), 32);
    // Verified against `echo -n "" | sha256sum`
    assert_eq!(
        hex::encode(&hash),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn test_hashing_large_input() {
    let input = vec![0u8; 1024 * 1024]; // 1MB of zeros
    let hash = hash_bytes(HashId::Sha256, &input);
    assert_eq!(hash.len(), 32);
}

#[test]
fn test_signature_verification_failure() {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = ed25519_verify_key(&signing_key);

    let message = b"hello world";
    let hash = hash_bytes(HashId::Sha256, message);
    let signature = ed25519_sign_hash(&signing_key, &hash);

    // Verify correct signature
    assert!(ed25519_verify_hash(&verifying_key, &hash, &signature).is_ok());

    // Verify failure with wrong message
    let wrong_message = b"hello universe";
    let wrong_hash = hash_bytes(HashId::Sha256, wrong_message);
    assert!(ed25519_verify_hash(&verifying_key, &wrong_hash, &signature).is_err());

    // Verify failure with wrong key
    let other_signing_key = SigningKey::generate(&mut csprng);
    let other_verifying_key = ed25519_verify_key(&other_signing_key);
    assert!(ed25519_verify_hash(&other_verifying_key, &hash, &signature).is_err());
}

#[test]
fn test_malformed_signature_bytes() {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = ed25519_verify_key(&signing_key);
    let message = b"test";
    let hash = hash_bytes(HashId::Sha256, message);

    let signature = ed25519_sign_hash(&signing_key, &hash);
    let mut sig_bytes = signature.to_bytes();
    sig_bytes[0] ^= 0xFF; // Flip bits

    let bad_signature = Signature::from_bytes(&sig_bytes);
    assert!(ed25519_verify_hash(&verifying_key, &hash, &bad_signature).is_err());
}
