// SPDX-License-Identifier: MPL-2.0
use std::time::{Duration, Instant};

use libp2p::{identity::Keypair, PeerId};
use spex_transport::chunking::{chunk_data, ChunkingConfig};
use spex_transport::transport::ChunkDescriptor;
use spex_transport::{
    derive_operation_correlation, Chunk, ChunkManifest, P2pNodeConfig, P2pTransport,
    PeerReputationState, TransportConfig,
};

/// Builds a chunk manifest from chunk data and payload length.
fn build_manifest_from_chunks(chunks: &[Chunk], total_len: usize) -> ChunkManifest {
    ChunkManifest {
        chunks: chunks
            .iter()
            .map(|chunk| ChunkDescriptor {
                index: chunk.index,
                hash: chunk.hash.clone(),
            })
            .collect(),
        total_len,
    }
}

/// Drives a node until it connects to at least one peer or the timeout elapses.
async fn drive_until_connected(node: &mut P2pTransport, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while node.connected_peer_count() == 0 && Instant::now() < deadline {
        node.drive_for(Duration::from_millis(200)).await;
    }
}

/// Drives a P2P node for a fixed duration to service network requests.
async fn drive_node_for(mut node: P2pTransport, duration: Duration) {
    node.drive_for(duration).await;
}

/// Publishes a manifest for an inbox key and verifies the peer recovers the payload.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_nodes_publish_and_recover_manifest_delivery() {
    let payload = b"manifest delivery over libp2p";
    let transport_config = TransportConfig {
        chunking: ChunkingConfig {
            chunk_size: 12,
            ..ChunkingConfig::default()
        },
        ..TransportConfig::default()
    };
    let chunks = chunk_data(&transport_config.chunking, payload);
    let manifest = build_manifest_from_chunks(&chunks, payload.len());

    let node_a_config = P2pNodeConfig {
        listen_addrs: vec!["/ip4/127.0.0.1/tcp/0".parse().expect("listen addr")],
        publish_wait: Duration::from_secs(6),
        ..P2pNodeConfig::default()
    };
    let mut node_a = P2pTransport::new(
        Keypair::generate_ed25519(),
        transport_config.clone(),
        node_a_config,
    )
    .await
    .expect("node A");
    let node_a_addr = node_a
        .listen_addrs()
        .first()
        .expect("node A address")
        .clone();
    let node_a_peer_addr = node_a.local_peer_multiaddr(&node_a_addr);

    let node_b_config = P2pNodeConfig {
        listen_addrs: vec!["/ip4/127.0.0.1/tcp/0".parse().expect("listen addr")],
        bootstrap_nodes: vec![node_a_peer_addr],
        query_timeout: Duration::from_secs(8),
        manifest_wait: Duration::from_secs(8),
        ..P2pNodeConfig::default()
    };
    let mut node_b = P2pTransport::new(
        Keypair::generate_ed25519(),
        transport_config.clone(),
        node_b_config,
    )
    .await
    .expect("node B");
    let node_b_addr = node_b
        .listen_addrs()
        .first()
        .expect("node B address")
        .clone();
    let node_b_peer_addr = node_b.local_peer_multiaddr(&node_b_addr);
    node_a.dial_peer(node_b_peer_addr).expect("dial node B");

    drive_until_connected(&mut node_a, Duration::from_secs(4)).await;
    drive_until_connected(&mut node_b, Duration::from_secs(4)).await;

    let inbox_key = b"recipient-inbox-key".to_vec();
    let mut recovery_node = node_b;
    let recovery_key = inbox_key.clone();
    let recovery_task = tokio::spawn(async move {
        recovery_node
            .recover_payloads_for_inbox(&recovery_key, Duration::from_secs(8))
            .await
    });
    tokio::time::sleep(Duration::from_millis(200)).await;
    node_a
        .publish_to_inboxes(std::slice::from_ref(&inbox_key), &manifest, &chunks)
        .await
        .expect("publish to inbox");

    let drive_task = tokio::spawn(drive_node_for(node_a, Duration::from_secs(10)));
    let recovered = recovery_task
        .await
        .expect("recovery task")
        .expect("recover");
    drive_task.await.expect("drive task");

    assert!(!recovered.is_empty());
    assert_eq!(recovered[0], payload);
}

/// Verifies recurring invalid payload behavior deterministically escalates to temporary ban.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recurring_invalid_payload_escalates_to_ban() {
    let mut node = P2pTransport::new(
        Keypair::generate_ed25519(),
        TransportConfig::default(),
        P2pNodeConfig {
            invalid_payload_ban_threshold: 2,
            ..P2pNodeConfig::default()
        },
    )
    .await
    .expect("node");

    let peer = PeerId::random();
    node.report_invalid_payload(peer);
    let first = node.peer_reputation_snapshot(peer);
    assert_eq!(first.state, PeerReputationState::Neutral);

    node.report_invalid_payload(peer);
    let second = node.peer_reputation_snapshot(peer);
    assert_eq!(second.invalid_payload_penalties, 2);
    assert_eq!(second.state, PeerReputationState::Banned);
    assert!(node.is_peer_banned(&peer));
}

/// Verifies publish observability falls back deterministically when inbox context is missing.
#[test]
fn publish_correlation_fallback_is_deterministic_without_inbox_context() {
    let fallback = derive_operation_correlation("publish", None);
    let empty = derive_operation_correlation("publish", Some(&[]));
    let with_context = derive_operation_correlation("publish", Some(b"inbox-key"));

    assert!(fallback.used_minimal_context);
    assert!(empty.used_minimal_context);
    assert!(!with_context.used_minimal_context);
    assert_eq!(fallback.correlation_id, empty.correlation_id);
    assert_ne!(fallback.correlation_id, with_context.correlation_id);
}
