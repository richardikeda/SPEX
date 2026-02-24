# Integração

Este guia cobre geração e validação de cartões, fluxo request/grant, criação de threads e envio de
mensagens. Inclui exemplos em Rust e princípios para outras linguagens.

## Cartões (ContactCard)

### Geração

Em Rust, use o `spex-core` para montar e serializar o card em CBOR canonical e depois codificar
em base64. O card pode ser assinado com Ed25519 para permitir validação de integridade.

```rust
use spex_core::types::{ContactCard, Ctap2Cbor, InviteToken};
use std::collections::BTreeMap;

let card = ContactCard {
    user_id: b"alice".to_vec(),
    verifying_key: vec![0x11; 32],
    device_id: b"alice-phone".to_vec(),
    device_nonce: vec![0x22; 16],
    issued_at: 1_735_000_000,
    invite: Some(InviteToken {
        major: 0,
        minor: 1,
        requires_puzzle: true,
        extensions: BTreeMap::new(),
    }),
    signature: None,
    extensions: BTreeMap::new(),
};

let cbor = card.to_ctap2_canonical_bytes()?;
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
// Pseudocódigo (RequestToken não está em `spex_core::types` atualmente).
// O fluxo abaixo mostra apenas a etapa compilável de GrantToken.
use spex_core::types::{Ctap2Cbor, GrantToken};
use std::collections::BTreeMap;

let grant = GrantToken {
    user_id: b"alice".to_vec(),
    role: 1,
    flags: Some(0),
    expires_at: Some(1_735_086_400),
    extensions: BTreeMap::new(),
};

let grant_cbor = grant.to_ctap2_canonical_bytes()?;
let grant_b64 = base64::encode(grant_cbor);
```

## Threads e mensagens

### Criação de thread

O `GrantToken` alimenta o `ThreadConfig`. Em Rust:

```rust
use spex_core::types::{GrantToken, ThreadConfig};
use std::collections::BTreeMap;

let grant = GrantToken {
    user_id: b"alice".to_vec(),
    role: 1,
    flags: None,
    expires_at: None,
    extensions: BTreeMap::new(),
};

let config = ThreadConfig {
    proto_major: 0,
    proto_minor: 1,
    ciphersuite_id: 1,
    flags: 0,
    thread_id: b"thread-01".to_vec(),
    grants: vec![grant],
    extensions: BTreeMap::new(),
};
```

### Envio de mensagens

Para envio, serialize `Envelope` e entregue via camada de transporte (P2P, bridge HTTP, etc.).

```rust
use spex_core::types::{Ctap2Cbor, Envelope};
use std::collections::BTreeMap;

let envelope = Envelope {
    thread_id: b"thread-01".to_vec(),
    epoch: 1,
    seq: 42,
    sender_user_id: b"alice".to_vec(),
    ciphertext: vec![0xAA, 0xBB, 0xCC],
    signature: None,
    extensions: BTreeMap::new(),
};

let envelope_cbor = envelope.to_ctap2_canonical_bytes()?;
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

## MLS callbacks and out-of-order policy

- `spex-client::apply_thread_commit_with_events` conecta commits MLS a callbacks consumíveis pela aplicação para eventos de `Rekey`, `MembershipUpdated` e `MembershipRemoved`.
- `spex-mls` fornece `process_external_commit_explicit`, `detect_external_commit_gap` e `process_external_commit_with_resync` para controle determinístico de commits externos.
- Commits fora de ordem retornam erro estruturado e não alteram estado local sem recuperação explícita.


## Perfis explícitos de tempo no P2P

O transporte libp2p agora expõe perfis explícitos com `P2pNodeConfig::for_profile(P2pRuntimeProfile::{Dev, Test, Prod})` para definir `publish_wait`, `query_timeout` e `manifest_wait` de forma determinística por ambiente.

Operações de publish/query/recovery usam backoff adaptativo com jitter, tuning por conectividade e instrumentação de métricas (contadores de sucesso/timeout/retries e histogramas de latência).

A reputação operacional agora usa três estágios: saudável, probation (redução de influência sem ban imediato) e ban temporário para abuso recorrente.

Snapshots persistidos incluem reputação de peers para continuidade durante churn e, em caso de corrupção, o arquivo é movido para quarentena e reinicializado com estado vazio seguro.


### Publicação de inbox via bridge (cliente/transporte)

O fluxo recomendado para publicação HTTP usa:

- `spex_transport::inbox::build_bridge_publish_request` para serializar envelope (`CTAP2/CBOR`), assinar grant e calcular PoW.
- `spex_transport::inbox::BridgeClient::publish_to_inbox` para `PUT /inbox/:key`.
- `spex_client::publish_via_bridge` como API de alto nível consumida pela CLI.

Contrato de integração:

- Método: `PUT /inbox/:hex_sha256(inbox_key_seed)`
- Header: `Content-Type: application/json`
- Campos obrigatórios: `data`, `grant`, `puzzle`
- Campo opcional: `ttl_seconds`

Erros esperados no cliente/transporte:

- `TransportError::GrantInvalid` para grant inválido.
- `TransportError::PowInvalid` para PoW inválido/insuficiente.
- `TransportError::InvalidTtl` para TTL fora da política.
