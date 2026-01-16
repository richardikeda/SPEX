use serde::{Deserialize, Serialize};

use spex_core::hash::{hash_bytes, HashId};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chunk {
    pub index: usize,
    pub hash: Vec<u8>,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct ChunkingConfig {
    pub chunk_size: usize,
    pub hash_id: HashId,
}

impl Default for ChunkingConfig {
    /// Returns the default chunking configuration used by the transport layer.
    fn default() -> Self {
        Self {
            chunk_size: 32 * 1024,
            hash_id: HashId::Sha256,
        }
    }
}

/// Splits data into hash-addressed chunks using the provided configuration.
pub fn chunk_data(config: &ChunkingConfig, data: &[u8]) -> Vec<Chunk> {
    if data.is_empty() {
        return Vec::new();
    }

    data.chunks(config.chunk_size)
        .enumerate()
        .map(|(index, chunk)| Chunk {
            index,
            hash: hash_bytes(config.hash_id, chunk),
            data: chunk.to_vec(),
        })
        .collect()
}

/// Reassembles chunks into the original byte payload based on chunk indices.
pub fn reassemble_chunks(chunks: &[Chunk]) -> Vec<u8> {
    let mut sorted = chunks.to_vec();
    sorted.sort_by_key(|chunk| chunk.index);
    let total_len: usize = sorted.iter().map(|chunk| chunk.data.len()).sum();
    let mut data = Vec::with_capacity(total_len);
    for chunk in sorted {
        data.extend_from_slice(&chunk.data);
    }
    data
}
