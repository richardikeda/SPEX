# Operations: Abuse, Revocation, and Recovery

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

This guide covers operational flows for heterogeneous environments, focusing on auditability and security.

## Abuse Log Export

Use this command to export bridge SQLite abuse events as stable JSON Lines.

```bash
cargo run -p spex-cli -- log export-abuse \
  --db-path <BRIDGE_DB_PATH> \
  --path <ABUSE_LOGS.jsonl> \
  --request-kind inbox \
  --outcome rejected \
  --since 1700000000 \
  --until 1700003600 \
  --limit 1000
```

Exported fields:
- timestamp
- identity_hash_hex (SHA-256 hash, no raw identity)
- ip_prefix (masked)
- request_kind
- outcome
- bytes

## Key Revocation with Audit Trail

1. Generate and register a recovery key:

```bash
cargo run -p spex-cli -- log create-recovery-key
```

2. Revoke compromised key with operational reason:

```bash
cargo run -p spex-cli -- log revoke-key --key-hex <HEX_KEY> --recovery-hex <RECOVERY_HEX> --reason "compromised"
```

Rules:
- revocation requires authenticated local identity
- target key must exist in checkpoint history
- operation is idempotent
- recovery-hex, when provided, must match valid non-expired recovery key entry

## Recovery for External Integrations

For cross-environment interoperability:

1. Export log from source environment:

```bash
cargo run -p spex-cli -- log export --path <CHECKPOINT_LOG.b64>
```

2. Import in destination environment:

```bash
cargo run -p spex-cli -- log import --path <CHECKPOINT_LOG.b64>
```

3. Verify consistency (gossip/prefix proof):

```bash
cargo run -p spex-cli -- log gossip-verify --path <CHECKPOINT_LOG.b64>
```
