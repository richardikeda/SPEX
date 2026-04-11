// SPDX-License-Identifier: MPL-2.0
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use libp2p::gossipsub::{
    Behaviour as Gossipsub, ConfigBuilder as GossipsubConfigBuilder, Event as GossipsubEvent,
    IdentTopic, MessageAuthenticity, PublishError,
};
use libp2p::identify::{Behaviour as Identify, Config as IdentifyConfig, Event as IdentifyEvent};
use libp2p::identity::Keypair;
use libp2p::kad::store::MemoryStore;
use libp2p::kad::{
    Behaviour as Kademlia, Config as KademliaConfig, Event as KademliaEvent, GetRecordOk, Mode,
    QueryId, QueryResult, Quorum, Record, RecordKey,
};
use libp2p::multiaddr::Protocol;
use libp2p::swarm::{NetworkBehaviour, Swarm, SwarmEvent};
use libp2p::{Multiaddr, PeerId, SwarmBuilder};
use rand::Rng;
use tracing::{info, warn};

use crate::telemetry::{
    derive_operation_correlation, NetworkHealthIndicators, NetworkHealthStatus,
    NetworkHealthThresholds,
};

use spex_core::hash::{hash_bytes, HashId};

use crate::chunking::Chunk;
use crate::error::TransportError;
use crate::inbox::{bridge_fallback_counters, derive_inbox_scan_key};
use crate::transport::{
    manifest_payload, parse_manifest_from_gossip, read_bootstrap_snapshot,
    reassemble_payload_from_store, robust_random_walk_with_sources,
    write_bootstrap_snapshot_atomic, ChunkManifest, PersistedBootstrapState, PersistedPeer,
    PersistedPeerReputation, TransportConfig,
};

/// Configuration for running a libp2p-backed transport node.
#[derive(Clone, Debug)]
pub struct P2pNodeConfig {
    pub listen_addrs: Vec<Multiaddr>,
    pub peers: Vec<Multiaddr>,
    pub bootstrap_nodes: Vec<Multiaddr>,
    pub publish_wait: Duration,
    pub query_timeout: Duration,
    pub manifest_wait: Duration,
    pub persistence_path: Option<PathBuf>,
    pub peer_ban_duration: Duration,
    pub peer_probation_duration: Duration,
    pub score_recovery_per_minute: i32,
    pub probation_score_threshold: i32,
    pub ban_score_threshold: i32,
    pub probation_clear_score: i32,
    pub invalid_payload_ban_threshold: u32,
    pub inconsistent_response_ban_threshold: u32,
}

/// Coarse-grained state class for peer reputation enforcement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerReputationState {
    Neutral,
    Probation,
    Banned,
}

/// Snapshot of one peer reputation record for deterministic assertions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerReputationSnapshot {
    pub score: i32,
    pub timeout_penalties: u32,
    pub invalid_payload_penalties: u32,
    pub inconsistent_response_penalties: u32,
    pub state: PeerReputationState,
}

/// Snapshot load outcome used for deterministic recovery diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotLoadState {
    NotConfigured,
    Missing,
    Loaded,
    QuarantinedRecovered,
}

/// Captures snapshot recovery integrity state for startup and restart flows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotRecoveryStatus {
    pub load_state: SnapshotLoadState,
    pub restored_known_peers: usize,
    pub restored_manifests: usize,
    pub restored_index_keys: usize,
    pub quarantined_snapshots: u64,
    pub last_quarantined_path: Option<String>,
}

impl Default for SnapshotRecoveryStatus {
    /// Returns a neutral startup status before any persistence decision is evaluated.
    fn default() -> Self {
        Self {
            load_state: SnapshotLoadState::NotConfigured,
            restored_known_peers: 0,
            restored_manifests: 0,
            restored_index_keys: 0,
            quarantined_snapshots: 0,
            last_quarantined_path: None,
        }
    }
}

/// Deployment profile with explicit timeout defaults for publish/query/recovery flows.
#[derive(Clone, Copy, Debug)]
pub enum P2pRuntimeProfile {
    Dev,
    Test,
    Prod,
}

/// Retry policy used by adaptive backoff operations.
#[derive(Clone, Debug)]
pub struct AdaptiveRetryConfig {
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub max_retries: u32,
    pub jitter_ratio: f64,
}

impl AdaptiveRetryConfig {
    /// Computes capped exponential delay with multiplicative jitter for one retry attempt.
    fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let exponent = 2u32.saturating_pow(attempt.min(20));
        let capped = self.max_delay.min(self.base_delay.saturating_mul(exponent));
        let mut rng = rand::rng();
        let jitter = rng.gen_range((1.0 - self.jitter_ratio)..=(1.0 + self.jitter_ratio));
        capped.mul_f64(jitter.max(0.1))
    }
}

/// Snapshot of counters and latency histograms collected for p2p transport operations.
#[derive(Clone, Debug, Default)]
pub struct P2pMetricsSnapshot {
    pub publish_success: u64,
    pub publish_timeout: u64,
    pub publish_retries: u64,
    pub publish_attempts: u64,
    pub query_success: u64,
    pub query_timeout: u64,
    pub query_retries: u64,
    pub query_attempts: u64,
    pub recovery_success: u64,
    pub recovery_timeout: u64,
    pub recovery_retries: u64,
    pub recovery_attempts: u64,
    pub fallback_attempts: u64,
    pub fallback_success: u64,
    pub fallback_failure: u64,
    pub reassemble_failures: u64,
    pub verification_failures: u64,
    pub reputation_probation_transitions: u64,
    pub reputation_ban_transitions: u64,
    pub publish_latency_ms: Vec<u64>,
    pub query_latency_ms: Vec<u64>,
    pub recovery_latency_ms: Vec<u64>,
}

impl P2pMetricsSnapshot {
    /// Returns publish success rate in basis points using attempts as denominator.
    pub fn publish_success_rate_bps(&self) -> u32 {
        if self.publish_attempts == 0 {
            return 0;
        }
        ((self.publish_success.saturating_mul(10_000)) / self.publish_attempts) as u32
    }

    /// Returns recovery timeout ratio in basis points using attempts as denominator.
    pub fn recovery_timeout_rate_bps(&self) -> u32 {
        if self.recovery_attempts == 0 {
            return 0;
        }
        ((self.recovery_timeout.saturating_mul(10_000)) / self.recovery_attempts) as u32
    }

    /// Returns fallback activation frequency in basis points relative to recovery attempts.
    pub fn fallback_frequency_bps(&self) -> u32 {
        if self.recovery_attempts == 0 {
            return 0;
        }
        ((self.fallback_attempts.saturating_mul(10_000)) / self.recovery_attempts) as u32
    }

    /// Returns publish retry pressure in basis points using publish attempts as denominator.
    pub fn publish_retry_pressure_bps(&self) -> u32 {
        if self.publish_attempts == 0 {
            return 0;
        }
        ((self.publish_retries.saturating_mul(10_000)) / self.publish_attempts) as u32
    }

