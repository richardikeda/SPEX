# Architecture Overview

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

SPEX is organized in layers to preserve modularity and interoperability.
The crate split allows transport/runtime evolution without weakening wire-format or authentication guarantees.

## Main Layers

1. Core (types and formats)
   - Canonical types (cards, tokens, envelopes, checkpoints).
   - Canonical CBOR/CTAP2 for deterministic signatures and hashes.
2. Security and identity
   - Ed25519 signatures, hashing, PoW validation, grant validation.
   - Append-only checkpoint log with Merkle consistency checks.
3. MLS and message layer
   - Thread lifecycle, commits, group state safety, encrypted payload flow.
4. Transport
   - Chunking/manifests, DHT/Kademlia, gossip, inbox scanning.
   - HTTP bridge fallback for delivery/retrieval.
5. Tooling
   - CLI and SDK flows for request/grant/thread/message operations.

## Crate Map

- spex-core: protocol types, canonical CBOR, crypto and validation primitives.
- spex-mls: MLS integration, external-commit handling, epoch safety controls.
- spex-transport: P2P transport runtime, chunk/manifests, fallback/recovery helpers.
- spex-bridge: HTTP bridge with SQLite persistence and abuse controls.
- spex-cli: reference operational interface.
- spex-client: high-level SDK for application integrations.

```mermaid
graph TD
    CLI["🖥️ spex-cli\nReference CLI"]
    CLIENT["📦 spex-client\nHigh-level SDK"]
    MLS["🔐 spex-mls\nMLS / Epoch Safety"]
    TRANSPORT["🌐 spex-transport\nP2P · Chunking · Inbox"]
    BRIDGE["🌉 spex-bridge\nHTTP Relay · SQLite · Abuse Controls"]
    CORE["⚙️ spex-core\nTypes · CBOR/CTAP2 · Ed25519 · PoW · Hashing"]

    CLI --> CLIENT
    CLIENT --> MLS
    CLIENT --> TRANSPORT
    TRANSPORT --> BRIDGE
    MLS --> CORE
    TRANSPORT --> CORE
    BRIDGE --> CORE
    CLIENT --> CORE

    style CORE fill:#1e3a5f,color:#fff,stroke:#4a90d9
    style MLS fill:#1a4731,color:#fff,stroke:#52c97b
    style TRANSPORT fill:#3d2b00,color:#fff,stroke:#f5a623
    style BRIDGE fill:#3d1a00,color:#fff,stroke:#e8603c
    style CLIENT fill:#2d1b4e,color:#fff,stroke:#9b59b6
    style CLI fill:#1a1a2e,color:#fff,stroke:#aaa
```

## Handshake Flow (Request/Grant)

1. Initial contact: sender shares a ContactCard (CBOR base64) or an InviteToken.
2. Request: sender submits a RequestToken (JSON base64).
3. Grant: recipient validates request and issues signed GrantToken (canonical CBOR base64).
4. MLS thread: grant is used in ThreadConfig to bootstrap secure group communication.

This flow enforces explicit authorization before message exchange.

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

## MLS Thread Lifecycle

```mermaid
stateDiagram-v2
    [*] --> Pending : thread new\n(ThreadConfig built)

    Pending --> Bootstrapping : MLS external commit\n(members + GrantTokens)

    Bootstrapping --> Active : epoch 0 established\ngroup state valid

    Active --> EpochAdvance : commit (member add/remove\nor key rotation)
    EpochAdvance --> Active : epoch N+1 confirmed\ncheckpoint appended

    Active --> OutOfOrder : external commit\nwrong epoch received
    OutOfOrder --> Active : explicit recovery\n(deterministic, no silent mutation)

    Active --> KeyRevoked : log revoke-key\nor key compromise detected
    KeyRevoked --> Recovery : recovery key workflow
    Recovery --> Active : new epoch after re-keying

    Active --> Closed : thread terminated\nor all grants expired

    Closed --> [*]

    note right of Active
        Append-only checkpoint log
        Merkle consistency checks
        Gossip-verify available
    end note

    note right of OutOfOrder
        State does NOT mutate
        on invalid epoch path.
        Structured error returned.
    end note
```

## Local Persistence

CLI local state is stored in ~/.spex/state.json (or SPEX_STATE_PATH).
State is encrypted using OS keychain material, or SPEX_STATE_PASSPHRASE fallback.

## Fingerprints

When importing cards, verify public-key fingerprint continuity.
Unexpected key changes should be treated as critical events.

## Transport and TLS

All external integrations must use TLS.
Transport protection complements, but never replaces, protocol-level signature and context validation.
