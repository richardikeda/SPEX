// SPDX-License-Identifier: MPL-2.0
#![forbid(unsafe_code)]

//! SPEX transport layer built on libp2p.
//!
//! This crate wires together chunking, Kademlia publication/replication, gossip,
//! random-walk peer discovery, and inbox scanning with an HTTP bridge fallback.

pub mod chunking;
pub mod error;
pub mod inbox;
pub mod ingest;
pub mod p2p;
pub mod telemetry;
pub mod transport;

pub use chunking::{chunk_data, reassemble_chunks, Chunk, ChunkingConfig};
pub use error::TransportError;
pub use inbox::{resolve_inbox_with_fallback, BridgeClient, InboxScanRequest, InboxScanResponse};
pub use ingest::{
    ingest_validation_correlation_id, validate_p2p_grant_payload, validate_p2p_puzzle_payload,
    P2pGrantPayload, P2pPuzzlePayload, PowParamsPayload,
};
pub use p2p::{
    inbox_gossip_topic, P2pMetricsSnapshot, P2pNodeConfig, P2pRuntimeProfile, P2pTransport,
    PeerReputationSnapshot, PeerReputationState, SnapshotLoadState, SnapshotRecoveryStatus,
};
pub use telemetry::{
    derive_minimal_correlation_id, derive_operation_correlation, derive_operation_correlation_id,
    NetworkHealthIndicators, NetworkHealthStatus, NetworkHealthThresholds, OperationCorrelation,
};
pub use transport::manifest_payload;
pub use transport::{
    collect_manifest_chunks, decode_bootstrap_snapshot, encode_bootstrap_snapshot,
    parse_manifest_from_gossip, passive_replicate_chunks, publish_payload, random_walk,
    random_walk_with_key, read_bootstrap_snapshot, reassemble_chunks_with_manifest,
    reassemble_correlation_id, reassemble_payload_from_store, reconstruct_envelope,
    recover_chunks_from_store, recover_manifest_from_gossip, renew_chunk_ttl, replicate_manifest,
    robust_random_walk_with_seed, robust_random_walk_with_sources, start_manifest_chunk_queries,
    write_bootstrap_snapshot_atomic, BuildTransport, ChunkManifest, PersistedBootstrapState,
    PersistedPeer, PersistedPeerReputation, TransportComponents, TransportConfig,
};