    /// Returns recovery retry pressure in basis points using recovery attempts as denominator.
    pub fn recovery_retry_pressure_bps(&self) -> u32 {
        if self.recovery_attempts == 0 {
            return 0;
        }
        ((self.recovery_retries.saturating_mul(10_000)) / self.recovery_attempts) as u32
    }
}

impl Default for P2pNodeConfig {
    /// Builds a default P2P node configuration with localhost listen and short timeouts.
    fn default() -> Self {
        Self {
            listen_addrs: vec!["/ip4/0.0.0.0/tcp/0".parse().expect("default listen addr")],
            peers: Vec::new(),
            bootstrap_nodes: Vec::new(),
            publish_wait: Duration::from_secs(2),
            query_timeout: Duration::from_secs(5),
            manifest_wait: Duration::from_secs(5),
            persistence_path: None,
            peer_ban_duration: Duration::from_secs(30),
            peer_probation_duration: Duration::from_secs(20),
            score_recovery_per_minute: 4,
            probation_score_threshold: -35,
            ban_score_threshold: -70,
            probation_clear_score: -25,
            invalid_payload_ban_threshold: 2,
            inconsistent_response_ban_threshold: 4,
        }
    }
}

impl P2pNodeConfig {
    /// Builds node config defaults for explicit runtime profiles used by environments.
    pub fn for_profile(profile: P2pRuntimeProfile) -> Self {
        let mut config = Self::default();
        match profile {
            P2pRuntimeProfile::Dev => {
                config.publish_wait = Duration::from_secs(1);
                config.query_timeout = Duration::from_secs(3);
                config.manifest_wait = Duration::from_secs(3);
                config.score_recovery_per_minute = 6;
            }
            P2pRuntimeProfile::Test => {
                config.publish_wait = Duration::from_secs(2);
                config.query_timeout = Duration::from_secs(4);
                config.manifest_wait = Duration::from_secs(4);
                config.score_recovery_per_minute = 5;
            }
            P2pRuntimeProfile::Prod => {
                config.publish_wait = Duration::from_secs(4);
                config.query_timeout = Duration::from_secs(8);
                config.manifest_wait = Duration::from_secs(8);
                config.peer_ban_duration = Duration::from_secs(90);
                config.peer_probation_duration = Duration::from_secs(45);
                config.score_recovery_per_minute = 3;
            }
        }
        config
    }

    /// Builds retry policy defaults tuned for publish/query/recovery across profiles.
    fn adaptive_retry(&self) -> AdaptiveRetryConfig {
        AdaptiveRetryConfig {
            base_delay: Duration::from_millis(150),
            max_delay: self
                .publish_wait
                .max(self.query_timeout)
                .min(Duration::from_secs(2)),
            max_retries: 6,
            jitter_ratio: 0.25,
        }
    }
}

/// Mutable metrics state protected by a mutex so async flows can update counters safely.
#[derive(Default)]
struct P2pMetrics {
    snapshot: P2pMetricsSnapshot,
}

/// Captures runtime score data for one remote peer.
#[derive(Clone, Debug)]
struct PeerScore {
    score: i32,
    timeout_penalties: u32,
    invalid_payload_penalties: u32,
    inconsistent_response_penalties: u32,
    probation_until: Option<Instant>,
    banned_until: Option<Instant>,
    last_decay_at: Instant,
}

impl PeerScore {
    /// Builds a neutral peer score state used for unseen peers.
    fn neutral() -> Self {
        Self {
            score: 0,
            timeout_penalties: 0,
            invalid_payload_penalties: 0,
            inconsistent_response_penalties: 0,
            probation_until: None,
            banned_until: None,
            last_decay_at: Instant::now(),
        }
    }
}

/// Network behaviour that combines Kademlia, gossipsub, and identify.
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "SpexBehaviourEvent")]
struct SpexBehaviour {
    kademlia: Kademlia<MemoryStore>,
    gossipsub: Gossipsub,
    identify: Identify,
}

/// Event wrapper for the SPEX transport behaviour.
#[derive(Debug)]
enum SpexBehaviourEvent {
    Kademlia(KademliaEvent),
    Gossipsub(GossipsubEvent),
    Identify(()),
}

impl From<KademliaEvent> for SpexBehaviourEvent {
    /// Wraps Kademlia events for the swarm event stream.
    fn from(event: KademliaEvent) -> Self {
        Self::Kademlia(event)
    }
}

impl From<GossipsubEvent> for SpexBehaviourEvent {
    /// Wraps gossipsub events for the swarm event stream.
    fn from(event: GossipsubEvent) -> Self {
        Self::Gossipsub(event)
    }
}

impl From<IdentifyEvent> for SpexBehaviourEvent {
    /// Wraps identify events for the swarm event stream.
    fn from(event: IdentifyEvent) -> Self {
        let _ = event;
        Self::Identify(())
    }
}

/// Builds a gossipsub topic for a hashed inbox key.
pub fn inbox_gossip_topic(hashed_key: &[u8]) -> IdentTopic {
    let topic = format!("spex/transport/{}", hex::encode(hashed_key));
    IdentTopic::new(topic)
}

/// Libp2p transport node used by the CLI for real network operations.
pub struct P2pTransport {
    config: TransportConfig,
    node_config: P2pNodeConfig,
    swarm: Swarm<SpexBehaviour>,
    listen_addrs: Vec<Multiaddr>,
    peer_store: HashMap<PeerId, PersistedPeer>,
    known_manifests: Vec<ChunkManifest>,
    known_index_keys: HashSet<String>,
    peer_scores: HashMap<PeerId, PeerScore>,
    persistence_warnings: Vec<String>,
    snapshot_recovery_status: SnapshotRecoveryStatus,
    metrics: Arc<Mutex<P2pMetrics>>,
}

impl P2pTransport {
    /// Creates a new P2P transport node with libp2p swarm and configured peers.
    pub async fn new(
        keypair: Keypair,
        config: TransportConfig,
        node_config: P2pNodeConfig,
    ) -> Result<Self, TransportError> {
        let mut swarm = build_swarm(&keypair)?;
        let listen_addrs = Vec::new();
        for addr in &node_config.listen_addrs {
            swarm
                .listen_on(addr.clone())
                .map_err(|err| TransportError::Libp2p(err.to_string()))?;
        }
        let mut transport = Self {
            config,
            node_config,
            swarm,
            listen_addrs,
            peer_store: HashMap::new(),
            known_manifests: Vec::new(),
            known_index_keys: HashSet::new(),
            peer_scores: HashMap::new(),
            persistence_warnings: Vec::new(),
            snapshot_recovery_status: SnapshotRecoveryStatus::default(),
            metrics: Arc::new(Mutex::new(P2pMetrics::default())),
        };
        if let Err(err) = transport.load_persisted_state() {
            transport.persistence_warnings.push(err.to_string());
            warn!(target: "spex_transport::p2p", operation="persistence_load", error=%err, "state snapshot was recovered with a safe fallback");
        }
        transport.configure_peers().await?;
        transport.collect_listen_addrs().await?;
        transport.persist_state()?;
        Ok(transport)
    }

