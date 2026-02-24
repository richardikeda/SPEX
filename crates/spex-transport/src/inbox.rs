use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use ed25519_dalek::SigningKey;
use libp2p::kad::store::MemoryStore;
use libp2p::kad::{Behaviour as Kademlia, GetRecordOk, QueryId, RecordKey};
use serde::{Deserialize, Serialize};
use spex_core::pow;
use spex_core::sign::{ed25519_sign_hash, ed25519_verify_key};
use spex_core::types::GrantToken;
use spex_core::validation;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tracing::{info, warn};

use spex_core::hash::{hash_bytes, hash_ctap2_cbor_value, HashId};

use crate::error::TransportError;
use crate::telemetry::{derive_minimal_correlation_id, derive_operation_correlation_id};

static BRIDGE_FALLBACK_TOTAL: AtomicU64 = AtomicU64::new(0);
static BRIDGE_FALLBACK_SUCCESS: AtomicU64 = AtomicU64::new(0);
static BRIDGE_FALLBACK_FAILURE: AtomicU64 = AtomicU64::new(0);

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
    let correlation_id = if inbox_scan_key.is_empty() {
        derive_minimal_correlation_id("fallback")
    } else {
        derive_operation_correlation_id("fallback", inbox_scan_key)
    };
    if let Some(result) = kademlia_result {
        let response = collect_inbox_items(result);
        if !response.items.is_empty() || bridge.is_none() {
            return Ok(response);
        }
    }

    if let Some(bridge_client) = bridge {
        BRIDGE_FALLBACK_TOTAL.fetch_add(1, Ordering::Relaxed);
        let start = Instant::now();
        let response = bridge_client.scan_inbox(inbox_scan_key).await;
        match &response {
            Ok(ok) => {
                BRIDGE_FALLBACK_SUCCESS.fetch_add(1, Ordering::Relaxed);
                info!(target: "spex_transport::bridge", operation="fallback_bridge", correlation_id=%correlation_id, items=ok.items.len(), latency_ms=start.elapsed().as_millis() as u64, "bridge fallback succeeded");
            }
            Err(_) => {
                BRIDGE_FALLBACK_FAILURE.fetch_add(1, Ordering::Relaxed);
                warn!(target: "spex_transport::bridge", operation="fallback_bridge", correlation_id=%correlation_id, latency_ms=start.elapsed().as_millis() as u64, "bridge fallback failed");
            }
        }
        return response;
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
struct BridgeErrorResponse {
    error: String,
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

/// Input required to build a bridge inbox publish request from a canonical envelope.
#[derive(Clone, Debug)]
pub struct BridgeEnvelopePublishInput {
    pub sender_user_id: Vec<u8>,
    pub recipient_key_seed: Vec<u8>,
    pub envelope: Vec<u8>,
    pub role: u64,
    pub flags: Option<u64>,
    pub now_unix: u64,
    pub grant_ttl_seconds: u64,
    pub inbox_ttl_seconds: Option<u64>,
    pub pow_params: pow::PowParams,
}

/// Builds a complete bridge inbox publish request with signed grant and PoW puzzle.
pub fn build_bridge_publish_request(
    signing_key: &SigningKey,
    input: &BridgeEnvelopePublishInput,
) -> Result<BridgePublishRequest, TransportError> {
    let grant = GrantToken {
        user_id: input.sender_user_id.clone(),
        role: input.role,
        flags: input.flags,
        expires_at: Some(input.now_unix.saturating_add(input.grant_ttl_seconds)),
        extensions: Default::default(),
    };
    let grant_hash = hash_ctap2_cbor_value(HashId::Sha256, &grant).map_err(TransportError::from)?;
    let signature = ed25519_sign_hash(signing_key, &grant_hash);
    let verifying_key = ed25519_verify_key(signing_key);

    let nonce = pow::generate_pow_nonce(pow::PowNonceParams::default());
    let puzzle_input = pow::build_puzzle_input(&nonce, &input.recipient_key_seed);
    let puzzle_output =
        pow::generate_puzzle_output(&input.recipient_key_seed, &puzzle_input, input.pow_params)
            .map_err(TransportError::from)?;
    validation::validate_pow_puzzle(
        &input.recipient_key_seed,
        &puzzle_input,
        &puzzle_output,
        input.pow_params,
        pow::PowParams::minimum(),
    )
    .map_err(|err| TransportError::InvalidPayload(err.to_string()))?;

    Ok(BridgePublishRequest {
        data: BASE64_STANDARD.encode(&input.envelope),
        grant: BridgeGrantPayload {
            user_id: BASE64_STANDARD.encode(&grant.user_id),
            role: grant.role,
            flags: grant.flags,
            expires_at: grant.expires_at,
            verifying_key: BASE64_STANDARD.encode(verifying_key.to_bytes()),
            signature: BASE64_STANDARD.encode(signature.to_bytes()),
        },
        puzzle: BridgePuzzlePayload {
            recipient_key: BASE64_STANDARD.encode(&input.recipient_key_seed),
            puzzle_input: BASE64_STANDARD.encode(&puzzle_input),
            puzzle_output: BASE64_STANDARD.encode(&puzzle_output),
            params: Some(BridgePowParams {
                memory_kib: input.pow_params.memory_kib,
                iterations: input.pow_params.iterations,
                parallelism: input.pow_params.parallelism,
                output_len: input.pow_params.output_len,
            }),
        },
        ttl_seconds: input.inbox_ttl_seconds,
    })
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
        let response = self.client.put(&url).json(request).send().await?;
        if response.status().is_success() {
            return Ok(());
        }
        let status = response.status();
        let body = response
            .json::<BridgeErrorResponse>()
            .await
            .ok()
            .map(|parsed| parsed.error)
            .unwrap_or_default();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            if body.contains("grant expired") {
                return Err(TransportError::GrantExpired);
            }
            if body.contains("grant signature invalid") {
                return Err(TransportError::GrantInvalid);
            }
            if body.contains("puzzle validation failed") {
                return Err(TransportError::PowInvalid);
            }
        }
        if status == reqwest::StatusCode::BAD_REQUEST && body.contains("invalid inbox ttl") {
            return Err(TransportError::InvalidTtl);
        }
        Err(TransportError::InvalidPayload(body))
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

    /// Ensures envelope publish requests serialize grant and payload fields deterministically.
    #[test]
    fn test_build_bridge_publish_request_serializes_envelope() {
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let input = BridgeEnvelopePublishInput {
            sender_user_id: vec![1u8; 32],
            recipient_key_seed: vec![2u8; 32],
            envelope: vec![9, 8, 7, 6],
            role: 1,
            flags: None,
            now_unix: 100,
            grant_ttl_seconds: 30,
            inbox_ttl_seconds: Some(15),
            pow_params: spex_core::pow::PowParams::default(),
        };
        let request = build_bridge_publish_request(&signing_key, &input).expect("build publish");
        let payload = BASE64_STANDARD
            .decode(request.data.as_bytes())
            .expect("payload");
        assert_eq!(payload, input.envelope);
        assert_eq!(request.ttl_seconds, Some(15));
        assert_eq!(request.grant.expires_at, Some(130));
    }

    /// Rejects PoW parameters weaker than the enforced minimum when building requests.
    #[test]
    fn test_build_bridge_publish_request_rejects_weak_pow() {
        let signing_key = SigningKey::from_bytes(&[8u8; 32]);
        let input = BridgeEnvelopePublishInput {
            sender_user_id: vec![3u8; 32],
            recipient_key_seed: vec![4u8; 32],
            envelope: vec![1, 2, 3],
            role: 1,
            flags: None,
            now_unix: 200,
            grant_ttl_seconds: 30,
            inbox_ttl_seconds: Some(20),
            pow_params: spex_core::pow::PowParams {
                memory_kib: 1,
                iterations: 1,
                parallelism: 1,
                output_len: 32,
            },
        };
        let err = build_bridge_publish_request(&signing_key, &input).expect_err("weak pow");
        assert!(matches!(
            err,
            TransportError::InvalidPayload(_) | TransportError::CborDecode(_)
        ));
    }

    /// Ensures fallback observability handles missing context keys without failing the pipeline.
    #[tokio::test]
    async fn test_resolve_inbox_with_fallback_accepts_empty_context() {
        reset_bridge_fallback_counters_for_test();
        let response = resolve_inbox_with_fallback(&[], None, None)
            .await
            .expect("fallback resolution");
        assert!(response.items.is_empty());
        assert!(matches!(response.source, InboxSource::Kademlia));
        assert_eq!(bridge_fallback_counters(), (0, 0, 0));
    }
}

/// Returns global counters for bridge fallback attempts, successes, and failures.
pub fn bridge_fallback_counters() -> (u64, u64, u64) {
    (
        BRIDGE_FALLBACK_TOTAL.load(Ordering::Relaxed),
        BRIDGE_FALLBACK_SUCCESS.load(Ordering::Relaxed),
        BRIDGE_FALLBACK_FAILURE.load(Ordering::Relaxed),
    )
}

/// Resets bridge fallback counters for deterministic tests.
#[cfg(test)]
pub(crate) fn reset_bridge_fallback_counters_for_test() {
    BRIDGE_FALLBACK_TOTAL.store(0, Ordering::Relaxed);
    BRIDGE_FALLBACK_SUCCESS.store(0, Ordering::Relaxed);
    BRIDGE_FALLBACK_FAILURE.store(0, Ordering::Relaxed);
}
