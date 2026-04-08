// SPDX-License-Identifier: MPL-2.0
use std::time::{Duration, Instant};

use libp2p::{identity::Keypair, PeerId};
use spex_transport::chunking::{chunk_data, ChunkingConfig};
use spex_transport::transport::ChunkDescriptor;
use spex_transport::{
    Chunk, ChunkManifest, P2pNodeConfig, P2pRuntimeProfile, P2pTransport, PeerReputationState,
    TransportConfig,
};
use spex_transport::{NetworkHealthStatus, NetworkHealthThresholds};

const CHURN_PUBLISH_MIN_MS: u64 = 400;
const CHURN_PUBLISH_MAX_MS: u64 = 8_000;
const CHURN_RECOVERY_MIN_MS: u64 = 2_000;
const CHURN_RECOVERY_MAX_MS: u64 = 5_000;
const CHURN_MAX_PUBLISH_RETRY_PRESSURE_BPS: u32 = 50_000;
const CHURN_MAX_RECOVERY_RETRY_PRESSURE_BPS: u32 = 50_000;

/// Builds a manifest descriptor list from generated chunks for test publication.
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

/// Verifies profile defaults expose explicit timeout values used by callers.
#[test]
fn profile_timeouts_are_explicit_and_ordered() {
    let dev = P2pNodeConfig::for_profile(P2pRuntimeProfile::Dev);
    let test = P2pNodeConfig::for_profile(P2pRuntimeProfile::Test);
    let prod = P2pNodeConfig::for_profile(P2pRuntimeProfile::Prod);

    assert!(dev.publish_wait < prod.publish_wait);
    assert!(dev.query_timeout < prod.query_timeout);
    assert!(test.manifest_wait <= prod.manifest_wait);
}

/// Validates publish backoff on churn-like conditions by measuring retry counters and convergence.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn publish_backoff_records_retries_under_degraded_network() {
    let mut node = P2pTransport::new(
        Keypair::generate_ed25519(),
        TransportConfig::default(),
        P2pNodeConfig {
            publish_wait: Duration::from_secs(2),
            ..P2pNodeConfig::for_profile(P2pRuntimeProfile::Test)
        },
    )
    .await
    .expect("node");

    let payload = b"degraded publish";
    let cfg = ChunkingConfig {
        chunk_size: 8,
        ..ChunkingConfig::default()
    };
    let chunks = chunk_data(&cfg, payload);
    let manifest = build_manifest_from_chunks(&chunks, payload.len());

    let started = Instant::now();
    let result = node
        .publish_to_inboxes(&[b"no-peers-inbox".to_vec()], &manifest, &chunks)
        .await;
    let elapsed = started.elapsed();

    assert!(result.is_err(), "publish should fail without peers");
    assert!(elapsed >= Duration::from_millis(CHURN_PUBLISH_MIN_MS));
    assert!(elapsed <= Duration::from_millis(CHURN_PUBLISH_MAX_MS));

    let snapshot = node.metrics_snapshot();
    assert!(snapshot.publish_timeout > 0);
    assert!(snapshot.publish_retries + snapshot.publish_timeout > 0);
    assert!(snapshot.publish_retry_pressure_bps() <= CHURN_MAX_PUBLISH_RETRY_PRESSURE_BPS);
}

/// Validates recovery/query timeout under degraded network while tracking retry and convergence bounds.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recovery_backoff_times_out_with_retry_metrics() {
    let mut node = P2pTransport::new(
        Keypair::generate_ed25519(),
        TransportConfig::default(),
        P2pNodeConfig {
            query_timeout: Duration::from_secs(2),
            manifest_wait: Duration::from_secs(2),
            ..P2pNodeConfig::for_profile(P2pRuntimeProfile::Test)
        },
    )
    .await
    .expect("node");

    let started = Instant::now();
    let recovered = node
        .recover_payloads_for_inbox(b"inbox-with-churn", Duration::from_secs(2))
        .await
        .expect("recover call should return result vector");
    let elapsed = started.elapsed();

    assert!(recovered.is_empty());
    assert!(elapsed >= Duration::from_millis(CHURN_RECOVERY_MIN_MS));
    assert!(elapsed <= Duration::from_millis(CHURN_RECOVERY_MAX_MS));

    let snapshot = node.metrics_snapshot();
    assert!(snapshot.recovery_retries > 0);
    assert!(snapshot.recovery_timeout > 0);
    assert!(snapshot.recovery_retry_pressure_bps() <= CHURN_MAX_RECOVERY_RETRY_PRESSURE_BPS);
}

/// Verifies connectivity-aware timeout tuning never exceeds configured profile caps.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn timeout_tuning_by_profile_respects_bounds() {
    let mut node = P2pTransport::new(
        Keypair::generate_ed25519(),
        TransportConfig::default(),
        P2pNodeConfig::for_profile(P2pRuntimeProfile::Prod),
    )
    .await
    .expect("node");

    let (publish, query, manifest) = node.tuned_timeouts();
    assert!(publish <= Duration::from_secs(4));
    assert!(query <= Duration::from_secs(8));
    assert!(manifest <= Duration::from_secs(8));

    node.drive_for(Duration::from_millis(300)).await;
    let (publish_after, query_after, manifest_after) = node.tuned_timeouts();
    assert!(publish_after <= publish);
    assert!(query_after <= query);
    assert!(manifest_after <= manifest);
}

/// Verifies intermittent timeout-only failures trigger probation but avoid immediate bans.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn intermittent_peer_timeout_penalties_do_not_immediately_ban() {
    let mut node = P2pTransport::new(
        Keypair::generate_ed25519(),
        TransportConfig::default(),
        P2pNodeConfig {
            peer_ban_duration: Duration::from_secs(60),
            peer_probation_duration: Duration::from_secs(60),
            ..P2pNodeConfig::for_profile(P2pRuntimeProfile::Test)
        },
    )
    .await
    .expect("node");

    let peer = PeerId::random();
    for _ in 0..5 {
        node.report_timeout(peer);
    }

    let snapshot = node.peer_reputation_snapshot(peer);
    assert_eq!(snapshot.timeout_penalties, 5);
    assert_eq!(snapshot.state, PeerReputationState::Probation);
    assert!(!node.is_peer_banned(&peer));

    for _ in 0..4 {
        node.report_successful_interaction(peer);
    }
    assert!(!node.is_peer_probationary(&peer));
}

/// Verifies health status thresholds classify churn behavior without silent ambiguity.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_health_thresholds_classify_degraded_and_critical_under_churn() {
    let node = P2pTransport::new(
        Keypair::generate_ed25519(),
        TransportConfig::default(),
        P2pNodeConfig::for_profile(P2pRuntimeProfile::Test),
    )
    .await
    .expect("node");

    let degraded = node.network_health_indicators(NetworkHealthThresholds {
        min_connected_peers: 0,
        max_timeout_ratio_bps: 5_000,
        max_fallback_failure_ratio_bps: 5_000,
    });
    assert_eq!(degraded.status, NetworkHealthStatus::Degraded);

    let critical = node.network_health_indicators(NetworkHealthThresholds {
        min_connected_peers: 1,
        max_timeout_ratio_bps: 5_000,
        max_fallback_failure_ratio_bps: 5_000,
    });
    assert_eq!(critical.status, NetworkHealthStatus::Critical);
}
