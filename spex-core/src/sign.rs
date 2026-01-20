use crate::{
    cbor,
    error::SpexError,
    hash::{hash_bytes, HashId},
    types::Ctap2Cbor,
};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey, SECRET_KEY_LENGTH};
use serde::Serialize;

/// Create an Ed25519 SigningKey from a 32-byte seed (test vectors use this).
pub fn ed25519_signing_key_from_seed(seed32: &[u8]) -> Result<SigningKey, SpexError> {
    if seed32.len() != SECRET_KEY_LENGTH {
        return Err(SpexError::InvalidLength("ed25519 seed must be 32 bytes"));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(seed32);
    Ok(SigningKey::from_bytes(&arr))
}

pub fn ed25519_verify_key(signing: &SigningKey) -> VerifyingKey {
    signing.verifying_key()
}

pub fn ed25519_sign_hash(signing: &SigningKey, msg_hash: &[u8]) -> Signature {
    // dalek signs arbitrary bytes; we sign the hash bytes per spec.
    signing.sign(msg_hash)
}

pub fn ed25519_verify_hash(
    verify: &VerifyingKey,
    msg_hash: &[u8],
    sig: &Signature,
) -> Result<(), SpexError> {
    verify
        .verify_strict(msg_hash, sig)
        .map_err(|_| SpexError::SigVerifyFailed)
}

/// Hash and sign a CBOR-serializable payload using CTAP2 canonical CBOR encoding.
pub fn ed25519_sign_ctap2_cbor<T: Serialize>(
    signing: &SigningKey,
    hash_id: HashId,
    value: &T,
) -> Result<Signature, SpexError> {
    let cbor = cbor::to_ctap2_canonical_bytes(value)?;
    let digest = hash_bytes(hash_id, &cbor);
    Ok(ed25519_sign_hash(signing, &digest))
}

/// Hash and verify a CBOR-serializable payload using CTAP2 canonical CBOR encoding.
pub fn ed25519_verify_ctap2_cbor<T: Serialize>(
    verify: &VerifyingKey,
    hash_id: HashId,
    value: &T,
    sig: &Signature,
) -> Result<(), SpexError> {
    let cbor = cbor::to_ctap2_canonical_bytes(value)?;
    let digest = hash_bytes(hash_id, &cbor);
    ed25519_verify_hash(verify, &digest, sig)
}

/// Hash and sign a SPEX CBOR structure using its CTAP2 canonical encoding.
pub fn ed25519_sign_ctap2_cbor_value(
    signing: &SigningKey,
    hash_id: HashId,
    value: &impl Ctap2Cbor,
) -> Result<Signature, SpexError> {
    let cbor = value.to_ctap2_canonical_bytes()?;
    let digest = hash_bytes(hash_id, &cbor);
    Ok(ed25519_sign_hash(signing, &digest))
}

/// Hash and verify a SPEX CBOR structure using its CTAP2 canonical encoding.
pub fn ed25519_verify_ctap2_cbor_value(
    verify: &VerifyingKey,
    hash_id: HashId,
    value: &impl Ctap2Cbor,
    sig: &Signature,
) -> Result<(), SpexError> {
    let cbor = value.to_ctap2_canonical_bytes()?;
    let digest = hash_bytes(hash_id, &cbor);
    ed25519_verify_hash(verify, &digest, sig)
}
