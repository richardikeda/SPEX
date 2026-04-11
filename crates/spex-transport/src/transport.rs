// SPDX-License-Identifier: MPL-2.0
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::Duration;

use libp2p::gossipsub::{
    Behaviour as Gossipsub, ConfigBuilder as GossipsubConfigBuilder, IdentTopic,
    MessageAuthenticity,
};
use libp2p::identity::Keypair;
use libp2p::kad::store::MemoryStore;
use libp2p::kad::{
    Behaviour as Kademlia, Config as KademliaConfig, GetRecordOk, Mode, QueryId, Quorum, Record,
    RecordKey,
};
use libp2p::{Multiaddr, PeerId};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use spex_core::cbor::from_ctap2_canonical_slice;
use spex_core::hash::{hash_bytes, HashId};
use spex_core::types::Envelope;

use crate::chunking::{chunk_data, Chunk, ChunkingConfig};
use crate::error::TransportError;
use crate::telemetry::derive_operation_correlation;

#[derive(Clone, Debug)]
pub struct TransportConfig {
    pub chunking: ChunkingConfig,
    pub gossip_topic: IdentTopic,
    pub record_ttl: Duration,
}

impl Default for TransportConfig {
    /// Builds a default transport configuration for chunking, gossip, and record TTL.
    fn default() -> Self {
        Self {
            chunking: ChunkingConfig::default(),
            gossip_topic: IdentTopic::new("spex/transport"),
            record_ttl: Duration::from_secs(60 * 60),
        }
    }
}

pub struct TransportComponents {
    pub local_peer_id: PeerId,
    pub kademlia: Kademlia<MemoryStore>,
    pub gossip: Gossipsub,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkManifest {
    pub chunks: Vec<ChunkDescriptor>,
    pub total_len: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkDescriptor {
    pub index: usize,
    pub hash: Vec<u8>,
}

/// Stores persisted observation details for a known peer.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedPeer {
    pub peer_id: String,
    pub addresses: Vec<String>,
    pub last_seen_unix_seconds: u64,
    pub origin_tag: String,
}

/// Stores persisted bootstrap state required to recover after a restart.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedBootstrapState {
    pub known_peers: Vec<PersistedPeer>,
    pub bootstrap_addrs: Vec<String>,
    pub manifests: Vec<ChunkManifest>,
    pub index_keys: Vec<String>,
    #[serde(default)]
    pub peer_reputation: Vec<PersistedPeerReputation>,
}

/// Stores persisted reputation status so runtime recovery can preserve abuse decisions.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedPeerReputation {
    pub peer_id: String,
    pub score: i32,
    pub timeout_penalties: u32,
    pub invalid_payload_penalties: u32,
    pub inconsistent_response_penalties: u32,
    pub probation_until_unix_seconds: Option<u64>,
    pub banned_until_unix_seconds: Option<u64>,
}

impl PersistedBootstrapState {
    /// Builds an empty bootstrap snapshot with deterministic defaults.
    pub fn empty() -> Self {
        Self {
            known_peers: Vec::new(),
            bootstrap_addrs: Vec::new(),
            manifests: Vec::new(),
            index_keys: Vec::new(),
            peer_reputation: Vec::new(),
        }
    }
}

pub struct BuildTransport;

/// Serializes bootstrap state in a deterministic JSON representation.
pub fn encode_bootstrap_snapshot(
    snapshot: &PersistedBootstrapState,
) -> Result<Vec<u8>, TransportError> {
    let mut canonical = snapshot.clone();
    canonical
        .known_peers
        .sort_by(|left, right| left.peer_id.cmp(&right.peer_id));
    for peer in &mut canonical.known_peers {
        peer.addresses.sort();
        peer.addresses.dedup();
    }
    canonical.bootstrap_addrs.sort();
    canonical.bootstrap_addrs.dedup();
    canonical.index_keys.sort();
    canonical.index_keys.dedup();
    canonical
        .peer_reputation
        .sort_by(|left, right| left.peer_id.cmp(&right.peer_id));
    canonical.manifests.sort_by(|left, right| {
        left.total_len
            .cmp(&right.total_len)
            .then(left.chunks.len().cmp(&right.chunks.len()))
    });
    serde_json::to_vec(&canonical).map_err(TransportError::from)
}

