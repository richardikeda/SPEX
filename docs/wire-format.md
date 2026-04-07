# Wire format (CBOR canonical/CTAP2)

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

This specification describes SPEX CBOR payloads.
All CBOR maps use integer keys and canonical CTAP2 serialization to preserve deterministic hashes/signatures.
Tables below document field IDs and CBOR types.

## Conventions

- CBOR types:
  - uint: unsigned integer
  - bytes: byte string
  - bool: true/false
  - map: CBOR map with integer keys
  - array: CBOR array
- Extensions: fields >= last reserved ID are extension-safe.
- Base64 examples use RFC 4648 encoding.

## ContactCard

| Field | ID (CBOR) | Type (CBOR) | Description |
| --- | --- | --- | --- |
| `user_id` | 0 | bytes | User identifier (32 bytes). |
| `verifying_key` | 1 | bytes | Ed25519 public key (32 bytes). |
| `device_id` | 2 | bytes | Device identifier. |
| `device_nonce` | 3 | bytes | Device nonce. |
| `issued_at` | 4 | uint | UNIX timestamp (seconds). |
| `invite` | 5 | map | Optional InviteToken. |
| `signature` | 6 | bytes | Optional Ed25519 signature. |
| `extensions` | >=7 | any | Custom extensions. |

Example values used in CBOR:

- `user_id`: `01..20` (32 bytes, hex `0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20`)
- `verifying_key`: `21..40` (32 bytes, hex `2122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f40`)
- `device_id`: `"device01"` (hex `6465766963653031`)
- `device_nonce`: `"nonce001"` (hex `6e6f6e6365303031`)
- `issued_at`: `1700000000`
- `invite`: `{0: 1, 1: 0, 2: true}`
- `signature`: `aa` repetido 64 vezes

**CBOR (hex):**

```
a70058200102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f200158202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f400248646576696365303103486e6f6e6365303031041a6553f10005a30001010002f5065840aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
```

**CBOR (base64):**

```
pwBYIAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8gAVggISIjJCUmJygpKissLS4vMDEyMzQ1Njc4OTo7PD0+P0ACSGRldmljZTAxA0hub25jZTAwMQQaZVPxAAWjAAEBAAL1BlhAqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqg==
```

## InviteToken

| Field | ID (CBOR) | Type (CBOR) | Description |
| --- | --- | --- | --- |
| `major` | 0 | uint | Protocol major version. |
| `minor` | 1 | uint | Protocol minor version. |
| `requires_puzzle` | 2 | bool | Whether PoW is required. |
| `extensions` | >=3 | any | Custom extensions. |

Example values used in CBOR: major=1, minor=0, requires_puzzle=true.

**CBOR (hex):**

```
a30001010002f5
```

**CBOR (base64):**

```
owABAQAC9Q==
```

## GrantToken

| Field | ID (CBOR) | Type (CBOR) | Description |
| --- | --- | --- | --- |
| `user_id` | 0 | bytes | User with permission. |
| `role` | 1 | uint | Role/access level. |
| `flags` | 2 | uint | Optional flags. |
| `expires_at` | 3 | uint | Optional expiration. |
| `extensions` | >=4 | any | Custom extensions. |

Example values used in CBOR:

- `user_id`: `01..20` (32 bytes)
- `role`: `1`
- `flags`: `0`
- `expires_at`: `1700003600`

**CBOR (hex):**

```
a40058200102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f2001010200031a6553ff10
```

**CBOR (base64):**

```
pABYIAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8gAQECAAMaZVP/EA==
```

## ThreadConfig

| Field | ID (CBOR) | Type (CBOR) | Description |
| --- | --- | --- | --- |
| `proto_major` | 0 | uint | Protocol major version. |
| `proto_minor` | 1 | uint | Protocol minor version. |
| `ciphersuite_id` | 2 | uint | Ciphersuite identifier. |
| `flags` | 3 | uint | Thread flags. |
| `thread_id` | 4 | bytes | Thread ID. |
| `grants` | 5 | array | GrantToken list. |
| `extensions` | >=6 | any | Custom extensions. |

Example values used in CBOR:

- `proto_major=1`, `proto_minor=0`, `ciphersuite_id=1`, `flags=0`
- `thread_id`: `41..60` (32 bytes)
- `grants`: array contendo o `GrantToken` do exemplo anterior

**CBOR (hex):**

```
a600010100020103000458204142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f600581a40058200102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f2001010200031a6553ff10
```

**CBOR (base64):**

```
pgABAQACAQMABFggQUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVpbXF1eX2AFgaQAWCABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fIAEBAgADGmVT/xA=
```

## Envelope

| Field | ID (CBOR) | Type (CBOR) | Description |
| --- | --- | --- | --- |
| `thread_id` | 0 | bytes | Thread ID. |
| `epoch` | 1 | uint | MLS epoch. |
| `seq` | 2 | uint | Envelope sequence number. |
| `sender_user_id` | 3 | bytes | Sender user ID. |
| `ciphertext` | 4 | bytes | Encrypted payload. |
| `signature` | 5 | bytes | Optional signature. |
| `extensions` | >=6 | any | Custom extensions. |

Example values used in CBOR:

- `thread_id`: `61..80` (32 bytes)
- `epoch`: `5`
- `seq`: `42`
- `sender_user_id`: `01..20` (32 bytes)
- `ciphertext`: `"ciphertext"` (hex `63697068657274657874`)
- `signature`: `bb` repetido 64 vezes

**CBOR (hex):**

```
a60058206162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f80010502182a0358200102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20044a63697068657274657874055840bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
```

**CBOR (base64):**

```
pgBYIGFiY2RlZmdoaWprbG1ub3BxcnN0dXZ3eHl6e3x9fn+AAQUCGCoDWCABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fIARKY2lwaGVydGV4dAVYQLu7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7u7s=
```
