use crate::error::SpexError;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey, SECRET_KEY_LENGTH};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug)]
pub enum HashId {
    Sha256 = 1,
    #[cfg(feature = "blake3_hash")]
    Blake3 = 2,
}

pub fn hash_bytes(hash_id: HashId, data: &[u8]) -> Vec<u8> {
    match hash_id {
        HashId::Sha256 => Sha256::digest(data).to_vec(),
        #[cfg(feature = "blake3_hash")]
        HashId::Blake3 => blake3::hash(data).as_bytes().to_vec(),
    }
}

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

pub fn ed25519_verify_hash(verify: &VerifyingKey, msg_hash: &[u8], sig: &Signature) -> Result<(), SpexError> {
    verify.verify_strict(msg_hash, sig).map_err(|_| SpexError::SigVerifyFailed)
}
