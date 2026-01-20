# Bridge HTTP API

Esta documentação descreve os endpoints expostos pela bridge HTTP do SPEX e os requisitos de
validação para payloads, PoW e grants.

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

## GET /inbox/:key

Endpoint de fallback usado pelo transporte para inbox scanning via HTTP. O payload retorna uma
lista de envelopes CBOR base64.

```http
GET /inbox/<HEX_KEY> HTTP/1.1
```

```json
{ "items": ["<BASE64_ENVELOPE>", "<BASE64_ENVELOPE>"] }
```

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
- `params` (quando informado) deve respeitar o mínimo de memória/iterações aceito pelo servidor.
- O mínimo pode ser ajustado dinamicamente conforme reputação local e volume recente de requests.
- A verificação é feita com `spex-core` (CTAP2/PoW) e retorna `401` se inválida.

## Rate limiting e logs de abuso

- O rate limiting considera identidade (`grant.user_id`) e aplica limites de mensagens e bytes por
  janela.
- As tentativas são registradas com timestamp, IP de origem e `slot_id` (quando aplicável), além de
  resultado aceito/rejeitado para análise de abuso.
