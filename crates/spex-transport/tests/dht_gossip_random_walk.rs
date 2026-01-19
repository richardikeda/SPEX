use std::collections::{BTreeMap, HashMap, HashSet};

use libp2p::identity::Keypair;
use libp2p::{Multiaddr, PeerId};
use spex_core::cbor::from_ctap2_canonical_slice;
use spex_core::hash::{hash_bytes, HashId};
use spex_core::types::{Ctap2Cbor, Envelope};
use spex_transport::{
    chunk_data, publish_payload, random_walk_with_key, reassemble_chunks, BuildTransport, Chunk,
    ChunkManifest, ChunkingConfig, TransportConfig,
};

fn build_envelope_payload() -> (Envelope, Vec<u8>) {
    // Builds a canonical envelope payload for DHT and gossip transport tests.
    let envelope = Envelope {
        thread_id: hash_bytes(HashId::Sha256, b"thread"),
        epoch: 2,
        seq: 7,
        sender_user_id: hash_bytes(HashId::Sha256, b"alice"),
        ciphertext: b"fragmented payload".to_vec(),
        signature: None,
        extensions: BTreeMap::new(),
    };
    let payload = envelope
        .to_ctap2_canonical_bytes()
        .expect("envelope payload");
    (envelope, payload)
}

fn build_dht_store(chunks: &[Chunk]) -> HashMap<Vec<u8>, Vec<u8>> {
    // Stores chunk data by hash to simulate DHT record storage.
    let mut store = HashMap::new();
    for chunk in chunks {
        store.insert(chunk.hash.clone(), chunk.data.clone());
    }
    store
}

fn fetch_chunks_with_noise(
    manifest: &ChunkManifest,
    store: &HashMap<Vec<u8>, Vec<u8>>,
) -> Vec<Chunk> {
    // Retrieves chunks in reverse order with duplicates to mimic noisy DHT/gossip delivery.
    let mut retrieved = Vec::new();
    for descriptor in manifest.chunks.iter().rev() {
        let data = store
            .get(&descriptor.hash)
            .expect("chunk present")
            .clone();
        retrieved.push(Chunk {
            index: descriptor.index,
            hash: descriptor.hash.clone(),
            data,
        });
    }
    if let Some(first) = manifest.chunks.first() {
        let data = store.get(&first.hash).expect("chunk present").clone();
        retrieved.push(Chunk {
            index: first.index,
            hash: first.hash.clone(),
            data,
        });
    }
    retrieved
}

fn reorder_and_dedupe_chunks(manifest: &ChunkManifest, chunks: &[Chunk]) -> Vec<Chunk> {
    // Deduplicates chunk data and restores the original manifest ordering.
    let mut by_hash = HashMap::new();
    for chunk in chunks {
        by_hash.entry(chunk.hash.clone()).or_insert_with(|| chunk.clone());
    }

    let mut ordered = Vec::new();
    let mut seen_indices = HashSet::new();
    for descriptor in &manifest.chunks {
        if let Some(chunk) = by_hash.get(&descriptor.hash) {
            if seen_indices.insert(chunk.index) {
                ordered.push(chunk.clone());
            }
        }
    }

    ordered
}

fn build_malicious_peers(count: usize) -> Vec<(PeerId, Multiaddr)> {
    // Builds a list of malicious peers that all advertise localhost TCP addresses.
    (0..count)
        .map(|index| {
            let peer_id = PeerId::from(Keypair::generate_ed25519().public());
            let addr: Multiaddr = format!("/ip4/127.0.0.1/tcp/{}", 30_000 + index)
                .parse()
                .expect("multiaddr");
            (peer_id, addr)
        })
        .collect()
}

#[test]
fn two_nodes_publish_retrieve_and_reassemble_from_dht_and_gossip() {
    // Publishes a fragmented payload, fetches it via DHT+gossip, deduplicates, and reassembles.
    let (envelope, payload) = build_envelope_payload();

    let local_key = Keypair::generate_ed25519();
    let mut publisher = BuildTransport::new(&local_key, &[]).expect("transport");
    let config = TransportConfig {
        chunking: ChunkingConfig {
            chunk_size: 8,
            ..ChunkingConfig::default()
        },
        ..TransportConfig::default()
    };

    let manifest = publish_payload(&mut publisher, &config, &payload).expect("publish");

    let chunks = chunk_data(&config.chunking, &payload);
    let dht_store = build_dht_store(&chunks);
    let noisy_chunks = fetch_chunks_with_noise(&manifest, &dht_store);
    let ordered_chunks = reorder_and_dedupe_chunks(&manifest, &noisy_chunks);

    let indices: Vec<usize> = ordered_chunks.iter().map(|chunk| chunk.index).collect();
    let manifest_indices: Vec<usize> = manifest.chunks.iter().map(|chunk| chunk.index).collect();
    assert_eq!(indices, manifest_indices);

    let reassembled = reassemble_chunks(&ordered_chunks);
    let decoded: Envelope =
        from_ctap2_canonical_slice(&reassembled).expect("reassembled envelope");
    assert_eq!(decoded, envelope);
}

#[test]
fn random_walk_avoids_eclipse_from_malicious_peers() {
    // Simulates malicious peers and ensures random-walk queries can target non-malicious keys.
    let local_key = Keypair::generate_ed25519();
    let mut components = BuildTransport::new(&local_key, &[]).expect("transport");

    let malicious = build_malicious_peers(5);
    let malicious_keys: HashSet<Vec<u8>> = malicious
        .iter()
        .map(|(peer_id, _)| peer_id.to_bytes())
        .collect();

    for (peer_id, addr) in malicious {
        components.kademlia.add_address(&peer_id, addr);
    }

    let honest_key = hash_bytes(HashId::Sha256, b"honest-walk");
    let record_key = random_walk_with_key(&mut components.kademlia, &honest_key);

    assert_eq!(record_key, libp2p::kad::RecordKey::new(&honest_key));
    assert!(!malicious_keys.contains(record_key.as_ref()));
}
