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
    collect_manifest_chunks, parse_manifest_from_gossip, passive_replicate_chunks,
    publish_payload, random_walk, random_walk_with_key, reconstruct_envelope, renew_chunk_ttl,
    replicate_manifest, start_manifest_chunk_queries, BuildTransport, ChunkManifest,
    TransportComponents, TransportConfig,
};
