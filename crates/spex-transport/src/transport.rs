use std::time::Duration;

use libp2p::gossipsub::{
    Behaviour as Gossipsub, ConfigBuilder as GossipsubConfigBuilder, IdentTopic,
    MessageAuthenticity,
};
use libp2p::identity::Keypair;
use libp2p::kad::store::MemoryStore;
use libp2p::kad::{GetRecordOk, Kademlia, KademliaConfig, Mode, QueryId, Quorum, Record, RecordKey};
use libp2p::{Multiaddr, PeerId};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use spex_core::cbor::from_ctap2_canonical_slice;
use spex_core::hash::hash_bytes;
use spex_core::types::Envelope;

use crate::chunking::{chunk_data, Chunk, ChunkingConfig};
use crate::error::TransportError;

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

#[derive(Debug)]
pub struct TransportComponents {
    pub local_peer_id: PeerId,
    pub kademlia: Kademlia<MemoryStore>,
    pub gossip: Gossipsub,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkManifest {
    pub chunks: Vec<ChunkDescriptor>,
    pub total_len: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkDescriptor {
    pub index: usize,
    pub hash: Vec<u8>,
}

pub struct BuildTransport;

impl BuildTransport {
    /// Builds libp2p transport components for Kademlia and gossipsub.
    pub fn new(
        local_key: &Keypair,
        listen_addrs: &[Multiaddr],
    ) -> Result<TransportComponents, TransportError> {
        let local_peer_id = PeerId::from(local_key.public());

        let mut kad_config = KademliaConfig::default();
        kad_config.set_mode(Some(Mode::Server));
        let store = MemoryStore::new(local_peer_id);
        let kademlia = Kademlia::with_config(local_peer_id, store, kad_config);

        let gossip_config = GossipsubConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(5))
            .build()
            .map_err(|err| TransportError::Libp2p(err.to_string()))?;
        let gossip = Gossipsub::new(MessageAuthenticity::Signed(local_key.clone()), gossip_config)
            .map_err(|err| TransportError::Libp2p(err.to_string()))?;

        let mut components = TransportComponents {
            local_peer_id,
            kademlia,
            gossip,
        };

        for addr in listen_addrs {
            components.kademlia.add_address(&local_peer_id, addr.clone());
        }

        Ok(components)
    }
}

/// Performs a random-walk query using a caller-provided key for testing or deterministic walks.
pub fn random_walk_with_key(
    kademlia: &mut Kademlia<MemoryStore>,
    random_key_bytes: &[u8],
) -> RecordKey {
    let record_key = RecordKey::new(random_key_bytes);
    let _ = kademlia.get_closest_peers(record_key.clone());
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
fn publish_chunks(kademlia: &mut Kademlia<MemoryStore>, config: &TransportConfig, chunks: &[Chunk]) {
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
    let payload = serde_json::to_vec(manifest)?;
    gossip.publish(config.gossip_topic.clone(), payload)?;
    Ok(())
}

/// Parses a gossipsub payload into a chunk manifest.
pub fn parse_manifest_from_gossip(payload: &[u8]) -> Result<ChunkManifest, TransportError> {
    let manifest = serde_json::from_slice(payload)?;
    Ok(manifest)
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
            kademlia.get_record(record_key, Quorum::One)
        })
        .collect()
}

/// Collects chunk data from DHT record results, validating hashes against the manifest.
pub fn collect_manifest_chunks(
    manifest: &ChunkManifest,
    results: &[GetRecordOk],
    config: &TransportConfig,
) -> Result<Vec<Chunk>, TransportError> {
    let mut payloads = std::collections::HashMap::new();
    for result in results {
        for record in &result.records {
            payloads.insert(record.record.key.as_ref().to_vec(), record.record.value.clone());
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

/// Reassembles chunks into an Envelope, validating the manifest length.
pub fn reconstruct_envelope(
    manifest: &ChunkManifest,
    chunks: &[Chunk],
) -> Result<Envelope, TransportError> {
    let payload = crate::chunking::reassemble_chunks(chunks);
    if payload.len() != manifest.total_len {
        return Err(TransportError::PayloadLengthMismatch {
            expected: manifest.total_len,
            actual: payload.len(),
        });
    }
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
fn store_chunk_record(kademlia: &mut Kademlia<MemoryStore>, config: &TransportConfig, chunk: &Chunk) {
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
    rand::thread_rng().fill_bytes(&mut random_bytes);
    let _ = random_walk_with_key(kademlia, &random_bytes);
}
