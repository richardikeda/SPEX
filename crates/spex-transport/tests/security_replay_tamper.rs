use std::collections::HashMap;

use libp2p::{identity::Keypair, PeerId};
use spex_transport::transport::ChunkDescriptor;
use spex_transport::{
    chunking::{chunk_data, ChunkingConfig},
    reassemble_chunks_with_manifest, reassemble_payload_from_store, ChunkManifest, P2pNodeConfig,
    P2pTransport, PeerReputationState, TransportConfig, TransportError,
};

/// Builds a deterministic chunk manifest for a payload and chunk configuration.
fn build_manifest(
    payload: &[u8],
    chunking: &ChunkingConfig,
) -> (ChunkManifest, HashMap<Vec<u8>, Vec<u8>>) {
    let chunks = chunk_data(chunking, payload);
    let manifest = ChunkManifest {
        chunks: chunks
            .iter()
            .map(|chunk| ChunkDescriptor {
                index: chunk.index,
                hash: chunk.hash.clone(),
            })
            .collect(),
        total_len: payload.len(),
    };
    let store = chunks
        .iter()
        .map(|chunk| (chunk.hash.clone(), chunk.data.clone()))
        .collect();
    (manifest, store)
}

/// Ensures replaying the same material in different chunk orders keeps deterministic recovery.
#[test]
fn replayed_material_recovers_with_different_chunk_orders() {
    let payload: Vec<u8> = (0..(96 * 1024)).map(|value| (value % 251) as u8).collect();
    let config = TransportConfig::default();
    let (manifest, store) = build_manifest(&payload, &config.chunking);

    let mut chunks = spex_transport::recover_chunks_from_store(&manifest, &store, &config)
        .expect("chunks should recover");
    chunks.reverse();

    let recovered = reassemble_chunks_with_manifest(&manifest, &chunks, &config)
        .expect("reassembly must be deterministic");
    assert_eq!(recovered, payload);

    let recovered_from_store = reassemble_payload_from_store(&manifest, &store, &config)
        .expect("store recovery must work");
    assert_eq!(recovered_from_store, payload);
}

/// Ensures a tampered chunk payload is rejected by hash validation without panicking.
#[test]
fn tampered_chunk_hash_is_rejected_without_panic() {
    let payload = b"transport tamper check payload";
    let config = TransportConfig::default();
    let (manifest, mut store) = build_manifest(payload, &config.chunking);

    let first_hash = manifest.chunks[0].hash.clone();
    store.insert(first_hash, b"tampered".to_vec());

    let result =
        std::panic::catch_unwind(|| reassemble_payload_from_store(&manifest, &store, &config));
    assert!(result.is_ok(), "tamper handling must not panic");

    let err = result
        .expect("function should return result")
        .expect_err("tampered chunk must fail");
    assert!(matches!(err, TransportError::ChunkHashMismatch(_)));
}

/// Ensures manifests with partial references are rejected by invariant checks.
#[test]
fn manifest_with_partial_references_is_rejected() {
    let payload = b"manifest partial reference payload";
    let config = TransportConfig {
        chunking: ChunkingConfig {
            chunk_size: 8,
            ..ChunkingConfig::default()
        },
        ..TransportConfig::default()
    };
    let (manifest, store) = build_manifest(payload, &config.chunking);

    let partial_manifest = ChunkManifest {
        chunks: manifest.chunks[..manifest.chunks.len() - 1].to_vec(),
        total_len: manifest.total_len,
    };

    let err = reassemble_payload_from_store(&partial_manifest, &store, &config)
        .expect_err("partial manifest must fail");
    assert!(matches!(err, TransportError::PayloadLengthMismatch { .. }));
}

/// Ensures inconsistent responses escalate through probation before deterministic ban.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inconsistent_responses_escalate_probation_then_ban() {
    let mut node = P2pTransport::new(
        Keypair::generate_ed25519(),
        TransportConfig::default(),
        P2pNodeConfig {
            inconsistent_response_ban_threshold: 4,
            ..P2pNodeConfig::default()
        },
    )
    .await
    .expect("node");

    let peer = PeerId::random();
    node.report_inconsistent_response(peer);
    node.report_inconsistent_response(peer);

    let probation = node.peer_reputation_snapshot(peer);
    assert_eq!(probation.state, PeerReputationState::Probation);
    assert!(!node.is_peer_banned(&peer));

    node.report_inconsistent_response(peer);
    node.report_inconsistent_response(peer);

    let banned = node.peer_reputation_snapshot(peer);
    assert_eq!(banned.inconsistent_response_penalties, 4);
    assert_eq!(banned.state, PeerReputationState::Banned);
    assert!(node.is_peer_banned(&peer));
}
