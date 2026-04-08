// SPDX-License-Identifier: MPL-2.0
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{cbor, error::SpexError, types::Ctap2Cbor};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// Hash a SPEX CBOR structure using its CTAP2 canonical encoding.
pub fn hash_ctap2_cbor_value(
    hash_id: HashId,
    value: &impl Ctap2Cbor,
) -> Result<Vec<u8>, SpexError> {
    let cbor = value.to_ctap2_canonical_bytes()?;
    Ok(hash_bytes(hash_id, &cbor))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_vectors;
    use serde::Serialize;

    /// Test that SHA256 hashing of raw bytes matches known test vectors.
    #[test]
    fn test_hash_bytes_sha256() {
        let data = hex::decode(test_vectors::TV1_CONFIG_CBOR_HEX).expect("invalid TV hex");
        let expected = hex::decode(test_vectors::TV1_CFG_HASH_SHA256_HEX).expect("invalid TV hex");
        let got = hash_bytes(HashId::Sha256, &data);
        assert_eq!(got, expected, "SHA256 hash mismatch for TV1");

        let data2 = hex::decode(test_vectors::TV2_CARD_WO_SIG_CBOR_HEX).expect("invalid TV hex");
        let expected2 =
            hex::decode(test_vectors::TV2_CARD_HASH_SHA256_HEX).expect("invalid TV hex");
        let got2 = hash_bytes(HashId::Sha256, &data2);
        assert_eq!(got2, expected2, "SHA256 hash mismatch for TV2");
    }

    /// Test that Blake3 hashing produces deterministic output of the correct length.
    #[test]
    #[cfg(feature = "blake3_hash")]
    fn test_hash_bytes_blake3() {
        let data = b"SPEX blake3 test";
        let got = hash_bytes(HashId::Blake3, data);
        assert_eq!(got.len(), 32, "Blake3 hash should be 32 bytes");

        let got2 = hash_bytes(HashId::Blake3, data);
        assert_eq!(got, got2, "Blake3 hash should be deterministic");
    }

    /// Test hashing of a CBOR-serializable structure using CTAP2 canonicalization.
    #[test]
    fn test_hash_ctap2_cbor() {
        #[derive(Serialize)]
        struct Simple {
            z: u32,
            a: u32,
        }
        let val = Simple { z: 20, a: 10 };
        let res = hash_ctap2_cbor(HashId::Sha256, &val);
        assert!(res.is_ok());
        let hash = res.unwrap();
        assert_eq!(hash.len(), 32);
    }

    /// Test hashing of a SPEX type implementing Ctap2Cbor.
    #[test]
    fn test_hash_ctap2_cbor_value() {
        use crate::types::InviteToken;
        let invite = InviteToken {
            major: 1,
            minor: 2,
            requires_puzzle: true,
            extensions: Default::default(),
        };
        let res = hash_ctap2_cbor_value(HashId::Sha256, &invite);
        assert!(res.is_ok());
        let hash = res.unwrap();
        assert_eq!(hash.len(), 32);
    }
}