/// Parses a persisted bootstrap snapshot from JSON bytes.
pub fn decode_bootstrap_snapshot(
    payload: &[u8],
) -> Result<PersistedBootstrapState, TransportError> {
    serde_json::from_slice(payload).map_err(TransportError::from)
}

/// Writes a deterministic bootstrap snapshot to disk atomically using temp+rename.
pub fn write_bootstrap_snapshot_atomic(
    path: &Path,
    snapshot: &PersistedBootstrapState,
) -> Result<(), TransportError> {
    let payload = encode_bootstrap_snapshot(snapshot)?;
    let parent = path.parent().ok_or_else(|| {
        TransportError::InvalidPayload("snapshot path missing parent".to_string())
    })?;
    fs::create_dir_all(parent).map_err(|err| TransportError::Libp2p(err.to_string()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            TransportError::InvalidPayload("snapshot path missing file name".to_string())
        })?;
    let temp_path = parent.join(format!("{file_name}.tmp"));
    fs::write(&temp_path, payload).map_err(|err| TransportError::Libp2p(err.to_string()))?;
    fs::rename(temp_path, path).map_err(|err| TransportError::Libp2p(err.to_string()))?;
    Ok(())
}

/// Reads a persisted bootstrap snapshot from disk and validates decode boundaries.
pub fn read_bootstrap_snapshot(path: &Path) -> Result<PersistedBootstrapState, TransportError> {
    let payload = fs::read(path).map_err(|err| TransportError::Libp2p(err.to_string()))?;
    decode_bootstrap_snapshot(&payload)
}

impl BuildTransport {
    /// Builds libp2p transport components for Kademlia and gossipsub.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        local_key: &Keypair,
        listen_addrs: &[Multiaddr],
    ) -> Result<TransportComponents, TransportError> {
        let local_peer_id = PeerId::from(local_key.public());

        let kad_config = KademliaConfig::default();
        let store = MemoryStore::new(local_peer_id);
        let mut kademlia = Kademlia::with_config(local_peer_id, store, kad_config);
        kademlia.set_mode(Some(Mode::Server));

        let gossip_config = GossipsubConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(5))
            .build()
            .map_err(|err| TransportError::Libp2p(err.to_string()))?;
        let gossip = Gossipsub::new(
            MessageAuthenticity::Signed(local_key.clone()),
            gossip_config,
        )
        .map_err(|err| TransportError::Libp2p(err.to_string()))?;

        let mut components = TransportComponents {
            local_peer_id,
            kademlia,
            gossip,
        };

        for addr in listen_addrs {
            components
                .kademlia
                .add_address(&local_peer_id, addr.clone());
        }

        Ok(components)
    }
}

/// Performs a random-walk query using a caller-provided key for testing or deterministic walks.
pub fn random_walk_with_key(
    kademlia: &mut Kademlia<MemoryStore>,
    random_key_bytes: &[u8],
) -> RecordKey {
    let record_key = RecordKey::new(&random_key_bytes.to_vec());
    let _ = kademlia.get_closest_peers(record_key.to_vec());
    record_key
}

/// Publishes a payload by chunking it, storing chunks in Kademlia, and gossipping a manifest.
pub fn publish_payload(
    components: &mut TransportComponents,
    config: &TransportConfig,
    payload: &[u8],
) -> Result<ChunkManifest, TransportError> {
    let chunks = chunk_data(&config.chunking, payload);
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

    publish_chunks(&mut components.kademlia, config, &chunks);
    publish_manifest(&mut components.gossip, config, &manifest)?;

    Ok(manifest)
}