    /// Returns a clone of collected counters and histograms for assertions and diagnostics.
    pub fn metrics_snapshot(&self) -> P2pMetricsSnapshot {
        let mut snapshot = self
            .metrics
            .lock()
            .map(|metrics| metrics.snapshot.clone())
            .unwrap_or_default();
        let (total, success, failure) = bridge_fallback_counters();
        snapshot.fallback_attempts = total;
        snapshot.fallback_success = success;
        snapshot.fallback_failure = failure;
        snapshot
    }

    /// Computes current network health indicators from peer state and timeout/fallback ratios.
    pub fn network_health_indicators(
        &self,
        thresholds: NetworkHealthThresholds,
    ) -> NetworkHealthIndicators {
        let snapshot = self.metrics_snapshot();
        let timeout_total = snapshot
            .publish_timeout
            .saturating_add(snapshot.query_timeout)
            .saturating_add(snapshot.recovery_timeout);
        let operation_total = snapshot
            .publish_attempts
            .saturating_add(snapshot.query_attempts)
            .saturating_add(snapshot.recovery_attempts)
            .max(1);
        let timeout_ratio_bps = ((timeout_total.saturating_mul(10_000)) / operation_total) as u32;
        let fallback_total = snapshot.fallback_attempts.max(1);
        let fallback_failure_ratio_bps =
            ((snapshot.fallback_failure.saturating_mul(10_000)) / fallback_total) as u32;
        let banned_peers = self
            .peer_scores
            .values()
            .filter(|state| {
                state
                    .banned_until
                    .map(|until| until > Instant::now())
                    .unwrap_or(false)
            })
            .count();
        let connected_peers = self.connected_peer_count();
        let status = if connected_peers < thresholds.min_connected_peers
            || timeout_ratio_bps > thresholds.max_timeout_ratio_bps
            || fallback_failure_ratio_bps > thresholds.max_fallback_failure_ratio_bps
        {
            NetworkHealthStatus::Critical
        } else if connected_peers <= thresholds.min_connected_peers.saturating_add(1)
            || timeout_ratio_bps > thresholds.max_timeout_ratio_bps / 2
            || fallback_failure_ratio_bps > thresholds.max_fallback_failure_ratio_bps / 2
        {
            NetworkHealthStatus::Degraded
        } else {
            NetworkHealthStatus::Healthy
        };
        NetworkHealthIndicators {
            connected_peers,
            known_peers: self.known_peer_count(),
            banned_peers,
            timeout_ratio_bps,
            fallback_failure_ratio_bps,
            status,
        }
    }

    /// Returns persistence warnings collected during startup-safe recovery paths.
    pub fn persistence_warnings(&self) -> &[String] {
        &self.persistence_warnings
    }

    /// Returns startup recovery integrity status for the persistence snapshot.
    pub fn snapshot_recovery_status(&self) -> &SnapshotRecoveryStatus {
        &self.snapshot_recovery_status
    }

    /// Returns timeout tuning computed from profile defaults and observed connectivity.
    pub fn tuned_timeouts(&self) -> (Duration, Duration, Duration) {
        let connected = self.connected_peer_count() as u32;
        let publish = tuned_timeout(self.node_config.publish_wait, connected);
        let query = tuned_timeout(self.node_config.query_timeout, connected);
        let manifest = tuned_timeout(self.node_config.manifest_wait, connected);
        (publish, query, manifest)
    }

