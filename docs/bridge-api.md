# Bridge HTTP API

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

Esta documentação descreve os endpoints expostos pela bridge HTTP do SPEX e os requisitos de
validação para payloads, PoW e grants.

## Status de implementação (alinhamento)

- `PUT /inbox/:key` e `GET /inbox/:key` estão implementados na bridge e documentados como endpoints atuais.
- A integração MLS e o runtime P2P não são responsabilidades diretas da bridge; essas pendências ficam no roadmap do cliente/transporte em `README.md` e `TODO.md`.

## Convenções

- **Base64**: todos os campos binários são transportados em base64 padrão (RFC 4648).
- **CBOR**: cards e envelopes são CBOR canonical codificados em base64.
- **Grant**: o servidor valida expiração, assinatura e formato do `GrantToken` recebido.
- **Puzzle (PoW)**: o servidor valida a saída do puzzle conforme `spex-core`.
- **Rate limiting**: o servidor aplica limites por identidade para mensagens e bytes por janela.
- **Auditoria**: o servidor persiste logs com timestamp, IP e slot para análise de abuso.

## Resumo de endpoints

| Método | Caminho | Descrição |
| --- | --- | --- |
| `PUT` | `/cards/:card_hash` | Armazena um `ContactCard` (CBOR base64). |
| `GET` | `/cards/:card_hash` | Recupera um `ContactCard` por hash. |
| `PUT` | `/slot/:slot_id` | Armazena blob genérico por hash. |
| `GET` | `/slot/:slot_id` | Recupera blob armazenado. |
| `PUT` | `/inbox/:key` | Armazena envelope para inbox scanning. |
| `GET` | `/inbox/:key` | Lista envelopes para inbox scanning. |

## PUT /cards/:card_hash

Armazena um `ContactCard` (CBOR base64). O `card_hash` deve ser o SHA-256 hex do CBOR bruto.

**Requisitos**

- `grant` válido e não expirado.
- `puzzle` válido para o `recipient_key` informado.
- `card_hash` precisa corresponder ao hash do `data`.

**Request**

```http
PUT /cards/<SHA256_HEX> HTTP/1.1
Content-Type: application/json
```

```json
{
  "data": "<BASE64_CBOR_CARD>",
  "grant": {
    "user_id": "<BASE64_USER_ID>",
    "role": 1,
    "flags": 0,
    "expires_at": 1700003600,
    "verifying_key": "<BASE64_ED25519_PUBLIC_KEY>",
    "signature": "<BASE64_ED25519_SIGNATURE>"
  },
  "puzzle": {
    "recipient_key": "<BASE64>",
    "puzzle_input": "<BASE64>",
    "puzzle_output": "<BASE64>",
    "params": {
      "memory_kib": 4096,
      "iterations": 3,
      "parallelism": 1,
      "output_len": 32
    }
  }
}
```

**Status codes**

- `204 No Content`: armazenamento concluído.
- `400 Bad Request`: payload inválido ou hash divergente.
- `401 Unauthorized`: puzzle inválido ou grant expirado.
- `429 Too Many Requests`: limites de mensagens ou bytes excedidos.
- `500 Internal Server Error`: falha de armazenamento.

## GET /cards/:card_hash

Recupera o `ContactCard` armazenado pelo hash.

```http
GET /cards/<SHA256_HEX> HTTP/1.1
```

```json
{ "data": "<BASE64_CBOR_CARD>" }
```

**Status codes**

- `200 OK`: payload encontrado.
- `404 Not Found`: hash não existe.
- `500 Internal Server Error`: falha de armazenamento.

## PUT /slot/:slot_id

Armazena um blob genérico (por exemplo, payloads de handshake) identificado por `slot_id`.
O `slot_id` deve ser o SHA-256 hex do blob armazenado.

**Requisitos**

- `grant` válido e não expirado.
- `puzzle` válido para o `recipient_key` informado.

**Request**

```http
PUT /slot/<SHA256_HEX> HTTP/1.1
Content-Type: application/json
```

```json
{
  "data": "<BASE64_BLOB>",
  "grant": {
    "user_id": "<BASE64_USER_ID>",
    "role": 1,
    "flags": 0,
    "expires_at": 1700003600,
    "verifying_key": "<BASE64_ED25519_PUBLIC_KEY>",
    "signature": "<BASE64_ED25519_SIGNATURE>"
  },
  "puzzle": {
    "recipient_key": "<BASE64>",
    "puzzle_input": "<BASE64>",
    "puzzle_output": "<BASE64>",
    "params": {
      "memory_kib": 4096,
      "iterations": 3,
      "parallelism": 1,
      "output_len": 32
    }
  }
}
```

**Status codes**

- `204 No Content`: armazenamento concluído.
- `400 Bad Request`: payload inválido.
- `401 Unauthorized`: puzzle inválido ou grant expirado.
- `429 Too Many Requests`: limites de mensagens ou bytes excedidos.
- `500 Internal Server Error`: falha de armazenamento.