/// Writes hash-addressed chunks to Kademlia and announces providers.
fn publish_chunks(
    kademlia: &mut Kademlia<MemoryStore>,
    config: &TransportConfig,
    chunks: &[Chunk],
) {
    for chunk in chunks {
        let record_key = RecordKey::new(&chunk.hash);
        let record = Record {
            key: record_key.clone(),
            value: chunk.data.clone(),
            publisher: None,
            expires: Some(std::time::Instant::now() + config.record_ttl),
        };
        let _ = kademlia.put_record(record, Quorum::One);
        let _ = kademlia.start_providing(record_key);
    }
}

/// Publishes the manifest on gossipsub for peer replication.
fn publish_manifest(
    gossip: &mut Gossipsub,
    config: &TransportConfig,
    manifest: &ChunkManifest,
) -> Result<(), TransportError> {
    let payload = manifest_payload(manifest)?;
    gossip.publish(config.gossip_topic.clone(), payload)?;
    Ok(())
}

/// Serializes a chunk manifest into a gossipsub payload.
pub fn manifest_payload(manifest: &ChunkManifest) -> Result<Vec<u8>, TransportError> {
    let payload = serde_json::to_vec(manifest)?;
    Ok(payload)
}

/// Parses a gossipsub payload into a chunk manifest.
pub fn parse_manifest_from_gossip(payload: &[u8]) -> Result<ChunkManifest, TransportError> {
    let manifest = serde_json::from_slice(payload)?;
    Ok(manifest)
}

/// Recovers the first valid chunk manifest from a list of gossipsub payloads.
pub fn recover_manifest_from_gossip(payloads: &[Vec<u8>]) -> Result<ChunkManifest, TransportError> {
    for payload in payloads {
        if let Ok(manifest) = parse_manifest_from_gossip(payload) {
            if validate_manifest(&manifest).is_ok() {
                return Ok(manifest);
            }
        }
    }
    Err(TransportError::InvalidManifest(
        "no valid gossipsub manifest found".to_string(),
    ))
}

/// Starts DHT record lookups for each chunk described in the manifest.
pub fn start_manifest_chunk_queries(
    kademlia: &mut Kademlia<MemoryStore>,
    manifest: &ChunkManifest,
) -> Vec<QueryId> {
    manifest
        .chunks
        .iter()
        .map(|descriptor| {
            let record_key = RecordKey::new(&descriptor.hash);
            kademlia.get_record(record_key)
        })
        .collect()
}

/// Collects chunk data from DHT record results, validating hashes against the manifest.
pub fn collect_manifest_chunks(
    manifest: &ChunkManifest,
    results: &[GetRecordOk],
    config: &TransportConfig,
) -> Result<Vec<Chunk>, TransportError> {
    validate_manifest(manifest)?;
    let mut payloads = std::collections::HashMap::new();
    for result in results {
        if let GetRecordOk::FoundRecord(record) = result {
            payloads.insert(
                record.record.key.as_ref().to_vec(),
                record.record.value.clone(),
            );
        }
    }

    let mut ordered = Vec::with_capacity(manifest.chunks.len());
    for descriptor in &manifest.chunks {
        let data = payloads
            .get(&descriptor.hash)
            .ok_or_else(|| TransportError::MissingChunk(hex::encode(&descriptor.hash)))?;
        let computed = hash_bytes(config.chunking.hash_id, data);
        if computed != descriptor.hash {
            return Err(TransportError::ChunkHashMismatch(descriptor.index));
        }
        ordered.push(Chunk {
            index: descriptor.index,
            hash: descriptor.hash.clone(),
            data: data.clone(),
        });
    }
    Ok(ordered)
}

/// Recovers chunk data from a hash-addressed store while validating hashes.
pub fn recover_chunks_from_store(
    manifest: &ChunkManifest,
    store: &HashMap<Vec<u8>, Vec<u8>>,
    config: &TransportConfig,
) -> Result<Vec<Chunk>, TransportError> {
    validate_manifest(manifest)?;
    let mut ordered = Vec::with_capacity(manifest.chunks.len());
    for descriptor in &manifest.chunks {
        let data = store
            .get(&descriptor.hash)
            .ok_or_else(|| TransportError::MissingChunk(hex::encode(&descriptor.hash)))?;
        let computed = hash_bytes(config.chunking.hash_id, data);
        if computed != descriptor.hash {
            return Err(TransportError::ChunkHashMismatch(descriptor.index));
        }
        ordered.push(Chunk {
            index: descriptor.index,
            hash: descriptor.hash.clone(),
            data: data.clone(),
        });
    }
    Ok(ordered)
}

