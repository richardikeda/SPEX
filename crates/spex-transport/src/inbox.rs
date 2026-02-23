use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use libp2p::kad::store::MemoryStore;
use libp2p::kad::{Behaviour as Kademlia, GetRecordOk, QueryId, RecordKey};
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
    kademlia.get_record(request.record_key.clone())
}

/// Collects inbox payloads returned from a successful Kademlia lookup.
pub fn collect_inbox_items(result: GetRecordOk) -> InboxScanResponse {
    let items = match result {
        GetRecordOk::FoundRecord(record) => vec![record.record.value],
        GetRecordOk::FinishedWithNoAdditionalRecord { .. } => Vec::new(),
    };
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

const DEFAULT_BRIDGE_PAGE_LIMIT: usize = 100;

#[derive(Debug, Serialize, Deserialize)]
struct BridgeInboxResponse {
    items: Vec<String>,
    next_cursor: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BridgePublishRequest {
    pub data: String,
    pub grant: BridgeGrantPayload,
    pub puzzle: BridgePuzzlePayload,
    pub ttl_seconds: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BridgeGrantPayload {
    pub user_id: String,
    pub role: u64,
    pub flags: Option<u64>,
    pub expires_at: Option<u64>,
    pub verifying_key: String,
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BridgePuzzlePayload {
    pub recipient_key: String,
    pub puzzle_input: String,
    pub puzzle_output: String,
    pub params: Option<BridgePowParams>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BridgePowParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub output_len: usize,
}

impl BridgeClient {
    /// Creates a new bridge client for the provided base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        // Disables proxy usage to ensure local bridge calls stay on-loopback.
        let client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            base_url: base_url.into(),
            client,
        }
    }

    /// Scans the bridge inbox endpoint for payloads matching the inbox scan key.
    pub async fn scan_inbox(
        &self,
        inbox_scan_key: &[u8],
    ) -> Result<InboxScanResponse, TransportError> {
        let encoded_key = hex::encode(inbox_scan_key);
        let mut items = Vec::new();
        let mut cursor: Option<i64> = None;
        loop {
            let mut url = format!(
                "{}/inbox/{}?limit={}",
                self.base_url.trim_end_matches('/'),
                encoded_key,
                DEFAULT_BRIDGE_PAGE_LIMIT
            );
            if let Some(cursor_value) = cursor {
                url.push_str(&format!("&cursor={cursor_value}"));
            }
            let response = self.client.get(url).send().await?.error_for_status()?;
            let payload: BridgeInboxResponse = response.json().await?;
            for item in payload.items {
                let decoded = BASE64_STANDARD
                    .decode(item.as_bytes())
                    .map_err(|_| TransportError::BridgePayload)?;
                items.push(decoded);
            }
            if let Some(next_cursor) = payload.next_cursor {
                cursor = Some(next_cursor);
            } else {
                break;
            }
        }
        Ok(InboxScanResponse {
            items,
            source: InboxSource::Bridge,
        })
    }

    /// Publishes an envelope to the bridge inbox.
    pub async fn publish_to_inbox(
        &self,
        inbox_scan_key: &[u8],
        request: &BridgePublishRequest,
    ) -> Result<(), TransportError> {
        let encoded_key = hex::encode(inbox_scan_key);
        let url = format!(
            "{}/inbox/{}",
            self.base_url.trim_end_matches('/'),
            encoded_key
        );
        self.client
            .put(&url)
            .json(request)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spex_core::hash::HashId;

    /// Tests that the inbox scan key is correctly derived using SHA-256.
    #[test]
    fn test_derive_inbox_scan_key() {
        let scan_key = b"test_scan_key";
        let hash_id = HashId::Sha256;
        let request = derive_inbox_scan_key(hash_id, scan_key);

        // Verify the hashed key matches the SHA-256 hash of the scan key.
        let expected_hash = spex_core::hash::hash_bytes(hash_id, scan_key);
        assert_eq!(
            request.hashed_key, expected_hash,
            "Hashed key mismatch for derive_inbox_scan_key"
        );

        // Verify the record key is correctly created from the hashed key.
        assert_eq!(
            request.record_key,
            RecordKey::new(&expected_hash),
            "Record key mismatch for derive_inbox_scan_key"
        );
    }
}
