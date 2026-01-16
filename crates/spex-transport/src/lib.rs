#![forbid(unsafe_code)]

//! SPEX transport layer built on libp2p.
//!
//! This crate wires together chunking, Kademlia publication/replication, gossip,
//! random-walk peer discovery, and inbox scanning with an HTTP bridge fallback.

pub mod chunking;
pub mod error;
pub mod inbox;
pub mod transport;

pub use chunking::{chunk_data, reassemble_chunks, Chunk, ChunkingConfig};
pub use error::TransportError;
pub use inbox::{
    resolve_inbox_with_fallback, BridgeClient, InboxScanRequest, InboxScanResponse,
};
pub use transport::{
    publish_payload, random_walk, replicate_manifest, BuildTransport, ChunkManifest,
    TransportComponents, TransportConfig,
};
