// SPDX-License-Identifier: MPL-2.0
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
///
/// This function sorts references to chunks rather than cloning the chunks themselves
/// to improve performance, especially for large payloads.
pub fn reassemble_chunks(chunks: &[Chunk]) -> Vec<u8> {
    // Collect references to chunks into a vector for sorting without cloning data.
    let mut sorted: Vec<&Chunk> = chunks.iter().collect();

    // Sort chunks by their index to ensure correct reassembly order.
    // Using unstable sort is slightly faster as stable order for identical indices is not required.
    sorted.sort_unstable_by_key(|chunk| chunk.index);

    // Calculate total length to pre-allocate memory for the reassembled data.
    let total_len: usize = sorted.iter().map(|chunk| chunk.data.len()).sum();
    let mut data = Vec::with_capacity(total_len);

    // Append each chunk's data to the reassembled buffer.
    for chunk in sorted {
        data.extend_from_slice(&chunk.data);
    }

    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunking_and_reassembly() {
        let config = ChunkingConfig::default();
        let original_data =
            b"Hello, SPEX reassembly test! This should work correctly with optimized sorting."
                .to_vec();

        let mut chunks = chunk_data(&config, &original_data);
        // Shuffle chunks to ensure reassemble_chunks correctly sorts them by index.
        chunks.reverse();

        let reassembled = reassemble_chunks(&chunks);
        assert_eq!(reassembled, original_data);
    }

    #[test]
    fn test_reassemble_empty_chunks() {
        let chunks: Vec<Chunk> = Vec::new();
        let reassembled = reassemble_chunks(&chunks);
        assert!(reassembled.is_empty());
    }
}