    /// Returns the local peer ID for this transport node.
    pub fn local_peer_id(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    /// Returns the resolved listen addresses for this transport node.
    pub fn listen_addrs(&self) -> &[Multiaddr] {
        &self.listen_addrs
    }

    /// Returns the number of currently connected peers for this node.
    pub fn connected_peer_count(&self) -> usize {
        self.swarm.connected_peers().count()
    }

    /// Returns the number of persisted known peers tracked by this runtime.
    pub fn known_peer_count(&self) -> usize {
        self.peer_store.len()
    }

    /// Dials a peer multiaddr and registers it for Kademlia and gossipsub.
    pub fn dial_peer(&mut self, addr: Multiaddr) -> Result<(), TransportError> {
        let (peer_id, base_addr) = split_peer_addr(&addr)?;
        if self.is_peer_banned(&peer_id) {
            return Err(TransportError::Libp2p(
                "peer is temporarily banned".to_string(),
            ));
        }
        self.swarm
            .behaviour_mut()
            .kademlia
            .add_address(&peer_id, base_addr);
        self.swarm
            .behaviour_mut()
            .gossipsub
            .add_explicit_peer(&peer_id);
        self.swarm
            .dial(addr)
            .map_err(|err| TransportError::Libp2p(err.to_string()))?;
        self.observe_peer(peer_id, vec![], "manual-dial");
        self.persist_state()?;
        Ok(())
    }

    /// Publishes a manifest and chunk set to the network for the provided inbox keys.
    pub async fn publish_to_inboxes(
        &mut self,
        inbox_keys: &[Vec<u8>],
        manifest: &ChunkManifest,
        chunks: &[Chunk],
    ) -> Result<(), TransportError> {
        self.known_manifests.push(manifest.clone());
        let operation_start = Instant::now();
        self.record_attempt("publish");
        let retry = self.node_config.adaptive_retry();
        let (publish_wait, _, _) = self.tuned_timeouts();
        for inbox_key in inbox_keys {
            self.known_index_keys.insert(hex::encode(inbox_key));
        }
        for chunk in chunks {
            let record_key = RecordKey::new(&chunk.hash);
            let record = Record {
                key: record_key.clone(),
                value: chunk.data.clone(),
                publisher: None,
                expires: Some(Instant::now() + self.config.record_ttl),
            };
            let _ = self
                .swarm
                .behaviour_mut()
                .kademlia
                .put_record(record, Quorum::One);
            let _ = self
                .swarm
                .behaviour_mut()
                .kademlia
                .start_providing(record_key);
        }

        let payload = manifest_payload(manifest)?;
        let mut ignored = Vec::new();
        self.drive_swarm(publish_wait, &mut ignored, |_, _| {})
            .await;
        for inbox_key in inbox_keys {
            let correlation = derive_operation_correlation("publish", Some(inbox_key));
            let correlation_id = correlation.correlation_id;
            let connect_deadline = Instant::now() + publish_wait;
            while self.swarm.connected_peers().next().is_none() && Instant::now() < connect_deadline
            {
                self.drive_swarm(Duration::from_millis(200), &mut ignored, |_, _| {})
                    .await;
            }
            let scan = derive_inbox_scan_key(HashId::Sha256, inbox_key);
            let topic = inbox_gossip_topic(&scan.hashed_key);
            self.swarm
                .behaviour_mut()
                .gossipsub
                .subscribe(&topic)
                .map_err(|err| TransportError::Libp2p(err.to_string()))?;
            let deadline = Instant::now() + publish_wait;
            let topic_hash = topic.hash();
            while Instant::now() < deadline {
                let has_subscribers = self
                    .swarm
                    .behaviour()
                    .gossipsub
                    .all_peers()
                    .any(|(_, topics)| topics.contains(&&topic_hash));
                if has_subscribers {
                    break;
                }
                let _ = self.swarm.behaviour_mut().gossipsub.subscribe(&topic);
                self.drive_swarm(Duration::from_millis(200), &mut ignored, |_, _| {})
                    .await;
            }
            if correlation.used_minimal_context {
                warn!(target: "spex_transport::p2p", operation="publish_manifest", correlation_id=%correlation_id, used_minimal_context=true, "publish correlation fallback applied");
            }
            let mut attempts = 0;
            loop {
                match self
                    .swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(topic.clone(), payload.clone())
                {
                    Ok(_) => {
                        info!(target: "spex_transport::p2p", operation="publish_manifest", correlation_id=%correlation_id, attempt=attempts, "manifest publish completed");
                        break;
                    }
                    Err(PublishError::InsufficientPeers)
                        if Instant::now() < deadline && attempts < retry.max_retries =>
                    {
                        attempts += 1;
                        self.record_retry("publish");
                        let delay = retry.delay_for_attempt(attempts);
                        warn!(target: "spex_transport::p2p", operation="publish_manifest", correlation_id=%correlation_id, attempt=attempts, delay_ms=delay.as_millis() as u64, "publish waiting for peers");
                        self.drive_swarm(delay, &mut ignored, |_, _| {}).await;
                    }
                    Err(PublishError::InsufficientPeers) => {
                        self.record_timeout("publish");
                        return Err(TransportError::Libp2p(
                            "manifest publish timed out waiting for peers".to_string(),
                        ));
                    }
                    Err(err) => return Err(err.into()),
                }
            }
        }
        self.drive_swarm(publish_wait, &mut ignored, |_, _| {})
            .await;
        self.persist_state()?;
        self.record_success("publish", operation_start.elapsed());
        Ok(())
    }

    /// Publishes chunk data to the DHT without gossipsub announcement.
    pub async fn publish_chunks(&mut self, chunks: &[Chunk]) -> Result<(), TransportError> {
        for chunk in chunks {
            let record_key = RecordKey::new(&chunk.hash);
            let record = Record {
                key: record_key.clone(),
                value: chunk.data.clone(),
                publisher: None,
                expires: Some(Instant::now() + self.config.record_ttl),
            };
            let _ = self
                .swarm
                .behaviour_mut()
                .kademlia
                .put_record(record, Quorum::One);
            let _ = self
                .swarm
                .behaviour_mut()
                .kademlia
                .start_providing(record_key);
        }
        let mut ignored = Vec::new();
        let (publish_wait, _, _) = self.tuned_timeouts();
        self.drive_swarm(publish_wait, &mut ignored, |_, _| {})
            .await;
        Ok(())
    }

    /// Recovers payloads by listening for manifests on the inbox topic and fetching chunks.
    pub async fn recover_payloads_for_inbox(
        &mut self,
        inbox_key: &[u8],
        wait: Duration,
    ) -> Result<Vec<Vec<u8>>, TransportError> {
        self.known_index_keys.insert(hex::encode(inbox_key));
        let scan = derive_inbox_scan_key(HashId::Sha256, inbox_key);
        let topic = inbox_gossip_topic(&scan.hashed_key);
        self.swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&topic)
            .map_err(|err| TransportError::Libp2p(err.to_string()))?;

        let operation_start = Instant::now();
        self.record_attempt("recovery");
        let correlation = derive_operation_correlation("recovery", Some(inbox_key));
        let correlation_id = correlation.correlation_id;
        let retry = self.node_config.adaptive_retry();
        let mut payloads = Vec::new();
        let mut resubscribe_at = Instant::now();
        let mut resubscribe_attempt = 0;
        let (_, _, manifest_wait) = self.tuned_timeouts();
        let effective_wait = wait.min(manifest_wait).max(Duration::from_millis(500));
        let deadline = Instant::now() + effective_wait;
        if correlation.used_minimal_context {
            warn!(target: "spex_transport::p2p", operation="recovery_inbox", correlation_id=%correlation_id, used_minimal_context=true, "recovery correlation fallback applied");
        }
        while Instant::now() < deadline {
            if Instant::now() >= resubscribe_at {
                let _ = self.swarm.behaviour_mut().gossipsub.subscribe(&topic);
                let delay = retry.delay_for_attempt(resubscribe_attempt.min(retry.max_retries));
                resubscribe_attempt = resubscribe_attempt.saturating_add(1);
                self.record_retry("recovery");
                info!(target: "spex_transport::p2p", operation="recovery_inbox", correlation_id=%correlation_id, attempt=resubscribe_attempt, delay_ms=delay.as_millis() as u64, "resubscribing inbox topic");
                resubscribe_at = Instant::now() + delay;
            }
            let delay = tokio::time::sleep(Duration::from_millis(200));
            tokio::pin!(delay);
            tokio::select! {
                _ = &mut delay => {}
                event = self.swarm.select_next_some() => {
                    if let SwarmEvent::Behaviour(SpexBehaviourEvent::Gossipsub(
                        GossipsubEvent::Message { message, .. },
                    )) = event
                    {
                        payloads.push(message.data.clone());
                    }
                }
            }
        }

        let mut recovered = Vec::new();
        for payload in payloads {
            let manifest = match parse_manifest_from_gossip(&payload) {
                Ok(parsed) => parsed,
                Err(_) => {
                    self.record_verification_failure();
                    warn!(target: "spex_transport::p2p", operation="manifest_parse", correlation_id=%correlation_id, "failed to parse manifest payload");
                    continue;
                }
            };
            let store = match self.fetch_manifest_chunks(&manifest).await {
                Ok(store) => store,
                Err(_) => {
                    self.record_verification_failure();
                    warn!(target: "spex_transport::p2p", operation="chunk_verify", correlation_id=%correlation_id, "failed to verify chunk set");
                    continue;
                }
            };
            let payload = match reassemble_payload_from_store(&manifest, &store, &self.config) {
                Ok(payload) => payload,
                Err(_) => {
                    self.record_reassemble_failure();
                    warn!(target: "spex_transport::p2p", operation="reassemble", correlation_id=%correlation_id, "failed to rebuild payload from chunk store");
                    continue;
                }
            };
            recovered.push(payload);
        }
        if recovered.is_empty() {
            self.record_timeout("recovery");
        } else {
            self.record_success("recovery", operation_start.elapsed());
        }
        self.persist_state()?;
        Ok(recovered)
    }

