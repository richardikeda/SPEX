# Wire format (CBOR canonical/CTAP2)

Esta especificação descreve os payloads CBOR usados no SPEX. Todos os mapas CBOR usam chaves
inteiras e **serialização canonical (CTAP2)** para garantir ordenação determinística e assinaturas
estáveis. As tabelas abaixo documentam os **IDs** e **tipos CBOR** por campo.

## Convenções

- **Tipos CBOR**:
  - `uint`: inteiro sem sinal.
  - `bytes`: sequência de bytes.
  - `bool`: `true`/`false`.
  - `map`: mapa CBOR com chaves inteiras.
  - `array`: array CBOR.
- **Extensões**: campos `>=` do último ID reservado são livres para extensões customizadas.
- **Base64**: os exemplos usam base64 padrão (RFC 4648) de bytes CBOR.

## ContactCard

| Campo | ID (CBOR) | Tipo (CBOR) | Descrição |
| --- | --- | --- | --- |
| `user_id` | 0 | bytes | Identificador do usuário (32 bytes). |
| `verifying_key` | 1 | bytes | Chave pública Ed25519 (32 bytes). |
| `device_id` | 2 | bytes | Identificador do dispositivo. |
| `device_nonce` | 3 | bytes | Nonce do dispositivo. |
| `issued_at` | 4 | uint | Timestamp UNIX (segundos). |
| `invite` | 5 | map | `InviteToken` opcional. |
| `signature` | 6 | bytes | Assinatura Ed25519 (opcional). |
| `extensions` | >=7 | any | Extensões customizadas. |

**Exemplo (valores usados no CBOR):**

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

| Campo | ID (CBOR) | Tipo (CBOR) | Descrição |
| --- | --- | --- | --- |
| `major` | 0 | uint | Versão major do protocolo. |
| `minor` | 1 | uint | Versão minor do protocolo. |
| `requires_puzzle` | 2 | bool | Indica se PoW é obrigatório. |
| `extensions` | >=3 | any | Extensões customizadas. |

**Exemplo (valores usados no CBOR):** `major=1`, `minor=0`, `requires_puzzle=true`.

**CBOR (hex):**

```
a30001010002f5
```

**CBOR (base64):**

```
owABAQAC9Q==
```

## GrantToken

| Campo | ID (CBOR) | Tipo (CBOR) | Descrição |
| --- | --- | --- | --- |
| `user_id` | 0 | bytes | Usuário com permissão. |
| `role` | 1 | uint | Papel/nível de acesso. |
| `flags` | 2 | uint | Flags opcionais. |
| `expires_at` | 3 | uint | Expiração (opcional). |
| `extensions` | >=4 | any | Extensões customizadas. |

**Exemplo (valores usados no CBOR):**

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

| Campo | ID (CBOR) | Tipo (CBOR) | Descrição |
| --- | --- | --- | --- |
| `proto_major` | 0 | uint | Versão major. |
| `proto_minor` | 1 | uint | Versão minor. |
| `ciphersuite_id` | 2 | uint | Identificador da suíte. |
| `flags` | 3 | uint | Flags da thread. |
| `thread_id` | 4 | bytes | ID da thread. |
| `grants` | 5 | array | Lista de `GrantToken`. |
| `extensions` | >=6 | any | Extensões customizadas. |

**Exemplo (valores usados no CBOR):**

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

| Campo | ID (CBOR) | Tipo (CBOR) | Descrição |
| --- | --- | --- | --- |
| `thread_id` | 0 | bytes | ID da thread. |
| `epoch` | 1 | uint | Epoch MLS. |
| `seq` | 2 | uint | Sequência do envelope. |
| `sender_user_id` | 3 | bytes | ID do remetente. |
| `ciphertext` | 4 | bytes | Payload cifrado. |
| `signature` | 5 | bytes | Assinatura opcional. |
| `extensions` | >=6 | any | Extensões customizadas. |

**Exemplo (valores usados no CBOR):**

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
