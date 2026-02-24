use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use libp2p::identity::Keypair;
use libp2p::PeerId;
use spex_transport::{
    decode_bootstrap_snapshot, encode_bootstrap_snapshot, P2pNodeConfig, P2pTransport,
    PersistedBootstrapState, PersistedPeer, TransportConfig,
};

/// Drives two nodes together so both sides can process connection handshakes.
async fn drive_pair_until_connected(
    left: &mut P2pTransport,
    right: &mut P2pTransport,
    timeout: Duration,
) {
    let deadline = tokio::time::Instant::now() + timeout;
    while (left.connected_peer_count() == 0 || right.connected_peer_count() == 0)
        && tokio::time::Instant::now() < deadline
    {
        left.drive_for(Duration::from_millis(150)).await;
        right.drive_for(Duration::from_millis(150)).await;
    }
}

/// Builds an isolated snapshot path for each test execution.
fn unique_snapshot_path(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("spex-{tag}-{nanos}.json"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_peer_persistence_across_restarts() {
    // Verifies that persisted peers are rehydrated and dialed after a runtime restart.
    let transport_config = TransportConfig::default();
    let snapshot_path = unique_snapshot_path("peer-persistence");

    let node_a_config = P2pNodeConfig {
        listen_addrs: vec!["/ip4/127.0.0.1/tcp/0".parse().expect("listen")],
        ..P2pNodeConfig::default()
    };
    let mut node_a = P2pTransport::new(
        Keypair::generate_ed25519(),
        transport_config.clone(),
        node_a_config,
    )
    .await
    .expect("node a");

    let node_a_addr = node_a.listen_addrs().first().expect("node a addr").clone();
    let node_a_peer_addr = node_a.local_peer_multiaddr(&node_a_addr);

    let first_run_config = P2pNodeConfig {
        listen_addrs: vec!["/ip4/127.0.0.1/tcp/0".parse().expect("listen")],
        bootstrap_nodes: vec![node_a_peer_addr.clone()],
        persistence_path: Some(snapshot_path.clone()),
        ..P2pNodeConfig::default()
    };
    let mut node_b = P2pTransport::new(
        Keypair::generate_ed25519(),
        transport_config.clone(),
        first_run_config,
    )
    .await
    .expect("node b");
    let node_b_addr = node_b.listen_addrs().first().expect("node b addr").clone();
    let node_b_peer_addr = node_b.local_peer_multiaddr(&node_b_addr);
    node_b
        .dial_peer(node_a_peer_addr.clone())
        .expect("dial bootstrap peer");
    node_a.dial_peer(node_b_peer_addr).expect("dial node b");

    drive_pair_until_connected(&mut node_a, &mut node_b, Duration::from_secs(6)).await;
    assert!(node_b.known_peer_count() > 0);
    drop(node_b);

    let second_run_config = P2pNodeConfig {
        listen_addrs: vec!["/ip4/127.0.0.1/tcp/0".parse().expect("listen")],
        bootstrap_nodes: vec![],
        persistence_path: Some(snapshot_path),
        ..P2pNodeConfig::default()
    };
    let mut restarted = P2pTransport::new(
        Keypair::generate_ed25519(),
        transport_config,
        second_run_config,
    )
    .await
    .expect("restarted node");

    drive_pair_until_connected(&mut node_a, &mut restarted, Duration::from_secs(6)).await;
    assert!(restarted.known_peer_count() > 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_anti_eclipse_peer_scoring() {
    // Verifies malicious peers lose score, become banned, and no longer influence random walk.
    let snapshot_path = unique_snapshot_path("anti-eclipse");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs();

    let malicious_peer = PeerId::random();
    let honest_peer = PeerId::random();
    let snapshot = PersistedBootstrapState {
        known_peers: vec![
            PersistedPeer {
                peer_id: malicious_peer.to_string(),
                addresses: vec![],
                last_seen_unix_seconds: now,
                origin_tag: "malicious-cluster".to_string(),
            },
            PersistedPeer {
                peer_id: honest_peer.to_string(),
                addresses: vec![],
                last_seen_unix_seconds: now,
                origin_tag: "honest-cluster".to_string(),
            },
        ],
        bootstrap_addrs: vec![],
        manifests: vec![],
        index_keys: vec![],
        peer_reputation: vec![],
    };
    let bytes = encode_bootstrap_snapshot(&snapshot).expect("encode snapshot");
    std::fs::write(&snapshot_path, bytes).expect("write snapshot");

    let mut node = P2pTransport::new(
        Keypair::generate_ed25519(),
        TransportConfig::default(),
        P2pNodeConfig {
            listen_addrs: vec!["/ip4/127.0.0.1/tcp/0".parse().expect("listen")],
            persistence_path: Some(snapshot_path),
            peer_ban_duration: Duration::from_secs(60),
            ..P2pNodeConfig::default()
        },
    )
    .await
    .expect("node");

    node.report_timeout(malicious_peer);
    node.report_timeout(malicious_peer);
    node.report_timeout(malicious_peer);
    node.report_inconsistent_response(malicious_peer);
    assert!(node.is_peer_probationary(&malicious_peer));
    assert!(!node.is_peer_banned(&malicious_peer));

    node.report_invalid_payload(malicious_peer);
    node.report_invalid_payload(malicious_peer);

    assert!(node.peer_score(malicious_peer) <= -70);
    assert!(node.is_peer_banned(&malicious_peer));

    node.report_successful_interaction(honest_peer);
    assert!(node.peer_score(honest_peer) >= 0);

    let candidates = node.random_walk_candidates(b"anti-eclipse", 16, 1);
    assert!(!candidates.is_empty());
}

#[test]
fn test_negative_snapshot_corrupted_and_empty_store() {
    // Ensures malformed snapshots return explicit errors and empty stores decode safely.
    let malformed = b"{not-json";
    assert!(decode_bootstrap_snapshot(malformed).is_err());

    let empty = PersistedBootstrapState::empty();
    let encoded = encode_bootstrap_snapshot(&empty).expect("encode empty");
    let decoded = decode_bootstrap_snapshot(&encoded).expect("decode empty");
    assert!(decoded.known_peers.is_empty());
    assert!(decoded.manifests.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_corrupted_snapshot_is_quarantined_with_explicit_warning() {
    // Ensures corrupted persisted state raises an explicit warning and runtime recovers safely.
    let snapshot_path = unique_snapshot_path("corrupted-recovery");
    std::fs::write(&snapshot_path, b"{invalid-json").expect("write corrupted snapshot");

    let node = P2pTransport::new(
        Keypair::generate_ed25519(),
        TransportConfig::default(),
        P2pNodeConfig {
            listen_addrs: vec!["/ip4/127.0.0.1/tcp/0".parse().expect("listen")],
            persistence_path: Some(snapshot_path.clone()),
            ..P2pNodeConfig::default()
        },
    )
    .await
    .expect("node should start with safe fallback");

    assert!(!node.persistence_warnings().is_empty());
    let parent = snapshot_path.parent().expect("parent");
    let has_quarantine = std::fs::read_dir(parent)
        .expect("read dir")
        .filter_map(Result::ok)
        .any(|entry| entry.file_name().to_string_lossy().contains("corrupt-"));
    assert!(has_quarantine);
}