    /// Recovers a payload directly from a manifest by fetching and validating its chunks.
    pub async fn recover_payload_from_manifest(
        &mut self,
        manifest: &ChunkManifest,
    ) -> Result<Vec<u8>, TransportError> {
        let store = self.fetch_manifest_chunks(manifest).await?;
        reassemble_payload_from_store(manifest, &store, &self.config)
    }

    /// Builds a multiaddr with the local peer ID appended.
    pub fn local_peer_multiaddr(&self, base: &Multiaddr) -> Multiaddr {
        let mut addr = base.clone();
        addr.push(Protocol::P2p(self.local_peer_id()));
        addr
    }

    /// Drives the swarm for a fixed duration without collecting payloads.
    pub async fn drive_for(&mut self, duration: Duration) {
        let mut ignored = Vec::new();
        self.drive_swarm(duration, &mut ignored, |_, _| {}).await;
    }

    /// Applies timeout penalties to a peer and disconnects if ban policy triggers.
    pub fn report_timeout(&mut self, peer_id: PeerId) {
        self.apply_penalty(peer_id, 8, "timeout", |score| score.timeout_penalties += 1);
    }

    /// Applies invalid-payload penalties to a peer and disconnects if ban policy triggers.
    pub fn report_invalid_payload(&mut self, peer_id: PeerId) {
        self.apply_penalty(peer_id, 30, "invalid_payload", |score| {
            score.invalid_payload_penalties += 1
        });
    }

    /// Applies inconsistent-response penalties to a peer and disconnects if ban policy triggers.
    pub fn report_inconsistent_response(&mut self, peer_id: PeerId) {
        self.apply_penalty(peer_id, 18, "inconsistent_response", |score| {
            score.inconsistent_response_penalties += 1
        });
    }

    /// Restores part of a peer score after successful interactions to avoid false-positive bans.
    pub fn report_successful_interaction(&mut self, peer_id: PeerId) {
        let state = self
            .peer_scores
            .entry(peer_id)
            .or_insert_with(PeerScore::neutral);
        state.score = (state.score + 6).min(20);
        if state.score > self.node_config.probation_clear_score {
            state.probation_until = None;
        }
        let _ = self.persist_state();
    }

    /// Returns the current score of a peer for anti-eclipse assertions.
    pub fn peer_score(&self, peer_id: PeerId) -> i32 {
        self.peer_scores
            .get(&peer_id)
            .map_or(0, |state| state.score)
    }

    /// Returns whether a peer is currently under temporary ban.
    pub fn is_peer_banned(&self, peer_id: &PeerId) -> bool {
        self.peer_scores
            .get(peer_id)
            .and_then(|state| state.banned_until)
            .is_some_and(|until| Instant::now() < until)
    }

    /// Returns whether a peer is currently in probation and should have reduced influence.
    pub fn is_peer_probationary(&self, peer_id: &PeerId) -> bool {
        self.peer_scores
            .get(peer_id)
            .and_then(|state| state.probation_until)
            .is_some_and(|until| Instant::now() < until)
    }

    /// Returns the current reputation snapshot for one peer.
    pub fn peer_reputation_snapshot(&self, peer_id: PeerId) -> PeerReputationSnapshot {
        let now = Instant::now();
        match self.peer_scores.get(&peer_id) {
            Some(state) => PeerReputationSnapshot {
                score: state.score,
                timeout_penalties: state.timeout_penalties,
                invalid_payload_penalties: state.invalid_payload_penalties,
                inconsistent_response_penalties: state.inconsistent_response_penalties,
                state: classify_peer_reputation_state(state, now),
            },
            None => PeerReputationSnapshot {
                score: 0,
                timeout_penalties: 0,
                invalid_payload_penalties: 0,
                inconsistent_response_penalties: 0,
                state: PeerReputationState::Neutral,
            },
        }
    }

    /// Returns peers selected for random walk while enforcing source diversity and influence caps.
    pub fn random_walk_candidates(
        &self,
        seed: &[u8],
        steps: usize,
        max_per_origin: usize,
    ) -> Vec<Vec<u8>> {
        let mut origins = Vec::new();
        for peer in self.peer_store.values() {
            let parsed_peer = peer
                .peer_id
                .parse()
                .unwrap_or_else(|_| self.local_peer_id());
            if !self.is_peer_banned(&parsed_peer) && !self.is_peer_probationary(&parsed_peer) {
                origins.push(peer.origin_tag.clone());
            }
        }
        robust_random_walk_with_sources(seed, steps, &origins, max_per_origin)
    }

