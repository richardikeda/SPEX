# Integration

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

This guide covers card validation, request/grant flow, thread creation, message exchange, and API-oriented integration patterns.

## Contact Cards

### Generation

Build cards in canonical CBOR (CTAP2 profile), then encode as base64 for transport.

### Validation

Validate on import:

- canonical encoding
- signature (when present)
- fingerprint continuity for known identities

## Request and Grant

Reference flow:

1. Receive ContactCard/InviteToken.
2. Build RequestToken with PoW when required.
3. Validate request and issue signed GrantToken.
4. Create ThreadConfig and start MLS-protected flow.

Use shared validation primitives from core/transport before accepting untrusted payloads.

```mermaid
sequenceDiagram
    participant A as Alice (Sender)
    participant B as Bob (Recipient)
    participant T as Transport / Bridge

    Note over A,B: Step 1 — Identity & Card

    A->>A: identity new
    A->>A: card create → ContactCard (CBOR base64)
    A-->>B: share ContactCard (out-of-band or via bridge)

    Note over A,B: Step 2 — Request

    B->>B: card redeem --card <BASE64>
    B->>B: Validate: signature · encoding · fingerprint
    B->>B: Build RequestToken + PoW (if requires_puzzle)
    B->>T: request send --to <USER_ID>

    Note over A,B: Step 3 — Grant

    A->>A: Validate RequestToken
    A->>A: Check PoW (memory ≥ 64 MiB, iterations ≥ 3)
    A->>A: grant accept → GrantToken (signed CBOR)
    A-->>B: GrantToken

    Note over A,B: Step 4 — Thread Bootstrap

    B->>B: Build ThreadConfig (GrantToken list)
    B->>B: MLS group init via spex-mls
    B-->>A: Thread ready ✓

    Note over A,B: Explicit authorization complete — messages may now flow
```

## Thread and Message Flow

- Build ThreadConfig from validated grants.
- Serialize envelope in canonical CBOR.
- Publish through P2P, bridge, or hybrid strategy.

```mermaid
flowchart TD
    START([Sender has valid GrantToken]) --> THREAD

    THREAD["thread new --members USER_IDs\n→ ThreadConfig + MLS group init"]
    THREAD --> COMPOSE

    COMPOSE["msg send --thread THREAD_ID --text '...' \n→ Build Envelope CBOR\n  thread_id · epoch · seq · sender_user_id\n  ciphertext · signature"]

    COMPOSE --> ROUTE{Transport\nStrategy}

    ROUTE -->|"--bridge-url"| BRIDGE_PATH
    ROUTE -->|"--p2p"| P2P_PATH
    ROUTE -->|hybrid| BOTH

    subgraph BRIDGE_PATH ["🌉 Bridge Path"]
        B1["Attach GrantToken + PoW puzzle"]
        B2["PUT /inbox/:key  (TLS required)"]
        B3["Bridge: validate grant + PoW\nStore envelope (TTL ≤ 604800s)"]
        B1 --> B2 --> B3
    end

    subgraph P2P_PATH ["🌐 P2P Path"]
        P1["Chunk envelope → Manifest"]
        P2["DHT / Kademlia publish"]
        P3["Gossip propagation"]
        P1 --> P2 --> P3
    end

    subgraph BOTH ["♻️ Hybrid"]
        H1["Attempt P2P first"]
        H2["Fallback to bridge on failure"]
        H1 --> H2
    end

    B3 --> INBOX
    P3 --> INBOX
    H2 --> INBOX

    INBOX["📥 Recipient: inbox poll\n→ Decrypt · Verify signature · Apply MLS epoch"]
    INBOX --> DONE([Message delivered ✓])

    style BRIDGE_PATH fill:#3d1a00,color:#fff,stroke:#e8603c
    style P2P_PATH fill:#3d2b00,color:#fff,stroke:#f5a623
    style BOTH fill:#1a1a2e,color:#fff,stroke:#aaa
```

## Non-Rust Integration Requirements

- canonical CBOR/CTAP2 encoding only
- deterministic signatures/hashes
- strict schema and type validation
- base64 transport encoding for binary payloads
- TLS for bridge/external APIs

## MLS Out-of-Order and Recovery Policy

- Out-of-order external commits must return explicit structured errors.
- Local state must not mutate on invalid order/epoch paths.
- Recovery must be explicit and deterministic.

## Advanced MLS Compliance References

See:

- `docs/mls-advanced-scenarios-matrix.md`
- `crates/spex-mls/tests/planned_concurrent_updates.rs`
- `crates/spex-mls/tests/mls_advanced_negative.rs`

## Bridge Publish Contract (Inbox)

Recommended flow:

- `spex_transport::inbox::build_bridge_publish_request`
- `spex_transport::inbox::BridgeClient::publish_to_inbox`
- `spex_client::publish_via_bridge`

Contract summary:

- method: `PUT /inbox/:hex_sha256(inbox_key_seed)`
- required fields: `data`, `grant`, `puzzle`
- optional field: `ttl_seconds`

Expected error classes:

- invalid grant
- invalid/insufficient PoW
- invalid TTL policy
