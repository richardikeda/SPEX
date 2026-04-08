// SPDX-License-Identifier: MPL-2.0
use thiserror::Error;

use spex_core::error::SpexError;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("libp2p error: {0}")]
    Libp2p(String),
    #[error("gossipsub publish error: {0}")]
    GossipPublish(String),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("cbor decode error: {0}")]
    CborDecode(#[from] SpexError),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("inbox bridge returned invalid payload")]
    BridgePayload,
    #[error("missing chunk for hash {0}")]
    MissingChunk(String),
    #[error("chunk hash mismatch for index {0}")]
    ChunkHashMismatch(usize),
    #[error("chunk index mismatch (expected {expected}, got {actual})")]
    ChunkIndexMismatch { expected: usize, actual: usize },
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),
    #[error("payload length mismatch (expected {expected}, got {actual})")]
    PayloadLengthMismatch { expected: usize, actual: usize },
    #[error("invalid payload: {0}")]
    InvalidPayload(String),
    #[error("bridge rejected ttl policy")]
    InvalidTtl,
    #[error("grant signature invalid")]
    GrantInvalid,
    #[error("grant expired")]
    GrantExpired,
    #[error("pow parameters below minimum")]
    PowTooWeak,
    #[error("pow puzzle invalid")]
    PowInvalid,
}

impl From<libp2p::gossipsub::PublishError> for TransportError {
    /// Converts a gossipsub publish error into the transport error variant.
    fn from(value: libp2p::gossipsub::PublishError) -> Self {
        Self::GossipPublish(value.to_string())
    }
}
