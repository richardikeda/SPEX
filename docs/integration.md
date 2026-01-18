# Integração

Este guia cobre geração e validação de cartões, fluxo PoW/Grant, criação de threads e envio de
mensagens. Consulte também o README para exemplos completos e notas de segurança já existentes.

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

Esses passos evitam card spoofing e trocas silenciosas de chave. Veja o README para a descrição
completa do wire-format e o fluxo de fingerprint.

## PoW e Grant

Quando `requires_puzzle` estiver habilitado no `InviteToken`, o remetente deve resolver o PoW e
incluir a prova no request/grant conforme o formato esperado no `spex-core`. O fluxo é:

1. Receber `InviteToken` indicando PoW obrigatório.
2. Resolver o puzzle conforme parâmetros do token.
3. Enviar `RequestToken` com a prova.
4. Validar a prova e emitir o `GrantToken`.

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

- **CBOR canonical/CTAP2**: utilize uma biblioteca que suporte mapas com chaves inteiras e
  preserve ordenação canônica.
- **Base64**: tokens e cards são transportados em base64 para compatibilidade com canais de texto.
- **JSON**: `RequestToken` é JSON base64 (observe campos e tipos descritos no README).
- **Ed25519**: use bibliotecas maduras para assinatura e verificação; os hashes e nonce devem
  seguir as especificações do `spex-core`.

## Referências no README

Consulte no README:
- **Fluxo básico de handshake (request/grant)**.
- **Persistência local e fingerprints**.
- **Wire format (CBOR canonical/CTAP2)**.
- **Checkpoints, recovery e revogação de chaves**.

Essas seções consolidam exemplos e observações de segurança que devem ser respeitadas por todas
as integrações.