/// Reassembles chunk data in manifest order, validating hashes and length.
pub fn reassemble_chunks_with_manifest(
    manifest: &ChunkManifest,
    chunks: &[Chunk],
    config: &TransportConfig,
) -> Result<Vec<u8>, TransportError> {
    validate_manifest(manifest)?;

    let mut by_hash = std::collections::HashMap::new();
    for chunk in chunks {
        let computed = hash_bytes(config.chunking.hash_id, &chunk.data);
        if computed != chunk.hash {
            return Err(TransportError::ChunkHashMismatch(chunk.index));
        }
        by_hash
            .entry(chunk.hash.clone())
            .or_insert_with(|| chunk.clone());
    }

    let mut ordered = Vec::with_capacity(manifest.chunks.len());
    for descriptor in &manifest.chunks {
        let chunk = by_hash
            .get(&descriptor.hash)
            .ok_or_else(|| TransportError::MissingChunk(hex::encode(&descriptor.hash)))?;
        if chunk.index != descriptor.index {
            return Err(TransportError::ChunkIndexMismatch {
                expected: descriptor.index,
                actual: chunk.index,
            });
        }
        ordered.push(chunk.clone());
    }

    let payload = crate::chunking::reassemble_chunks(&ordered);
    if payload.len() != manifest.total_len {
        return Err(TransportError::PayloadLengthMismatch {
            expected: manifest.total_len,
            actual: payload.len(),
        });
    }
    Ok(payload)
}

/// Reassembles a payload from a hash-addressed store with manifest verification.
pub fn reassemble_payload_from_store(
    manifest: &ChunkManifest,
    store: &HashMap<Vec<u8>, Vec<u8>>,
    config: &TransportConfig,
) -> Result<Vec<u8>, TransportError> {
    let chunks = recover_chunks_from_store(manifest, store, config)?;
    reassemble_chunks_with_manifest(manifest, &chunks, config)
}

/// Builds deterministic reassemble correlation from manifest shape without chunk payload bytes.
pub fn reassemble_correlation_id(manifest: &ChunkManifest) -> String {
    let mut context = Vec::with_capacity(16);
    context.extend_from_slice(&(manifest.total_len as u64).to_be_bytes());
    context.extend_from_slice(&(manifest.chunks.len() as u64).to_be_bytes());
    derive_operation_correlation("reassemble", Some(&context)).correlation_id
}

/// Reassembles chunks into an Envelope, validating hashes and manifest length.
pub fn reconstruct_envelope(
    manifest: &ChunkManifest,
    chunks: &[Chunk],
    config: &TransportConfig,
) -> Result<Envelope, TransportError> {
    let payload = reassemble_chunks_with_manifest(manifest, chunks, config)?;
    let envelope = from_ctap2_canonical_slice(&payload)?;
    Ok(envelope)
}

/// Replicates manifest chunks by storing empty records and providing their hashes.
pub fn replicate_manifest(
    kademlia: &mut Kademlia<MemoryStore>,
    config: &TransportConfig,
    manifest: &ChunkManifest,
) {
    for chunk in &manifest.chunks {
        let record_key = RecordKey::new(&chunk.hash);
        let record = Record {
            key: record_key.clone(),
            value: Vec::new(),
            publisher: None,
            expires: Some(std::time::Instant::now() + config.record_ttl),
        };
        let _ = kademlia.put_record(record, Quorum::One);
        let _ = kademlia.start_providing(record_key);
    }
}

/// Passively replicates retrieved chunks by storing them and announcing providers.
pub fn passive_replicate_chunks(
    kademlia: &mut Kademlia<MemoryStore>,
    config: &TransportConfig,
    chunks: &[Chunk],
) {
    for chunk in chunks {
        store_chunk_record(kademlia, config, chunk);
    }
}

