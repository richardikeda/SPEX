# SPEX — Flows & Workflows

Visual reference for the SPEX protocol flows. Use these diagrams alongside [docs/overview.md](overview.md) and [docs/integration.md](integration.md).

---

## 1. System Architecture

Crate layering and dependency directions.

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

---

## 2. Handshake Flow — Contact → Request → Grant

Explicit permission exchange before any message can be sent.

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

---

## 3. Message Send Flow

Thread creation and message delivery with transport selection.

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

---

## 4. Bridge Validation Pipeline

Every payload entering the bridge passes through mandatory checks.

```mermaid
flowchart LR
    REQ(["Client\nPUT /inbox/:key"]) --> D1

    D1{"Decode\nbase64 fields"}
    D1 -->|invalid| ERR400["400 Bad Request"]
    D1 -->|ok| D2

    D2{"Grant\nvalidation"}
    D2 -->|expired / bad sig| ERR401a["401 Unauthorized\n(invalid grant)"]
    D2 -->|ok| D3

    D3{"PoW\nvalidation\nmemory ≥ 64 MiB\niter ≥ 3"}
    D3 -->|insufficient| ERR401b["401 Unauthorized\n(weak PoW)"]
    D3 -->|ok| D4

    D4{"TTL\npolicy\n≤ 604800s"}
    D4 -->|out of range| ERR400b["400 Bad Request\n(invalid TTL)"]
    D4 -->|ok| D5

    D5{"Payload size\n≤ 262144 bytes"}
    D5 -->|too large| ERR400c["400 Bad Request"]
    D5 -->|ok| D6

    D6{"Rate limit\nquota"}
    D6 -->|exceeded| ERR429["429 Too Many Requests"]
    D6 -->|ok| STORE

    STORE["Store envelope\nSQLite (TTL-aware)"]
    STORE --> OK["204 No Content ✓"]

    style ERR400 fill:#7f1d1d,color:#fff,stroke:#ef4444
    style ERR400b fill:#7f1d1d,color:#fff,stroke:#ef4444
    style ERR400c fill:#7f1d1d,color:#fff,stroke:#ef4444
    style ERR401a fill:#78350f,color:#fff,stroke:#f59e0b
    style ERR401b fill:#78350f,color:#fff,stroke:#f59e0b
    style ERR429 fill:#4c1d95,color:#fff,stroke:#a78bfa
    style OK fill:#14532d,color:#fff,stroke:#22c55e
```

---

## 5. MLS Thread Lifecycle

Epoch progression and safety controls in `spex-mls`.

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

---

## 6. Local State & Key Management (CLI)

How identity, cards, and log state are managed locally.

```mermaid
flowchart TD
    STATE["~/.spex/state.json\n(SPEX_STATE_PATH)\nEncrypted at rest\nOS keychain or SPEX_STATE_PASSPHRASE"]

    STATE --> ID["🪪 Identity\nEd25519 keypair\nuser_id · device_id"]
    STATE --> CARDS["📋 Known Cards\nContactCard index\nFingerprint continuity checks"]
    STATE --> GRANTS["🎫 Grants Issued/Received\nRole · Expiry · Flags"]
    STATE --> THREADS["🧵 Thread State\nMLS group state\nEpoch history"]
    STATE --> LOG["📜 Checkpoint Log\nAppend-only\nMerkle consistency\nRevocation records"]

    LOG --> GOSSIP["log gossip-verify\nVerify log integrity\nwith peers"]
    LOG --> EXPORT["log export\nAudit / backup"]
    LOG --> RECOVERY["log create-recovery-key\nlog import (recovery path)"]

    ID -->|"key compromise"| REVOKE["log revoke-key\n--key-hex --reason"]
    REVOKE --> LOG

    style STATE fill:#1e3a5f,color:#fff,stroke:#4a90d9
    style LOG fill:#1a4731,color:#fff,stroke:#52c97b
    style REVOKE fill:#7f1d1d,color:#fff,stroke:#ef4444
```

---

## 7. Anti-Spam & Abuse Controls

Economic friction model that makes message initiation costly.

```mermaid
flowchart LR
    SENDER(["Sender"])

    SENDER --> POW["Compute PoW Puzzle\nArgon2-style\nmemory ≥ 64 MiB\niterations ≥ 3"]
    POW --> GRANT["Attach signed\nGrantToken\n(role · expiry · flags)"]
    GRANT --> BRIDGE_CHECK{"Bridge\nIngress\nValidation"}

    BRIDGE_CHECK -->|"PoW too weak"| DENIED["Request Denied\n401"]
    BRIDGE_CHECK -->|"Grant expired/invalid"| DENIED
    BRIDGE_CHECK -->|"Rate limit exceeded"| THROTTLED["Throttled\n429"]
    BRIDGE_CHECK -->|"All checks pass"| ACCEPTED["Accepted\n204 ✓"]

    ACCEPTED --> ABUSE_LOG["Abuse event log\n(identity-scoped)"]
    DENIED --> ABUSE_LOG
    THROTTLED --> ABUSE_LOG

    subgraph ASYMMETRY ["⚖️ Asymmetric Cost Model"]
        COST_SEND["Sending: O(PoW compute)"]
        COST_RECV["Receiving: O(verify)  ← cheap"]
        COST_SEND -.->|"much more expensive"| COST_RECV
    end

    style DENIED fill:#7f1d1d,color:#fff,stroke:#ef4444
    style THROTTLED fill:#4c1d95,color:#fff,stroke:#a78bfa
    style ACCEPTED fill:#14532d,color:#fff,stroke:#22c55e
    style ASYMMETRY fill:#1a1a2e,color:#fff,stroke:#555
```
