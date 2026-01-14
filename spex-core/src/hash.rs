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