/// Renews the TTL for chunk records by re-storing them with a refreshed expiry.
pub fn renew_chunk_ttl(
    kademlia: &mut Kademlia<MemoryStore>,
    config: &TransportConfig,
    chunks: &[Chunk],
) {
    for chunk in chunks {
        store_chunk_record(kademlia, config, chunk);
    }
}

/// Stores a chunk record with TTL and provider advertisement.
fn store_chunk_record(
    kademlia: &mut Kademlia<MemoryStore>,
    config: &TransportConfig,
    chunk: &Chunk,
) {
    let record_key = RecordKey::new(&chunk.hash);
    let record = Record {
        key: record_key.clone(),
        value: chunk.data.clone(),
        publisher: None,
        expires: Some(std::time::Instant::now() + config.record_ttl),
    };
    let _ = kademlia.put_record(record, Quorum::One);
    let _ = kademlia.start_providing(record_key);
}

/// Performs a random-walk query to discover nearby peers via Kademlia.
pub fn random_walk(kademlia: &mut Kademlia<MemoryStore>) {
    let mut random_bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut random_bytes);
    let _ = robust_random_walk_with_seed(kademlia, &random_bytes, 3);
}

/// Performs a robust random walk by issuing multiple Kademlia queries from a seed.
pub fn robust_random_walk_with_seed(
    kademlia: &mut Kademlia<MemoryStore>,
    seed: &[u8],
    steps: usize,
) -> Vec<RecordKey> {
    let steps = steps.max(1);
    let mut record_keys = Vec::with_capacity(steps);
    for step in 0..steps {
        let mut material = Vec::with_capacity(seed.len() + 8);
        material.extend_from_slice(seed);
        material.extend_from_slice(&(step as u64).to_be_bytes());
        let derived = hash_bytes(HashId::Sha256, &material);
        let record_key = RecordKey::new(&derived);
        let _ = kademlia.get_closest_peers(record_key.to_vec());
        record_keys.push(record_key);
    }
    record_keys
}

/// Builds random-walk keys while enforcing source diversity and bounded influence.
pub fn robust_random_walk_with_sources(
    seed: &[u8],
    steps: usize,
    sources: &[String],
    max_per_source: usize,
) -> Vec<Vec<u8>> {
    let mut source_counts: HashMap<String, usize> = HashMap::new();
    let mut used_material: HashSet<Vec<u8>> = HashSet::new();
    let mut keys = Vec::new();
    let normalized_limit = max_per_source.max(1);
    for step in 0..steps.max(1) {
        let origin = sources
            .get(step % sources.len().max(1))
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let count = source_counts.entry(origin.clone()).or_insert(0);
        if *count >= normalized_limit {
            continue;
        }
        let mut material = Vec::with_capacity(seed.len() + origin.len() + 8);
        material.extend_from_slice(seed);
        material.extend_from_slice(origin.as_bytes());
        material.extend_from_slice(&(step as u64).to_be_bytes());
        let derived = hash_bytes(HashId::Sha256, &material);
        if used_material.insert(derived.clone()) {
            keys.push(derived);
            *count += 1;
        }
    }
    keys
}

/// Validates manifest structure by checking uniqueness of indices and hashes.
fn validate_manifest(manifest: &ChunkManifest) -> Result<(), TransportError> {
    if manifest.chunks.is_empty() && manifest.total_len > 0 {
        return Err(TransportError::InvalidManifest(
            "empty manifest for non-empty payload".to_string(),
        ));
    }
    let mut indices = std::collections::HashSet::new();
    let mut hashes = std::collections::HashSet::new();
    for descriptor in &manifest.chunks {
        if !indices.insert(descriptor.index) {
            return Err(TransportError::InvalidManifest(format!(
                "duplicate chunk index {}",
                descriptor.index
            )));
        }
        if !hashes.insert(descriptor.hash.clone()) {
            return Err(TransportError::InvalidManifest(
                "duplicate chunk hash".to_string(),
            ));
        }
    }
    Ok(())
}
