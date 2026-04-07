# SPEX Test Documentation

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

This document outlines the testing strategy, structure, and coverage for the SPEX project.

## Running Tests

To run all tests in the workspace:

```bash
cargo test --workspace
```

To run a specific test file:

```bash
cargo test --test <test_filename>
```

To run ignored tests (long-running or pending features):

```bash
cargo test -- --ignored
```

## Test Structure

The project uses standard Rust testing conventions:
- **Unit Tests:** Located within `src/` files in `mod tests` blocks. These test internal logic and private functions.
- **Integration Tests:** Located in `tests/` directories within each crate. These test the public API and component interactions.

## Component Breakdown

### 1. Core (`spex-core`)

Tests the fundamental building blocks: cryptography, serialization, and validation logic.

*   `src/lib.rs` (Unit): Tests for utility functions (e.g., big-endian conversion).
*   `tests/ctap2_cbor_vectors.rs`: Verifies CTAP2 canonical CBOR encoding against test vectors.
*   `tests/pow.rs`: Validates Proof-of-Work generation and verification (Argon2id), including difficulty checks.
*   `tests/sign_validation.rs`: Tests Ed25519 signature generation and verification for grants and cards.
*   `tests/vectors_v011.rs`: Comprehensive test vectors for v0.1.1 spec compliance (hashes, keys, signatures).
*   `tests/crypto_edge_cases.rs`: Validates edge cases for hashing (empty/large inputs) and signature verification (malformed keys/signatures).

### 2. MLS (`spex-mls`)

Tests the Messaging Layer Security integration using `mls-rs`.

*   `tests/group_context.rs`: Verifies that SPEX extensions are correctly embedded in MLS group contexts.
*   `tests/mls_membership.rs`: Tests adding/removing members and roster updates.
*   `tests/mls_real_groups.rs`: Simulates real-world group scenarios (create, add, commit, update).
*   `tests/mls_rs_integration.rs`: validtes integration with the underlying `mls-rs` library.
*   `tests/mls_scenarios.rs`: Advanced scenarios like key rotation, member permutation, and resynchronization.
*   `tests/secrets_crypto.rs`: Verifies encryption/decryption of application messages using group secrets.
*   `tests/stress_group.rs`: Stress tests for group creation and messaging with multiple members (partial/full sync).
*   `tests/planned_concurrent_updates.rs`: Tests for concurrent proposal handling and epoch recovery (active in CI/local suites).

### 3. Transport (`spex-transport`)

Tests the P2P networking layer, chunking, and data delivery.

*   `src/chunking.rs` (Unit): Tests splitting data into chunks and reassembling them.
*   `tests/dht_gossip_random_walk.rs`: Simulates DHT behavior, gossip propagation, and anti-eclipse random walks.
*   `tests/p2p_ingest_validation.rs`: Validates that the transport layer rejects invalid grants, expired tokens, and weak PoW during ingestion.
*   `tests/p2p_manifest_delivery.rs`: Tests the delivery of chunk manifests between nodes.
*   `tests/p2p_manifest_recovery.rs`: Tests recovering full payloads from scattered chunks fetched via manifests.
*   `tests/two_identity_flow.rs`: End-to-end flow between two identities using the transport layer (mocked/simulated network).
*   `tests/stress_chunking.rs`: Stress tests for chunking large payloads (10MB) and many small chunks.
*   `tests/planned_p2p_persistence.rs`: Tests for peer persistence and anti-eclipse scoring (active in CI/local suites).

### 4. Bridge (`spex-bridge`)

Tests the HTTP relay server.

*   `src/lib.rs` (Unit): Basic state initialization tests.
*   `tests/integration.rs`: Tests public API endpoints (`/cards`, `/slot`, `/inbox`), including:
    *   Round-trip storage and retrieval.
    *   PoW validation (salt, difficulty).
    *   Grant validation (signatures, expiration).
    *   Rate limiting and constraints.
*   `tests/full_flow_integration.rs`: A complex scenario simulating a full client interaction with the bridge.
*   `tests/load_test.rs`: Concurrency load test for inbox reads (SQLite locking).

### 5. Client (`spex-client`)

Tests the high-level client library that ties everything together.

*   `src/lib.rs` (Unit): Tests for failure reason mapping and internal helpers.
*   `tests/bridge_publish.rs`: Tests publishing envelopes to the bridge.
*   `tests/e2e_flow.rs`: Simulates a complete conversation between two clients (identity creation, card exchange, messaging).
*   `tests/inbox_decrypt.rs`: Verifies that inbox messages can be decrypted and parsed correctly.
*   `tests/state_encryption.rs`: Tests the encryption of the local state file (At-Rest Encryption) using passphrases.
*   `tests/state_flow.rs`: Tests state transitions (e.g., updating thread state, redeeming cards).
*   `tests/integration_scenarios.rs`: Multi-party conversation simulation (3 members) with mocked delivery.

### 6. CLI (`spex-cli`)

*   `tests/planned_cli_flow.rs`: Integration tests invoking the `spex` binary for identity and messaging flows (active in CI/local suites).

## Pending & Planned Tests

The following areas are identified for future expansion. Existing `planned_*` suites are active and no longer documentation-placeholders.

*   **Robust Libp2p Runtime:** Expand long-duration chaos and soak coverage under prolonged churn.
*   **Full MLS Sync:** Add broader multi-group fault-injection scenarios for extreme divergence/recovery.
*   **CLI Integration:** Expand live-network matrix and platform variability checks.
*   **Known ignored test:** `crates/spex-bridge/tests/integration.rs` contains `tls_validation_checklist` as ignored checklist coverage.
