use serde::{Deserialize, Serialize};
use spex_core::hash::{hash_bytes, HashId};

/// Encodes operational health status classes used by dashboards and alerts.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NetworkHealthStatus {
    Healthy,
    Degraded,
    Critical,
}

/// Defines threshold values used to classify transport network health.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkHealthThresholds {
    pub min_connected_peers: usize,
    pub max_timeout_ratio_bps: u32,
    pub max_fallback_failure_ratio_bps: u32,
}

impl Default for NetworkHealthThresholds {
    /// Returns conservative defaults for production-safe health classification.
    fn default() -> Self {
        Self {
            min_connected_peers: 2,
            max_timeout_ratio_bps: 2_500,
            max_fallback_failure_ratio_bps: 3_000,
        }
    }
}

/// Captures computed indicators consumed by continuous network health monitoring.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkHealthIndicators {
    pub connected_peers: usize,
    pub known_peers: usize,
    pub banned_peers: usize,
    pub timeout_ratio_bps: u32,
    pub fallback_failure_ratio_bps: u32,
    pub status: NetworkHealthStatus,
}

/// Correlation output that explicitly indicates whether minimal-context fallback was used.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationCorrelation {
    pub correlation_id: String,
    pub used_minimal_context: bool,
}

/// Builds a deterministic correlation identifier from operation class and context bytes.
pub fn derive_operation_correlation_id(operation: &str, context: &[u8]) -> String {
    let mut material = Vec::with_capacity(operation.len() + context.len() + 10);
    material.extend_from_slice(b"spex-op-v1|");
    material.extend_from_slice(operation.as_bytes());
    material.extend_from_slice(b"|");
    material.extend_from_slice(context);
    let digest = hash_bytes(HashId::Sha256, &material);
    hex::encode(&digest[..16])
}

/// Builds a deterministic correlation identifier for incomplete telemetry contexts.
pub fn derive_minimal_correlation_id(operation: &str) -> String {
    derive_operation_correlation_id(operation, b"missing-context")
}

/// Builds operation correlation output and marks whether minimal fallback context was used.
pub fn derive_operation_correlation(
    operation: &str,
    context: Option<&[u8]>,
) -> OperationCorrelation {
    match context {
        Some(bytes) if !bytes.is_empty() => OperationCorrelation {
            correlation_id: derive_operation_correlation_id(operation, bytes),
            used_minimal_context: false,
        },
        _ => OperationCorrelation {
            correlation_id: derive_minimal_correlation_id(operation),
            used_minimal_context: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies correlation IDs are deterministic and operation-scoped.
    #[test]
    fn test_derive_operation_correlation_id_is_deterministic() {
        let one = derive_operation_correlation_id("recovery", b"abc");
        let two = derive_operation_correlation_id("recovery", b"abc");
        let other = derive_operation_correlation_id("publish", b"abc");
        assert_eq!(one, two);
        assert_ne!(one, other);
    }

    /// Verifies missing telemetry context falls back to a stable correlation identifier.
    #[test]
    fn test_derive_minimal_correlation_id_for_missing_metadata() {
        let missing = derive_minimal_correlation_id("recovery");
        let explicit = derive_operation_correlation_id("recovery", b"missing-context");
        assert_eq!(missing, explicit);
    }

    /// Verifies optional context selection is deterministic and reports fallback usage.
    #[test]
    fn test_derive_operation_correlation_reports_fallback_usage() {
        let fallback = derive_operation_correlation("publish", None);
        let empty = derive_operation_correlation("publish", Some(&[]));
        let contextual = derive_operation_correlation("publish", Some(b"inbox-key"));

        assert!(fallback.used_minimal_context);
        assert!(empty.used_minimal_context);
        assert!(!contextual.used_minimal_context);
        assert_eq!(fallback.correlation_id, empty.correlation_id);
        assert_ne!(fallback.correlation_id, contextual.correlation_id);
    }
}
