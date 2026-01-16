# SPEX

## Visão geral

**Objetivo**: fornecer uma base aberta e modular para mensagens seguras e interoperáveis, com foco em
criptografia de ponta a ponta, auditabilidade e integridade dos dados.

**Escopo**:
- Protocolos e formatos de mensagens.
- Camadas de transporte e integração com outros componentes.
- Ferramentas e bibliotecas para desenvolvimento e testes.

**Princípios**:
- **Segurança por padrão** (criptografia, autenticidade e integridade).
- **Interoperabilidade** (interfaces claras e camadas desacopladas).
- **Simplicidade e rastreabilidade** (implementações pequenas, testes claros e documentação objetiva).

## Estado atual

Implementação inicial em andamento com os seguintes componentes:
- **spex-core**: tipos, CBOR canonical (CTAP2), hashes, assinatura e provas de trabalho.
- **spex-mls**: estruturas mínimas para contexto MLS + extensões SPEX.
- **spex-transport**: chunking, publicação DHT/Kademlia, gossip e fallback via bridge HTTP.
- **spex-bridge**: bridge HTTP com armazenamento simples (cards/slots) e validações básicas.
- **spex-cli**: CLI de referência para identidades, cartões e fluxo básico de pedidos/grants.

## Build e uso

### Build

```bash
cargo build
```

### Testes

```bash
cargo test
```

### Uso rápido (CLI)

```bash
# gerar identidade local
cargo run -p spex-cli -- identity new

# criar cartão de contato (CBOR base64)
cargo run -p spex-cli -- card create

# aceitar cartão e salvar contato (valida assinatura se presente)
cargo run -p spex-cli -- card redeem --card <BASE64>

# enviar pedido de grant
cargo run -p spex-cli -- request send --to <USER_ID_HEX> --role 1

# aceitar grant recebido (token base64 do request)
cargo run -p spex-cli -- grant accept --request <BASE64>
```

## Wire format (CBOR canonical/CTAP2)

Todos os payloads CBOR usam mapas com chaves inteiras e serialização CTAP2
para garantir canonicalização e assinaturas determinísticas.

### ContactCard

| Campo | ID | Tipo | Descrição |
| --- | --- | --- | --- |
| `user_id` | 0 | bytes | Identificador do usuário (32 bytes). |
| `verifying_key` | 1 | bytes | Chave Ed25519 pública (32 bytes). |
| `device_id` | 2 | bytes | Identificador do dispositivo. |
| `device_nonce` | 3 | bytes | Nonce do dispositivo. |
| `issued_at` | 4 | uint | Timestamp UNIX (segundos). |
| `invite` | 5 | map | `InviteToken` opcional. |
| `signature` | 6 | bytes | Assinatura Ed25519 do card (opcional). |
| `extensions` | >=7 | any | Extensões customizadas. |

### InviteToken

| Campo | ID | Tipo | Descrição |
| --- | --- | --- | --- |
| `major` | 0 | uint | Versão major do protocolo. |
| `minor` | 1 | uint | Versão minor do protocolo. |
| `requires_puzzle` | 2 | bool | Indica se PoW é obrigatório. |
| `extensions` | >=3 | any | Extensões customizadas. |

### GrantToken

| Campo | ID | Tipo | Descrição |
| --- | --- | --- | --- |
| `user_id` | 0 | bytes | Usuário com permissão. |
| `role` | 1 | uint | Papel/nível de acesso. |
| `flags` | 2 | uint | Flags opcionais. |
| `expires_at` | 3 | uint | Expiração (opcional). |
| `extensions` | >=4 | any | Extensões customizadas. |

### ThreadConfig

| Campo | ID | Tipo | Descrição |
| --- | --- | --- | --- |
| `proto_major` | 0 | uint | Versão major. |
| `proto_minor` | 1 | uint | Versão minor. |
| `ciphersuite_id` | 2 | uint | Identificador da suíte. |
| `flags` | 3 | uint | Flags da thread. |
| `thread_id` | 4 | bytes | ID da thread. |
| `grants` | 5 | array | Lista de `GrantToken`. |
| `extensions` | >=6 | any | Extensões customizadas. |

### Envelope

| Campo | ID | Tipo | Descrição |
| --- | --- | --- | --- |
| `thread_id` | 0 | bytes | ID da thread. |
| `epoch` | 1 | uint | Epoch MLS. |
| `seq` | 2 | uint | Sequência do envelope. |
| `sender_user_id` | 3 | bytes | ID do remetente. |
| `ciphertext` | 4 | bytes | Payload cifrado. |
| `signature` | 5 | bytes | Assinatura opcional. |
| `extensions` | >=6 | any | Extensões customizadas. |

## Extensões MLS (SPEX)

As extensões MLS usam a faixa privada (0xF0A0/0xF0A1). O wire-format usa:

```
extension_type(u16) || extension_length(u16) || extension_data
```

### ext_spex_proto_suite (0xF0A0)

```
extension_data = major(u16) || minor(u16) || ciphersuite_id(u16) || flags(u8)
```

### ext_spex_cfg_hash (0xF0A1)

```
extension_data = hash_id(u16) || len(u8) || cfg_hash(len bytes)
```

## HTTP (bridge)

### PUT/GET /cards/:card_hash

```http
PUT /cards/<SHA256_HEX> HTTP/1.1
Content-Type: application/json

{
  "data": "<BASE64_CBOR_CARD>",
  "grant": {
    "user_id": "<BASE64_USER_ID>",
    "role": 1,
    "flags": null,
    "expires_at": 1700000200
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

```http
GET /cards/<SHA256_HEX> HTTP/1.1
```

```json
{ "data": "<BASE64_CBOR_CARD>" }
```

### PUT/GET /slot/:slot_id

```http
PUT /slot/<SLOT_ID> HTTP/1.1
Content-Type: application/json

{ "...mesmo payload do /cards..." }
```

```http
GET /slot/<SLOT_ID> HTTP/1.1
```

```json
{ "data": "<BASE64_BLOB>" }
```

### GET /inbox/:key (bridge fallback)

O cliente de inbox do transporte espera um endpoint simples:

```http
GET /inbox/<HEX_KEY> HTTP/1.1
```

```json
{ "items": ["<BASE64_ENVELOPE>", "..."] }
```

## Notas de segurança

- **CBOR canonical**: use sempre CTAP2 canonical para assinar/verificar cards e tokens.
- **Verificação de cartão**: valide assinatura e trate mudança de chave como evento crítico.
- **PoW/anti-abuso**: valide puzzles, limite taxa e atualize parâmetros conforme a ameaça.
- **Bridge/DHT são não confiáveis**: sempre verifique hashes, assinaturas e contexto MLS.
- **TLS recomendado**: use HTTPS para evitar vazamento de metadados na bridge.
- **Expiração de grants**: rejeite grants expirados e trate revogações explicitamente.

## Próximos passos

1. **Integração MLS completa** (handshake + estados reais).
2. **Bridge inbox** compatível com o fallback HTTP.
3. **Runtime libp2p** com anti-eclipse e persistência.
4. **CLI end-to-end** conectado ao transporte real.

## Test vectors v0.1.1

Os vetores de teste da versão **v0.1.1** devem ser usados como referência para validação de
compatibilidade. É importante observar que eles assumem **CBOR canonical (CTAP2)**.
