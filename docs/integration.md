# Integração

Este guia cobre geração e validação de cartões, fluxo request/grant, criação de threads e envio de
mensagens. Inclui exemplos em Rust e princípios para outras linguagens.

## Cartões (ContactCard)

### Geração

Em Rust, use o `spex-core` para montar e serializar o card em CBOR canonical e depois codificar
em base64. O card pode ser assinado com Ed25519 para permitir validação de integridade.

```rust
// Pseudocódigo ilustrativo.
use spex_core::cards::ContactCard;

// Build de card a partir da identidade local.
let card = ContactCard::builder()
    .user_id(user_id)
    .verifying_key(public_key)
    .device_id(device_id)
    .issued_at(now)
    .build();

let cbor = card.to_cbor()?; // CBOR canonical
let card_b64 = base64::encode(cbor);
```

### Validação

Ao importar um card, valide:
- Consistência do CBOR canonical.
- Assinatura (quando presente).
- Continuidade do fingerprint da chave pública para contatos já conhecidos.

Esses passos evitam card spoofing e trocas silenciosas de chave.

## Request/grant

O fluxo básico é:

1. Receber `ContactCard` ou `InviteToken`.
2. Criar `RequestToken` (JSON base64) com PoW se exigido.
3. Validar request e emitir `GrantToken` assinado (CBOR canonical base64).
4. Usar o grant para criar `ThreadConfig` e iniciar a thread MLS.

Para integrações em Rust, o `spex-core` expõe validadores compartilhados de grant e PoW
(`validation::validate_grant_token` e `validation::validate_pow_puzzle`) que podem ser usados
antes de aceitar tokens recebidos via transporte P2P ou bridge HTTP.
O `spex-transport` também expõe validadores de ingestão P2P (`validate_p2p_grant_payload` e
`validate_p2p_puzzle_payload`) que reaproveitam a validação do core com payloads base64.

### Exemplo em Rust (request/grant)

```rust
// Pseudocódigo ilustrativo.
use spex_core::tokens::{RequestToken, GrantToken};

let request = RequestToken::builder()
    .user_id(requester_id)
    .role(role)
    .puzzle_solution(puzzle_solution)
    .build();

let request_b64 = base64::encode(request.to_json_bytes()?);

// Do lado do destinatário:
let grant = GrantToken::builder()
    .user_id(requester_id)
    .role(role)
    .expires_at(expires_at)
    .build();

let grant_cbor = grant.to_cbor()?; // CBOR canonical
let grant_b64 = base64::encode(grant_cbor);
```

## Threads e mensagens

### Criação de thread

O `GrantToken` alimenta o `ThreadConfig`. Em Rust:

```rust
// Pseudocódigo ilustrativo.
use spex_core::thread::ThreadConfig;

let config = ThreadConfig::builder()
    .thread_id(thread_id)
    .grants(vec![grant_token])
    .build();
```

### Envio de mensagens

Para envio, serialize `Envelope` e entregue via camada de transporte (P2P, bridge HTTP, etc.).

```rust
// Pseudocódigo ilustrativo.
use spex_core::envelope::Envelope;

let envelope = Envelope::builder()
    .thread_id(thread_id)
    .sender_user_id(sender_id)
    .ciphertext(ciphertext)
    .build();
```

## Integração em outras linguagens

Ao implementar fora de Rust, mantenha os seguintes princípios:

- **CBOR canonical/CTAP2**: use biblioteca que preserve a ordenação canônica e mapas com chaves
  inteiras conforme a especificação.
- **Base64**: cards/tokens são transportados em base64 para compatibilidade com canais de texto.
- **JSON**: `RequestToken` é JSON base64; respeite nomes de campos e tipos conforme `spex-core`.
- **Ed25519**: assine/verifique com bibliotecas maduras e hashing determinístico.
- **PoW mínimo**: se `requires_puzzle` estiver ativo, valide o puzzle antes de emitir grants.
- **TLS obrigatório**: use HTTPS quando integrar com bridge HTTP ou serviços externos.

## Persistência local

Se você implementar armazenamento local, use um arquivo equivalente ao `~/.spex/state.json` para
manter chaves e contatos, com permissões restritas e criptografia em repouso quando possível.
