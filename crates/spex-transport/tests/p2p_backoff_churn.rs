use std::time::{Duration, Instant};

use libp2p::identity::Keypair;
use spex_transport::chunking::{chunk_data, ChunkingConfig};
use spex_transport::transport::ChunkDescriptor;
use spex_transport::{
    Chunk, ChunkManifest, P2pNodeConfig, P2pRuntimeProfile, P2pTransport, TransportConfig,
};

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
    assert!(elapsed >= Duration::from_millis(400));
    assert!(elapsed <= Duration::from_secs(8));

    let snapshot = node.metrics_snapshot();
    assert!(snapshot.publish_timeout > 0);
    assert!(snapshot.publish_retries + snapshot.publish_timeout > 0);
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
    assert!(elapsed >= Duration::from_secs(2));
    assert!(elapsed <= Duration::from_secs(5));

    let snapshot = node.metrics_snapshot();
    assert!(snapshot.recovery_retries > 0);
    assert!(snapshot.recovery_timeout > 0);
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
