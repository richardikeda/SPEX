use std::collections::HashMap;
use std::time::{Duration, Instant};

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

use spex_core::hash::{hash_bytes, HashId};

use crate::chunking::Chunk;
use crate::error::TransportError;
use crate::inbox::derive_inbox_scan_key;
use crate::transport::{
    manifest_payload, parse_manifest_from_gossip, reassemble_payload_from_store, ChunkManifest,
    TransportConfig,
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
        };
        transport.configure_peers().await?;
        transport.collect_listen_addrs().await?;
        Ok(transport)
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

    /// Dials a peer multiaddr and registers it for Kademlia and gossipsub.
    pub fn dial_peer(&mut self, addr: Multiaddr) -> Result<(), TransportError> {
        let (peer_id, base_addr) = split_peer_addr(&addr)?;
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
        Ok(())
    }

    /// Publishes a manifest and chunk set to the network for the provided inbox keys.
    pub async fn publish_to_inboxes(
        &mut self,
        inbox_keys: &[Vec<u8>],
        manifest: &ChunkManifest,
        chunks: &[Chunk],
    ) -> Result<(), TransportError> {
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
        self.drive_swarm(self.node_config.publish_wait, &mut ignored, |_, _| {})
            .await;
        for inbox_key in inbox_keys {
            let connect_deadline = Instant::now() + self.node_config.publish_wait;
            while self.swarm.connected_peers().next().is_none()
                && Instant::now() < connect_deadline
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
            let deadline = Instant::now() + self.node_config.publish_wait;
            let topic_hash = topic.hash();
            while Instant::now() < deadline {
                let has_subscribers = self
                    .swarm
                    .behaviour()
                    .gossipsub
                    .all_peers()
                    .any(|(_, topics)| topics.iter().any(|candidate| *candidate == &topic_hash));
                if has_subscribers {
                    break;
                }
                let _ = self.swarm.behaviour_mut().gossipsub.subscribe(&topic);
                self.drive_swarm(Duration::from_millis(200), &mut ignored, |_, _| {})
                    .await;
            }
            loop {
                match self
                    .swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(topic.clone(), payload.clone())
                {
                    Ok(_) => break,
                    Err(PublishError::InsufficientPeers) if Instant::now() < deadline => {
                        self.drive_swarm(Duration::from_millis(200), &mut ignored, |_, _| {})
                            .await;
                    }
                    Err(err) => return Err(err.into()),
                }
            }
        }
        self.drive_swarm(self.node_config.publish_wait, &mut ignored, |_, _| {})
            .await;
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
        self.drive_swarm(self.node_config.publish_wait, &mut ignored, |_, _| {})
            .await;
        Ok(())
    }

    /// Recovers payloads by listening for manifests on the inbox topic and fetching chunks.
    pub async fn recover_payloads_for_inbox(
        &mut self,
        inbox_key: &[u8],
        wait: Duration,
    ) -> Result<Vec<Vec<u8>>, TransportError> {
        let scan = derive_inbox_scan_key(HashId::Sha256, inbox_key);
        let topic = inbox_gossip_topic(&scan.hashed_key);
        self.swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&topic)
            .map_err(|err| TransportError::Libp2p(err.to_string()))?;

        let mut payloads = Vec::new();
        let mut resubscribe_at = Instant::now();
        let deadline = Instant::now() + wait;
        while Instant::now() < deadline {
            if Instant::now() >= resubscribe_at {
                let _ = self.swarm.behaviour_mut().gossipsub.subscribe(&topic);
                resubscribe_at = Instant::now() + Duration::from_secs(1);
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
                Err(_) => continue,
            };
            let store = match self.fetch_manifest_chunks(&manifest).await {
                Ok(store) => store,
                Err(_) => continue,
            };
            let payload = match reassemble_payload_from_store(&manifest, &store, &self.config) {
                Ok(payload) => payload,
                Err(_) => continue,
            };
            recovered.push(payload);
        }
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
        addr.push(Protocol::P2p(self.local_peer_id().into()));
        addr
    }

    /// Drives the swarm for a fixed duration without collecting payloads.
    pub async fn drive_for(&mut self, duration: Duration) {
        let mut ignored = Vec::new();
        self.drive_swarm(duration, &mut ignored, |_, _| {}).await;
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
        for addr in self
            .node_config
            .peers
            .iter()
            .chain(self.node_config.bootstrap_nodes.iter())
        {
            let (peer_id, base_addr) = split_peer_addr(addr)?;
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
        Ok(())
    }

    /// Fetches chunks for a manifest via Kademlia lookups, returning a hash-addressed store.
    async fn fetch_manifest_chunks(
        &mut self,
        manifest: &ChunkManifest,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, TransportError> {
        let mut pending: HashMap<QueryId, Vec<u8>> = HashMap::new();
        let expected: std::collections::HashSet<Vec<u8>> =
            manifest.chunks.iter().map(|chunk| chunk.hash.clone()).collect();
        for descriptor in &manifest.chunks {
            let record_key = RecordKey::new(&descriptor.hash);
            let query_id = self.swarm.behaviour_mut().kademlia.get_record(record_key);
            pending.insert(query_id, descriptor.hash.clone());
        }

        let deadline = Instant::now() + self.node_config.query_timeout;
        let mut store = HashMap::new();
        while !pending.is_empty() && Instant::now() < deadline {
            match self.swarm.select_next_some().await {
                SwarmEvent::Behaviour(SpexBehaviourEvent::Kademlia(
                    KademliaEvent::OutboundQueryProgressed { id, result, .. },
                )) => {
                    if let QueryResult::GetRecord(result) = result {
                        if let Ok(record_ok) = result {
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
                                            let record_key = RecordKey::new(&hash);
                                            let new_id =
                                                self.swarm.behaviour_mut().kademlia.get_record(
                                                    record_key,
                                                );
                                            pending.insert(new_id, hash);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    pending.remove(&id);
                }
                _ => {}
            }
        }

        if pending.is_empty() {
            Ok(store)
        } else {
            Err(TransportError::Libp2p(
                "timed out waiting for DHT records".to_string(),
            ))
        }
    }
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
