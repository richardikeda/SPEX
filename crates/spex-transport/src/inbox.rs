use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use libp2p::kad::{record::Key as RecordKey, GetRecordOk, Kademlia, QueryId, Quorum};
use libp2p::kad::store::MemoryStore;
use serde::{Deserialize, Serialize};

use spex_core::hash::{hash_bytes, HashId};

use crate::error::TransportError;

#[derive(Clone, Debug)]
pub struct InboxScanRequest {
    pub record_key: RecordKey,
    pub hashed_key: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InboxScanResponse {
    pub items: Vec<Vec<u8>>,
    pub source: InboxSource,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InboxSource {
    Kademlia,
    Bridge,
}

/// Derives the DHT record key and hashed key for inbox scanning.
pub fn derive_inbox_scan_key(hash_id: HashId, inbox_scan_key: &[u8]) -> InboxScanRequest {
    let hashed_key = hash_bytes(hash_id, inbox_scan_key);
    InboxScanRequest {
        record_key: RecordKey::new(&hashed_key),
        hashed_key,
    }
}

/// Starts a Kademlia record lookup for the derived inbox scan key.
pub fn start_inbox_scan(
    kademlia: &mut Kademlia<MemoryStore>,
    request: &InboxScanRequest,
) -> QueryId {
    kademlia.get_record(request.record_key.clone(), Quorum::One)
}

/// Collects inbox payloads returned from a successful Kademlia lookup.
pub fn collect_inbox_items(result: GetRecordOk) -> InboxScanResponse {
    let items = result
        .records
        .into_iter()
        .map(|record| record.record.value)
        .collect();
    InboxScanResponse {
        items,
        source: InboxSource::Kademlia,
    }
}

/// Resolves an inbox scan, falling back to a bridge client when Kademlia yields no items.
pub async fn resolve_inbox_with_fallback(
    inbox_scan_key: &[u8],
    kademlia_result: Option<GetRecordOk>,
    bridge: Option<&BridgeClient>,
) -> Result<InboxScanResponse, TransportError> {
    if let Some(result) = kademlia_result {
        let response = collect_inbox_items(result);
        if !response.items.is_empty() || bridge.is_none() {
            return Ok(response);
        }
    }

    if let Some(bridge_client) = bridge {
        return bridge_client.scan_inbox(inbox_scan_key).await;
    }

    Ok(InboxScanResponse {
        items: Vec::new(),
        source: InboxSource::Kademlia,
    })
}

#[derive(Clone, Debug)]
pub struct BridgeClient {
    base_url: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize, Deserialize)]
struct BridgeInboxResponse {
    items: Vec<String>,
}

impl BridgeClient {
    /// Creates a new bridge client for the provided base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Scans the bridge inbox endpoint for payloads matching the inbox scan key.
    pub async fn scan_inbox(
        &self,
        inbox_scan_key: &[u8],
    ) -> Result<InboxScanResponse, TransportError> {
        let encoded_key = hex::encode(inbox_scan_key);
        let url = format!("{}/inbox/{}", self.base_url.trim_end_matches('/'), encoded_key);
        let response = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?;
        let payload: BridgeInboxResponse = response.json().await?;
        let mut items = Vec::with_capacity(payload.items.len());
        for item in payload.items {
            let decoded = BASE64_STANDARD
                .decode(item.as_bytes())
                .map_err(|_| TransportError::BridgePayload)?;
            items.push(decoded);
        }
        Ok(InboxScanResponse {
            items,
            source: InboxSource::Bridge,
        })
    }
}