    /// Drives the libp2p swarm for a bounded amount of time while handling events.
    async fn drive_swarm<F>(
        &mut self,
        duration: Duration,
        payloads: &mut Vec<Vec<u8>>,
        mut handler: F,
    ) where
        F: FnMut(SwarmEvent<SpexBehaviourEvent>, &mut Vec<Vec<u8>>),
    {
        let deadline = tokio::time::sleep(duration);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                _ = &mut deadline => break,
                event = self.swarm.select_next_some() => {
                    handler(event, payloads);
                }
            }
        }
    }

    /// Collects listen addresses after the swarm reports them.
    async fn collect_listen_addrs(&mut self) -> Result<(), TransportError> {
        if !self.listen_addrs.is_empty() {
            return Ok(());
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if let SwarmEvent::NewListenAddr { address, .. } = self.swarm.select_next_some().await {
                self.listen_addrs.push(address);
                if self.listen_addrs.len() >= self.node_config.listen_addrs.len() {
                    break;
                }
            }
        }
        Ok(())
    }

    /// Adds configured peers and bootstrap nodes to Kademlia and dials them.
    async fn configure_peers(&mut self) -> Result<(), TransportError> {
        let mut bootstrap_peers = Vec::new();
        let persisted_bootstrap = self.persisted_bootstrap_addrs();
        let addresses: Vec<Multiaddr> = self
            .node_config
            .peers
            .iter()
            .cloned()
            .chain(self.node_config.bootstrap_nodes.iter().cloned())
            .chain(persisted_bootstrap)
            .collect();
        for addr in &addresses {
            let (peer_id, base_addr) = split_peer_addr(addr)?;
            if self.is_peer_banned(&peer_id) {
                continue;
            }
            self.swarm
                .behaviour_mut()
                .kademlia
                .add_address(&peer_id, base_addr.clone());
            self.swarm
                .behaviour_mut()
                .gossipsub
                .add_explicit_peer(&peer_id);
            self.swarm
                .dial(addr.clone())
                .map_err(|err| TransportError::Libp2p(err.to_string()))?;
            self.observe_peer(peer_id, vec![addr.to_string()], infer_origin_tag(addr));
            if self
                .node_config
                .bootstrap_nodes
                .iter()
                .any(|bootstrap| bootstrap == addr)
            {
                bootstrap_peers.push(peer_id);
            }
        }

        if !bootstrap_peers.is_empty() {
            let _ = self.swarm.behaviour_mut().kademlia.bootstrap();
        }
        self.persist_state()?;
        Ok(())
    }

    /// Fetches chunks for a manifest via Kademlia lookups, returning a hash-addressed store.
    async fn fetch_manifest_chunks(
        &mut self,
        manifest: &ChunkManifest,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, TransportError> {
        self.record_attempt("query");
        let mut pending: HashMap<QueryId, Vec<u8>> = HashMap::new();
        let expected: std::collections::HashSet<Vec<u8>> = manifest
            .chunks
            .iter()
            .map(|chunk| chunk.hash.clone())
            .collect();
        for descriptor in &manifest.chunks {
            let record_key = RecordKey::new(&descriptor.hash);
            let query_id = self.swarm.behaviour_mut().kademlia.get_record(record_key);
            pending.insert(query_id, descriptor.hash.clone());
        }

        let started = Instant::now();
        let retry = self.node_config.adaptive_retry();
        let (_, query_timeout, _) = self.tuned_timeouts();
        let deadline = Instant::now() + query_timeout;
        let mut store = HashMap::new();
        let mut retries: HashMap<Vec<u8>, u32> = HashMap::new();
        while !pending.is_empty() && Instant::now() < deadline {
            if let SwarmEvent::Behaviour(SpexBehaviourEvent::Kademlia(
                KademliaEvent::OutboundQueryProgressed { id, result, .. },
            )) = self.swarm.select_next_some().await
            {
                if let QueryResult::GetRecord(Ok(record_ok)) = result {
                    match record_ok {
                        GetRecordOk::FoundRecord(record) => {
                            store.extend(extract_records(
                                GetRecordOk::FoundRecord(record),
                                &expected,
                                self.config.chunking.hash_id,
                            ));
                        }
                        GetRecordOk::FinishedWithNoAdditionalRecord { .. } => {
                            if Instant::now() < deadline {
                                if let Some(hash) = pending.get(&id).cloned() {
                                    let attempt = retries.entry(hash.clone()).or_insert(0);
                                    if *attempt < retry.max_retries {
                                        *attempt += 1;
                                        self.record_retry("query");
                                        let delay = retry.delay_for_attempt(*attempt);
                                        tokio::time::sleep(delay).await;
                                        let record_key = RecordKey::new(&hash);
                                        let new_id = self
                                            .swarm
                                            .behaviour_mut()
                                            .kademlia
                                            .get_record(record_key);
                                        pending.insert(new_id, hash);
                                    }
                                }
                            }
                        }
                    }
                }
                pending.remove(&id);
            }
        }

        if pending.is_empty() {
            self.record_success("query", started.elapsed());
            Ok(store)
        } else {
            self.record_timeout("query");
            Err(TransportError::Libp2p(
                "timed out waiting for DHT records".to_string(),
            ))
        }
    }

    /// Updates attempt counters for publish/query/recovery operation classes.
    fn record_attempt(&self, operation: &str) {
        if let Ok(mut metrics) = self.metrics.lock() {
            match operation {
                "publish" => metrics.snapshot.publish_attempts += 1,
                "query" => metrics.snapshot.query_attempts += 1,
                "recovery" => metrics.snapshot.recovery_attempts += 1,
                _ => {}
            }
        }
    }

    /// Updates retry counters for publish/query/recovery operation classes.
    fn record_retry(&self, operation: &str) {
        if let Ok(mut metrics) = self.metrics.lock() {
            match operation {
                "publish" => metrics.snapshot.publish_retries += 1,
                "query" => metrics.snapshot.query_retries += 1,
                "recovery" => metrics.snapshot.recovery_retries += 1,
                _ => {}
            }
        }
    }

    /// Updates timeout counters for publish/query/recovery operation classes.
    fn record_timeout(&self, operation: &str) {
        if let Ok(mut metrics) = self.metrics.lock() {
            match operation {
                "publish" => metrics.snapshot.publish_timeout += 1,
                "query" => metrics.snapshot.query_timeout += 1,
                "recovery" => metrics.snapshot.recovery_timeout += 1,
                _ => {}
            }
        }
    }

    /// Updates success counters and latency histograms for operation classes.
    fn record_success(&self, operation: &str, latency: Duration) {
        if let Ok(mut metrics) = self.metrics.lock() {
            let latency_ms = latency.as_millis() as u64;
            match operation {
                "publish" => {
                    metrics.snapshot.publish_success += 1;
                    metrics.snapshot.publish_latency_ms.push(latency_ms);
                }
                "query" => {
                    metrics.snapshot.query_success += 1;
                    metrics.snapshot.query_latency_ms.push(latency_ms);
                }
                "recovery" => {
                    metrics.snapshot.recovery_success += 1;
                    metrics.snapshot.recovery_latency_ms.push(latency_ms);
                }
                _ => {}
            }
        }
    }

    /// Increments the counter that tracks manifest reassemble failures.
    fn record_reassemble_failure(&self) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.snapshot.reassemble_failures += 1;
        }
    }

    /// Increments the counter that tracks payload verification failures.
    fn record_verification_failure(&self) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.snapshot.verification_failures += 1;
        }
    }
}

/// Infers an origin tag from the first protocol component of a multiaddr.
fn infer_origin_tag(addr: &Multiaddr) -> &str {
    match addr.iter().next() {
        Some(Protocol::Ip4(_)) => "ip4",
        Some(Protocol::Ip6(_)) => "ip6",
        Some(Protocol::Dns(_)) | Some(Protocol::Dns4(_)) | Some(Protocol::Dns6(_)) => "dns",
        _ => "unknown",
    }
}

impl P2pTransport {
    /// Applies a generic penalty function and enforces probation and temporary-ban policy.
    fn apply_penalty<F>(&mut self, peer_id: PeerId, penalty: i32, reason: &str, mutate: F)
    where
        F: FnOnce(&mut PeerScore),
    {
        let now = Instant::now();
        let mut transition = None;
        {
            let state = self
                .peer_scores
                .entry(peer_id)
                .or_insert_with(PeerScore::neutral);
            let previous_state = classify_peer_reputation_state(state, now);
            apply_score_decay(state, now, self.node_config.score_recovery_per_minute);
            state.score -= penalty;
            mutate(state);

            let reached_ban_threshold = state.score <= self.node_config.ban_score_threshold
                || state.invalid_payload_penalties
                    >= self.node_config.invalid_payload_ban_threshold
                || state.inconsistent_response_penalties
                    >= self.node_config.inconsistent_response_ban_threshold;

            if reached_ban_threshold {
                state.banned_until = Some(now + self.node_config.peer_ban_duration);
                state.probation_until = None;
                let _ = self.swarm.disconnect_peer_id(peer_id);
                self.swarm
                    .behaviour_mut()
                    .gossipsub
                    .remove_explicit_peer(&peer_id);
            } else if state.score <= self.node_config.probation_score_threshold {
                state.probation_until = Some(now + self.node_config.peer_probation_duration);
            }

            let current_state = classify_peer_reputation_state(state, now);
            if previous_state != current_state {
                transition = Some((
                    previous_state,
                    current_state,
                    state.score,
                    state.timeout_penalties,
                    state.invalid_payload_penalties,
                    state.inconsistent_response_penalties,
                ));
            }
        }

        if let Some((
            previous_state,
            current_state,
            score,
            timeout_penalties,
            invalid_payload_penalties,
            inconsistent_response_penalties,
        )) = transition
        {
            self.record_reputation_transition(current_state);
            warn!(
                target: "spex_transport::p2p",
                operation = "peer_reputation_transition",
                peer_id = %peer_id,
                reason = %reason,
                previous_state = ?previous_state,
                current_state = ?current_state,
                score,
                timeout_penalties,
                invalid_payload_penalties,
                inconsistent_response_penalties,
                "peer reputation state transitioned"
            );
        }
        let _ = self.persist_state();
    }

