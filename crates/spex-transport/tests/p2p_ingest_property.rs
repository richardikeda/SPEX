use proptest::prelude::*;
use spex_transport::{
    ingest_validation_correlation_id, validate_p2p_grant_payload, validate_p2p_puzzle_payload,
    P2pGrantPayload, P2pPuzzlePayload,
};

proptest! {
    /// Ensures arbitrary JSON-like byte payloads never panic grant validation paths.
    #[test]
    fn grant_payload_validation_never_panics(input in proptest::collection::vec(any::<u8>(), 0..4096)) {
        if let Ok(payload) = serde_json::from_slice::<P2pGrantPayload>(&input) {
            let _ = validate_p2p_grant_payload(1_700_000_000, &payload);
        }
    }

    /// Ensures arbitrary JSON-like byte payloads never panic puzzle validation paths.
    #[test]
    fn puzzle_payload_validation_never_panics(input in proptest::collection::vec(any::<u8>(), 0..4096)) {
        if let Ok(payload) = serde_json::from_slice::<P2pPuzzlePayload>(&input) {
            let _ = validate_p2p_puzzle_payload(&payload);
        }
    }

    /// Ensures grant validation is deterministic for identical payloads and timestamp.
    #[test]
    fn grant_payload_validation_is_deterministic(input in proptest::collection::vec(any::<u8>(), 0..4096)) {
        if let Ok(payload) = serde_json::from_slice::<P2pGrantPayload>(&input) {
            let first = validate_p2p_grant_payload(1_700_000_000, &payload);
            let second = validate_p2p_grant_payload(1_700_000_000, &payload);
            prop_assert_eq!(first.is_ok(), second.is_ok());
            if let (Err(first_err), Err(second_err)) = (first, second) {
                prop_assert_eq!(first_err.to_string(), second_err.to_string());
            }
        }
    }

    /// Ensures ingest correlation helper remains deterministic for equal context hints.
    #[test]
    fn ingest_correlation_is_deterministic(input in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let first = ingest_validation_correlation_id(Some(&input));
        let second = ingest_validation_correlation_id(Some(&input));
        prop_assert_eq!(first, second);
    }
}