## GET /slot/:slot_id

Recupera o blob armazenado pelo `slot_id`.

```http
GET /slot/<SHA256_HEX> HTTP/1.1
```

```json
{ "data": "<BASE64_BLOB>" }
```

**Status codes**

- `200 OK`: payload encontrado.
- `404 Not Found`: slot não existe.
- `500 Internal Server Error`: falha de armazenamento.

## Contrato cliente/transporte → bridge (inbox publish)

Fluxo de referência no código:

- `spex_transport::inbox::build_bridge_publish_request`: serializa envelope + grant + PoW de forma determinística.
- `spex_transport::inbox::BridgeClient::publish_to_inbox`: envia `PUT /inbox/:key` e mapeia erros HTTP da bridge.
- `spex_client::publish_via_bridge`: API de alto nível usada pela CLI para publicar mensagens em inbox remota.

Mapeamento de erros para integração:

- `401` com `grant signature invalid` → `TransportError::GrantInvalid`
- `401` com `puzzle validation failed` → `TransportError::PowInvalid`
- `400` com `invalid inbox ttl` → `TransportError::InvalidTtl`

## PUT /inbox/:key

Armazena um envelope (CBOR base64) associado ao `inbox_key`. O payload segue o mesmo formato de
`/cards` e `/slot`, com um campo adicional para definir expiração.

**Requisitos**

- `grant` válido e não expirado.
- `puzzle` válido para o `recipient_key` informado.
- `ttl_seconds` deve estar entre 1s e 604.800s (padrão 86.400s).
- `data` deve ter no máximo 262.144 bytes.

**Request**

```http
PUT /inbox/<HEX_KEY> HTTP/1.1
Content-Type: application/json
```

```json
{
  "data": "<BASE64_ENVELOPE>",
  "grant": {
    "user_id": "<BASE64_USER_ID>",
    "role": 1,
    "flags": 0,
    "expires_at": 1700003600,
    "verifying_key": "<BASE64_ED25519_PUBLIC_KEY>",
    "signature": "<BASE64_ED25519_SIGNATURE>"
  },
  "puzzle": {
    "recipient_key": "<BASE64>",
    "puzzle_input": "<BASE64>",
    "puzzle_output": "<BASE64>",
    "params": {
      "memory_kib": 4096,
      "iterations": 3,
      "parallelism": 1,
      "output_len": 32
    }
  },
  "ttl_seconds": 3600
}
```

**Status codes**

- `204 No Content`: armazenamento concluído.
- `400 Bad Request`: payload inválido.
- `401 Unauthorized`: puzzle inválido ou grant expirado.
- `429 Too Many Requests`: limites de mensagens ou bytes excedidos.
- `500 Internal Server Error`: falha de armazenamento.

## GET /inbox/:key

Endpoint de fallback usado pelo transporte para inbox scanning via HTTP. O payload retorna uma
lista de envelopes CBOR base64 com paginação e filtragem de expiração.

```http
GET /inbox/<HEX_KEY> HTTP/1.1
```

```json
{
  "items": ["<BASE64_ENVELOPE>", "<BASE64_ENVELOPE>"],
  "next_cursor": 42
}
```

**Query params**

- `limit` (opcional): máximo de itens por página (padrão 100, máximo 500).
- `cursor` (opcional): retorna itens com `id` maior que o cursor informado.
- `max_bytes` (opcional): limite total de bytes retornados por página.

Itens com `expires_at` no passado são omitidos.

**Status codes (recomendados)**

- `200 OK`: retorna um array (pode estar vazio).
- `404 Not Found`: inbox ainda não existe.
- `500 Internal Server Error`: falha no backend de inbox.

## Validação de grant

- `grant.user_id` deve ser base64 válido.
- `grant.expires_at` é opcional; se presente precisa ser maior que o timestamp atual.
- `grant.role` e `grant.flags` são validados como inteiros.
- `grant.verifying_key` e `grant.signature` devem ser base64 válidos e formam uma assinatura
  Ed25519 do hash CTAP2 canonical do `GrantToken`.

## Validação de puzzle (PoW)

- Os campos `recipient_key`, `puzzle_input` e `puzzle_output` devem ser base64 válidos.
- `params` é opcional; caso omitido, o servidor usa parâmetros padrão.
- `params` (quando informado) deve respeitar o mínimo de memória/iterações aceito pelo servidor
  (memória ≥64 MiB, iterações ≥3).
- O mínimo pode ser ajustado dinamicamente conforme reputação local e volume recente de requests.
- A verificação é feita com `spex-core` (CTAP2/PoW) e retorna `401` se inválida.

## Rate limiting e logs de abuso

- O rate limiting considera identidade (`grant.user_id`) e aplica limites de mensagens e bytes por
  janela.
- As tentativas são registradas com timestamp, IP de origem e `slot_id` (quando aplicável), além de
  resultado aceito/rejeitado para análise de abuso.
