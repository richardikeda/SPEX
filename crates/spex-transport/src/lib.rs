#![forbid(unsafe_code)]

//! SPEX transport layer built on libp2p.
//!
//! This crate wires together chunking, Kademlia publication/replication, gossip,
//! random-walk peer discovery, and inbox scanning with an HTTP bridge fallback.

pub mod chunking;
pub mod error;
pub mod ingest;
pub mod inbox;
pub mod p2p;
pub mod transport;

pub use chunking::{chunk_data, reassemble_chunks, Chunk, ChunkingConfig};
pub use error::TransportError;
pub use ingest::{
    validate_p2p_grant_payload, validate_p2p_puzzle_payload, P2pGrantPayload, P2pPuzzlePayload,
    PowParamsPayload,
};
pub use inbox::{resolve_inbox_with_fallback, BridgeClient, InboxScanRequest, InboxScanResponse};
pub use p2p::{inbox_gossip_topic, P2pNodeConfig, P2pTransport};
pub use transport::manifest_payload;
pub use transport::{
    collect_manifest_chunks, parse_manifest_from_gossip, passive_replicate_chunks, publish_payload,
    random_walk, random_walk_with_key, reassemble_chunks_with_manifest,
    reassemble_payload_from_store, reconstruct_envelope, recover_chunks_from_store,
    recover_manifest_from_gossip, renew_chunk_ttl, replicate_manifest,
    robust_random_walk_with_seed, start_manifest_chunk_queries, BuildTransport, ChunkManifest,
    TransportComponents, TransportConfig,
};
