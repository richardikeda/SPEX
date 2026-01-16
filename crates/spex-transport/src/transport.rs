use std::time::Duration;

use libp2p::gossipsub::{
    Behaviour as Gossipsub, ConfigBuilder as GossipsubConfigBuilder, IdentTopic,
    MessageAuthenticity,
};
use libp2p::identity::Keypair;
use libp2p::kad::store::MemoryStore;
use libp2p::kad::{Kademlia, KademliaConfig, Mode, Quorum, Record, RecordKey};
use libp2p::{Multiaddr, PeerId};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::chunking::{chunk_data, Chunk, ChunkingConfig};
use crate::error::TransportError;

#[derive(Clone, Debug)]
pub struct TransportConfig {
    pub chunking: ChunkingConfig,
    pub gossip_topic: IdentTopic,
    pub record_ttl: Duration,
}

impl Default for TransportConfig {
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
    pub fn new(local_key: &Keypair, listen_addrs: &[Multiaddr]) -> Result<TransportComponents, TransportError> {
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

fn publish_manifest(
    gossip: &mut Gossipsub,
    config: &TransportConfig,
    manifest: &ChunkManifest,
) -> Result<(), TransportError> {
    let payload = serde_json::to_vec(manifest)?;
    gossip.publish(config.gossip_topic.clone(), payload)?;
    Ok(())
}

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

pub fn random_walk(kademlia: &mut Kademlia<MemoryStore>) {
    let mut random_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut random_bytes);
    let random_key = RecordKey::new(&random_bytes);
    let _ = kademlia.get_closest_peers(random_key);
}