    /// Increments counters for probation and ban transitions.
    fn record_reputation_transition(&self, state: PeerReputationState) {
        if let Ok(mut metrics) = self.metrics.lock() {
            match state {
                PeerReputationState::Probation => {
                    metrics.snapshot.reputation_probation_transitions += 1;
                }
                PeerReputationState::Banned => {
                    metrics.snapshot.reputation_ban_transitions += 1;
                }
                PeerReputationState::Neutral => {}
            }
        }
    }

    /// Registers peer observation metadata so restart bootstrap can be deterministic.
    fn observe_peer(&mut self, peer_id: PeerId, addresses: Vec<String>, origin_tag: &str) {
        let entry = self.peer_store.entry(peer_id).or_insert(PersistedPeer {
            peer_id: peer_id.to_string(),
            addresses: Vec::new(),
            last_seen_unix_seconds: 0,
            origin_tag: origin_tag.to_string(),
        });
        for address in addresses {
            if !entry.addresses.contains(&address) {
                entry.addresses.push(address);
            }
        }
        entry.last_seen_unix_seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        entry.origin_tag = origin_tag.to_string();
    }

    /// Loads persisted state from disk and quarantines corrupted snapshots before fallback.
    fn load_persisted_state(&mut self) -> Result<(), TransportError> {
        let Some(path) = &self.node_config.persistence_path else {
            self.snapshot_recovery_status.load_state = SnapshotLoadState::NotConfigured;
            return Ok(());
        };
        if !path.exists() {
            self.snapshot_recovery_status.load_state = SnapshotLoadState::Missing;
            return Ok(());
        }
        let snapshot = match read_bootstrap_snapshot(path) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                let quarantined = self.quarantine_corrupted_snapshot(path)?;
                write_bootstrap_snapshot_atomic(path, &PersistedBootstrapState::empty())?;
                self.snapshot_recovery_status.load_state = SnapshotLoadState::QuarantinedRecovered;
                self.snapshot_recovery_status.quarantined_snapshots = self
                    .snapshot_recovery_status
                    .quarantined_snapshots
                    .saturating_add(1);
                self.snapshot_recovery_status.last_quarantined_path =
                    Some(quarantined.display().to_string());
                self.snapshot_recovery_status.restored_known_peers = 0;
                self.snapshot_recovery_status.restored_manifests = 0;
                self.snapshot_recovery_status.restored_index_keys = 0;
                return Err(TransportError::InvalidPayload(format!(
                    "corrupted persisted state detected and quarantined: {err}"
                )));
            }
        };
        self.snapshot_recovery_status.load_state = SnapshotLoadState::Loaded;
        self.snapshot_recovery_status.restored_known_peers = snapshot.known_peers.len();
        self.snapshot_recovery_status.restored_manifests = snapshot.manifests.len();
        self.snapshot_recovery_status.restored_index_keys = snapshot.index_keys.len();
        self.known_manifests = snapshot.manifests;
        self.known_index_keys = snapshot.index_keys.into_iter().collect();
        for peer in snapshot.known_peers {
            if let Ok(peer_id) = peer.peer_id.parse() {
                self.peer_store.insert(peer_id, peer);
            }
        }
        for reputation in snapshot.peer_reputation {
            if let Ok(peer_id) = reputation.peer_id.parse() {
                self.peer_scores
                    .insert(peer_id, load_peer_score(&reputation));
            }
        }
        Ok(())
    }

    /// Persists current known peers and bootstrap metadata using atomic snapshot writes.
    fn persist_state(&self) -> Result<(), TransportError> {
        let Some(path) = &self.node_config.persistence_path else {
            return Ok(());
        };
        let mut bootstrap_addrs: Vec<String> = self
            .node_config
            .bootstrap_nodes
            .iter()
            .map(ToString::to_string)
            .collect();
        for peer in self.peer_store.values() {
            bootstrap_addrs.extend(peer.addresses.iter().cloned());
        }
        let snapshot = PersistedBootstrapState {
            known_peers: self.peer_store.values().cloned().collect(),
            bootstrap_addrs,
            manifests: self.known_manifests.clone(),
            index_keys: self.known_index_keys.iter().cloned().collect(),
            peer_reputation: self
                .peer_scores
                .iter()
                .map(|(peer_id, score)| persist_peer_score(peer_id, score))
                .collect(),
        };
        write_bootstrap_snapshot_atomic(path, &snapshot)
    }

    /// Moves a corrupted snapshot away from active path so recovery can continue safely.
    fn quarantine_corrupted_snapshot(&self, path: &PathBuf) -> Result<PathBuf, TransportError> {
        let unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        let quarantined = path.with_extension(format!("corrupt-{unix}.json"));
        fs::rename(path, &quarantined).map_err(|err| TransportError::Libp2p(err.to_string()))?;
        Ok(quarantined)
    }

    /// Returns bootstrap addresses loaded from persistent snapshots for rehydration.
    fn persisted_bootstrap_addrs(&self) -> Vec<Multiaddr> {
        let mut addrs = Vec::new();
        for peer in self.peer_store.values() {
            for addr in &peer.addresses {
                if let Ok(parsed) = addr.parse() {
                    addrs.push(parsed);
                }
            }
        }
        addrs
    }
}

/// Classifies a peer reputation state from runtime probation/ban timers.
fn classify_peer_reputation_state(state: &PeerScore, now: Instant) -> PeerReputationState {
    if state.banned_until.is_some_and(|until| now < until) {
        PeerReputationState::Banned
    } else if state.probation_until.is_some_and(|until| now < until) {
        PeerReputationState::Probation
    } else {
        PeerReputationState::Neutral
    }
}

/// Computes an adaptive timeout from profile defaults and current connectivity.
fn tuned_timeout(base: Duration, connected_peers: u32) -> Duration {
    if connected_peers >= 4 {
        base.mul_f64(0.65).max(Duration::from_millis(600))
    } else if connected_peers >= 2 {
        base.mul_f64(0.8).max(Duration::from_millis(800))
    } else {
        base
    }
}

