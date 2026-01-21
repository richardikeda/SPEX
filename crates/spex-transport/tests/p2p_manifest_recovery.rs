use std::time::Duration;

use libp2p::identity::Keypair;
use spex_transport::chunking::{chunk_data, ChunkingConfig};
use spex_transport::transport::ChunkDescriptor;
use spex_transport::{Chunk, ChunkManifest, P2pNodeConfig, P2pTransport, TransportConfig};

/// Builds a chunk manifest from a set of chunks and payload length.
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

/// Publishes a manifest on one node and recovers the payload on another node via libp2p.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_nodes_recover_payload_via_manifest() {
    let payload = b"manifest recovery over libp2p";
    let transport_config = TransportConfig {
        chunking: ChunkingConfig {
            chunk_size: 8,
            ..ChunkingConfig::default()
        },
        ..TransportConfig::default()
    };
    let chunks = chunk_data(&transport_config.chunking, payload);
    let manifest = build_manifest_from_chunks(&chunks, payload.len());

    let node_a_config = P2pNodeConfig {
        listen_addrs: vec!["/ip4/127.0.0.1/tcp/0".parse().expect("listen addr")],
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
        manifest_wait: Duration::from_secs(6),
        ..P2pNodeConfig::default()
    };
    let mut node_b =
        P2pTransport::new(Keypair::generate_ed25519(), transport_config, node_b_config)
            .await
            .expect("node B");
    let node_b_addr = node_b
        .listen_addrs()
        .first()
        .expect("node B address")
        .clone();
    let node_b_peer_addr = node_b.local_peer_multiaddr(&node_b_addr);
    node_a.dial_peer(node_b_peer_addr).expect("dial node B");

    let inbox_key = b"recipient";
    let recover_task = tokio::spawn(async move {
        node_b
            .recover_payloads_for_inbox(inbox_key, Duration::from_secs(6))
            .await
    });

    tokio::time::sleep(Duration::from_secs(2)).await;
    node_a
        .publish_to_inboxes(&[inbox_key.to_vec()], &manifest, &chunks)
        .await
        .expect("publish");

    let recovered = recover_task.await.expect("task").expect("recover");
    assert!(!recovered.is_empty());
    assert_eq!(recovered[0], payload);
}
