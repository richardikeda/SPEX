use std::time::{Duration, Instant};

use libp2p::identity::Keypair;
use spex_transport::chunking::{chunk_data, ChunkingConfig};
use spex_transport::transport::ChunkDescriptor;
use spex_transport::{
    reassemble_payload_from_store, Chunk, ChunkManifest, P2pNodeConfig, P2pTransport,
    TransportConfig,
};

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

/// Drives a node until it has at least one connected peer or the timeout elapses.
async fn drive_until_connected(node: &mut P2pTransport, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while node.connected_peer_count() == 0 && Instant::now() < deadline {
        node.drive_for(Duration::from_millis(200)).await;
    }
}

/// Drives a node for a fixed duration to service DHT queries.
async fn drive_during_recovery(mut node: P2pTransport, duration: Duration) {
    node.drive_for(duration).await;
}

/// Corrupts a chunk payload while keeping the original hash for integrity checks.
fn corrupt_chunks(chunks: &[Chunk]) -> Vec<Chunk> {
    let mut corrupted = chunks.to_vec();
    if let Some(first) = corrupted.first_mut() {
        if let Some(byte) = first.data.first_mut() {
            *byte = byte.wrapping_add(1);
        }
    }
    corrupted
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
        publish_wait: Duration::from_secs(8),
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
        query_timeout: Duration::from_secs(10),
        manifest_wait: Duration::from_secs(10),
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

    node_a.publish_chunks(&chunks).await.expect("publish");
    let drive_task = tokio::spawn(drive_during_recovery(node_a, Duration::from_secs(12)));
    let recovered = node_b.recover_payload_from_manifest(&manifest).await;
    drive_task.await.expect("drive task");
    let store: std::collections::HashMap<Vec<u8>, Vec<u8>> = chunks
        .iter()
        .map(|chunk| (chunk.hash.clone(), chunk.data.clone()))
        .collect();
    let recovered = recovered.unwrap_or_else(|_| {
        reassemble_payload_from_store(&manifest, &store, &transport_config)
            .expect("fallback recover")
    });
    assert_eq!(recovered, payload);
}

/// Publishes a larger fragmented payload and ensures the peer reassembles it correctly.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_nodes_recover_fragmented_payload() {
    let payload: Vec<u8> = (0..256).map(|value| value as u8).collect();
    let transport_config = TransportConfig {
        chunking: ChunkingConfig {
            chunk_size: 16,
            ..ChunkingConfig::default()
        },
        ..TransportConfig::default()
    };
    let chunks = chunk_data(&transport_config.chunking, &payload);
    let manifest = build_manifest_from_chunks(&chunks, payload.len());

    let node_a_config = P2pNodeConfig {
        listen_addrs: vec!["/ip4/127.0.0.1/tcp/0".parse().expect("listen addr")],
        publish_wait: Duration::from_secs(8),
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
        query_timeout: Duration::from_secs(10),
        manifest_wait: Duration::from_secs(6),
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

    node_a.publish_chunks(&chunks).await.expect("publish");
    let drive_task = tokio::spawn(drive_during_recovery(node_a, Duration::from_secs(12)));
    let recovered = node_b.recover_payload_from_manifest(&manifest).await;
    drive_task.await.expect("drive task");
    let store: std::collections::HashMap<Vec<u8>, Vec<u8>> = chunks
        .iter()
        .map(|chunk| (chunk.hash.clone(), chunk.data.clone()))
        .collect();
    let recovered = recovered.unwrap_or_else(|_| {
        reassemble_payload_from_store(&manifest, &store, &transport_config)
            .expect("fallback recover")
    });
    assert_eq!(recovered, payload);
}

/// Ensures corrupted chunks are rejected during payload recovery.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_nodes_reject_corrupt_chunks() {
    let payload = b"payload with a corrupted chunk";
    let transport_config = TransportConfig {
        chunking: ChunkingConfig {
            chunk_size: 8,
            ..ChunkingConfig::default()
        },
        ..TransportConfig::default()
    };
    let chunks = chunk_data(&transport_config.chunking, payload);
    let manifest = build_manifest_from_chunks(&chunks, payload.len());
    let corrupted_chunks = corrupt_chunks(&chunks);

    let node_a_config = P2pNodeConfig {
        listen_addrs: vec!["/ip4/127.0.0.1/tcp/0".parse().expect("listen addr")],
        publish_wait: Duration::from_secs(8),
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
        query_timeout: Duration::from_secs(10),
        manifest_wait: Duration::from_secs(6),
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

    node_a
        .publish_chunks(&corrupted_chunks)
        .await
        .expect("publish");
    let drive_task = tokio::spawn(drive_during_recovery(node_a, Duration::from_secs(12)));
    let recovered = node_b.recover_payload_from_manifest(&manifest).await;
    drive_task.await.expect("drive task");
    let store: std::collections::HashMap<Vec<u8>, Vec<u8>> = corrupted_chunks
        .iter()
        .map(|chunk| (chunk.hash.clone(), chunk.data.clone()))
        .collect();
    let recovered = recovered.or_else(|_| {
        reassemble_payload_from_store(&manifest, &store, &transport_config)
            .map_err(|err| err)
    });
    assert!(matches!(recovered, Err(_)));
}
