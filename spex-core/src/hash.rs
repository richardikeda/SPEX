use sha2::{Digest, Sha256};
use serde::Serialize;

use crate::{cbor, error::SpexError};

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

/// Hash a CBOR-serializable payload using CTAP2 canonical CBOR encoding.
pub fn hash_ctap2_cbor<T: Serialize>(hash_id: HashId, value: &T) -> Result<Vec<u8>, SpexError> {
    let cbor = cbor::to_ctap2_canonical_bytes(value)?;
    Ok(hash_bytes(hash_id, &cbor))
}
