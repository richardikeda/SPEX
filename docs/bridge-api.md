# Bridge HTTP API

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

This document defines current SPEX bridge HTTP endpoints and validation contracts.

## Endpoint Summary

| Method | Path | Description |
| --- | --- | --- |
| PUT | /cards/:card_hash | Store ContactCard payload (base64 canonical CBOR). |
| GET | /cards/:card_hash | Fetch ContactCard by hash. |
| PUT | /slot/:slot_id | Store generic payload blob by hash id. |
| GET | /slot/:slot_id | Fetch stored blob. |
| PUT | /inbox/:key | Store inbox envelope payload. |
| GET | /inbox/:key | List inbox envelopes. |

## Conventions

- Binary payload fields use RFC 4648 base64.
- Signed structures use canonical CBOR/CTAP2.
- Grant and PoW validation are mandatory at ingress.
- Rate limiting and abuse logs are part of bridge policy.

## PUT /cards/:card_hash

Requirements:

- valid, non-expired grant
- valid PoW puzzle
- card_hash must match request data hash

Status codes:

- 204 No Content
- 400 Bad Request
- 401 Unauthorized
- 429 Too Many Requests
- 500 Internal Server Error

## GET /cards/:card_hash

Status codes:

- 200 OK
- 404 Not Found
- 500 Internal Server Error

## PUT /slot/:slot_id

Requirements:

- valid, non-expired grant
- valid PoW puzzle

Status codes:

- 204 No Content
- 400 Bad Request
- 401 Unauthorized
- 429 Too Many Requests
- 500 Internal Server Error

## GET /slot/:slot_id

Status codes:

- 200 OK
- 404 Not Found
- 500 Internal Server Error

## PUT /inbox/:key

Requirements:

- valid, non-expired grant
- valid PoW puzzle
- ttl_seconds within policy (default 86400, max 604800)
- payload max size 262144 bytes

Status codes:

- 204 No Content
- 400 Bad Request
- 401 Unauthorized
- 429 Too Many Requests
- 500 Internal Server Error

## GET /inbox/:key

Response shape:

```json
{
  "items": ["<BASE64_ENVELOPE>", "<BASE64_ENVELOPE>"],
  "next_cursor": 42
}
```

Query params:

- `limit`
- `cursor`
- `max_bytes`

Expired items are omitted.

Recommended status codes:

- 200 OK
- 404 Not Found
- 500 Internal Server Error

## Validation Rules

### Grant validation

- base64 decoding checks
- signature checks
- expiration checks
- role/flags type checks

### Puzzle validation

- required fields must decode correctly
- minimum policy: memory >= 64 MiB, iterations >= 3
- invalid puzzle returns 401

Every payload entering the bridge passes through the following mandatory pipeline:

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

## Rate Limiting and Abuse Logging

- identity-scoped quotas per time window
- event recording for denied/accepted outcomes

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

## Client/Transport Bridge Contract

Reference integration flow:

- `spex_transport::inbox::build_bridge_publish_request`
- `spex_transport::inbox::BridgeClient::publish_to_inbox`
- `spex_client::publish_via_bridge`

Expected error classes for clients:

- invalid grant
- invalid/insufficient PoW
- invalid TTL