/// Applies score decay over time to avoid over-penalizing transient failures.
fn apply_score_decay(state: &mut PeerScore, now: Instant, recovery_per_minute: i32) {
    let elapsed_secs = now.saturating_duration_since(state.last_decay_at).as_secs();
    if elapsed_secs == 0 || recovery_per_minute <= 0 {
        return;
    }
    let recovered = ((elapsed_secs as i32) * recovery_per_minute) / 60;
    if recovered > 0 {
        state.score = (state.score + recovered).min(20);
        state.last_decay_at = now;
    }
}

/// Converts persisted reputation data into runtime score tracking.
fn load_peer_score(snapshot: &PersistedPeerReputation) -> PeerScore {
    PeerScore {
        score: snapshot.score,
        timeout_penalties: snapshot.timeout_penalties,
        invalid_payload_penalties: snapshot.invalid_payload_penalties,
        inconsistent_response_penalties: snapshot.inconsistent_response_penalties,
        probation_until: snapshot
            .probation_until_unix_seconds
            .map(unix_seconds_to_instant),
        banned_until: snapshot
            .banned_until_unix_seconds
            .map(unix_seconds_to_instant),
        last_decay_at: Instant::now(),
    }
}

/// Converts runtime score tracking into persisted reputation data.
fn persist_peer_score(peer_id: &PeerId, state: &PeerScore) -> PersistedPeerReputation {
    PersistedPeerReputation {
        peer_id: peer_id.to_string(),
        score: state.score,
        timeout_penalties: state.timeout_penalties,
        invalid_payload_penalties: state.invalid_payload_penalties,
        inconsistent_response_penalties: state.inconsistent_response_penalties,
        probation_until_unix_seconds: state.probation_until.map(instant_to_unix_seconds),
        banned_until_unix_seconds: state.banned_until.map(instant_to_unix_seconds),
    }
}

/// Converts a unix timestamp to a runtime instant using saturating arithmetic.
fn unix_seconds_to_instant(unix_seconds: u64) -> Instant {
    let now_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    if unix_seconds <= now_unix {
        Instant::now()
    } else {
        Instant::now() + Duration::from_secs(unix_seconds - now_unix)
    }
}

/// Converts an instant into unix timestamp for persistence and restart recovery.
fn instant_to_unix_seconds(instant: Instant) -> u64 {
    let now_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    now_unix + instant.saturating_duration_since(Instant::now()).as_secs()
}

/// Builds a libp2p swarm with Kademlia, gossipsub, and identify configured.
fn build_swarm(keypair: &Keypair) -> Result<Swarm<SpexBehaviour>, TransportError> {
    let peer_id = PeerId::from(keypair.public());
    let mut kad_config = KademliaConfig::default();
    kad_config.set_query_timeout(Duration::from_secs(5));
    let store = MemoryStore::new(peer_id);
    let mut kademlia = Kademlia::with_config(peer_id, store, kad_config);
    kademlia.set_mode(Some(Mode::Server));

    let gossip_config = GossipsubConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(1))
        .build()
        .map_err(|err| TransportError::Libp2p(err.to_string()))?;
    let gossipsub = Gossipsub::new(MessageAuthenticity::Signed(keypair.clone()), gossip_config)
        .map_err(|err| TransportError::Libp2p(err.to_string()))?;

    let identify = Identify::new(IdentifyConfig::new(
        "spex/transport/1.0".into(),
        keypair.public(),
    ));

    let behaviour = SpexBehaviour {
        kademlia,
        gossipsub,
        identify,
    };

    let swarm = SwarmBuilder::with_existing_identity(keypair.clone())
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )
        .map_err(|err| TransportError::Libp2p(err.to_string()))?
        .with_behaviour(|_| behaviour)
        .map_err(|err| TransportError::Libp2p(err.to_string()))?
        .build();

    Ok(swarm)
}

/// Splits a multiaddr containing a peer ID into the peer and base address.
fn split_peer_addr(addr: &Multiaddr) -> Result<(PeerId, Multiaddr), TransportError> {
    let mut base = Multiaddr::empty();
    let mut peer_id = None;
    for protocol in addr.iter() {
        if let Protocol::P2p(multihash) = protocol {
            peer_id = Some(multihash);
            break;
        } else {
            base.push(protocol);
        }
    }
    let peer_id = peer_id.ok_or_else(|| TransportError::Libp2p("missing peer id".to_string()))?;
    Ok((peer_id, base))
}

/// Extracts chunk records from Kademlia results and validates their hashes.
fn extract_records(
    result: GetRecordOk,
    expected: &std::collections::HashSet<Vec<u8>>,
    hash_id: HashId,
) -> HashMap<Vec<u8>, Vec<u8>> {
    let mut store = HashMap::new();
    if let GetRecordOk::FoundRecord(record) = result {
        let hash = record.record.key.as_ref().to_vec();
        if expected.contains(&hash) {
            let computed = hash_bytes(hash_id, &record.record.value);
            if computed == hash {
                store.insert(hash, record.record.value);
            }
        }
    }
    store
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies metric rates are emitted deterministically for success and failure counters.
    #[test]
    fn test_metrics_rates_cover_success_and_failure_paths() {
        let snapshot = P2pMetricsSnapshot {
            publish_success: 3,
            publish_attempts: 4,
            recovery_timeout: 2,
            recovery_attempts: 5,
            fallback_attempts: 2,
            ..Default::default()
        };
        assert_eq!(snapshot.publish_success_rate_bps(), 7_500);
        assert_eq!(snapshot.recovery_timeout_rate_bps(), 4_000);
        assert_eq!(snapshot.fallback_frequency_bps(), 4_000);
    }

    /// Ensures zero-attempt snapshots never panic and return explicit zero rates.
    #[test]
    fn test_metrics_rates_handle_missing_context() {
        let snapshot = P2pMetricsSnapshot::default();
        assert_eq!(snapshot.publish_success_rate_bps(), 0);
        assert_eq!(snapshot.recovery_timeout_rate_bps(), 0);
        assert_eq!(snapshot.fallback_frequency_bps(), 0);
    }

    /// Verifies adaptive retry stays bounded under extreme timeout configurations.
    #[test]
    fn test_adaptive_retry_bounds_with_extreme_timeouts() {
        let low = P2pNodeConfig {
            publish_wait: Duration::from_millis(50),
            query_timeout: Duration::from_millis(40),
            ..P2pNodeConfig::default()
        };
        let low_retry = low.adaptive_retry();
        let low_delay = low_retry.delay_for_attempt(50);
        assert!(low_delay <= Duration::from_millis(100));

        let high = P2pNodeConfig {
            publish_wait: Duration::from_secs(30),
            query_timeout: Duration::from_secs(40),
            ..P2pNodeConfig::default()
        };
        let high_retry = high.adaptive_retry();
        let high_delay = high_retry.delay_for_attempt(50);
        assert!(high_delay <= Duration::from_millis(2_500));
    }
}
