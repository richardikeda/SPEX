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
- **spex-mls**: estruturas mínimas para contexto MLS + extensões SPEX, commits de membros e validação de epoch/configuração (mls-rs).
- **spex-transport**: chunking por hash, publicação/replicação DHT/Kademlia, gossip, random walks e inbox scanning derivado de `inbox_scan_key` com fallback via bridge HTTP.
    - **spex-bridge**: bridge HTTP com armazenamento SQLite (cards/slots) e validações básicas.
- **spex-cli**: CLI de referência para identidades, cartões e fluxo básico de pedidos/grants.
- **spex-core/log**: log append-only com Merkle tree para checkpoints de chaves, recovery keys e declarações de revogação.

## Documentação

- [Visão geral da arquitetura](docs/overview.md)
- [CLI (spex-cli)](docs/cli.md)
- [Integração](docs/integration.md)
- [Wire format (CBOR)](docs/wire-format.md)
- [Bridge HTTP API](docs/bridge-api.md)
- [Segurança](docs/security.md)

## Build e uso

### Build

```bash
cargo build
```

### Build (componentes específicos)

```bash
# build da bridge HTTP
cargo build -p spex-bridge

# build do transporte
cargo build -p spex-transport
```

### Testes

```bash
cargo test
```

Os testes incluem vetores, além de validações de assinatura para `ContactCard` e `GrantToken`.

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

# aceitar ou negar grant recebido (token base64 do request)
cargo run -p spex-cli -- grant accept --request <BASE64>
cargo run -p spex-cli -- grant deny --request <BASE64>

# criar thread local com membros (hex separados por vírgula)
cargo run -p spex-cli -- thread new --members <USER_ID_HEX>,<USER_ID_HEX>

# enviar mensagem local para uma thread
cargo run -p spex-cli -- msg send --thread <THREAD_ID_HEX> --text "Olá"

# verificar inbox local ou via bridge HTTP
cargo run -p spex-cli -- inbox poll
cargo run -p spex-cli -- inbox poll --bridge-url <URL> --inbox-key <HEX_KEY>

# checkpoints de chaves e log append-only
cargo run -p spex-cli -- log append-checkpoint
cargo run -p spex-cli -- log create-recovery-key
cargo run -p spex-cli -- log revoke-key --key-hex <HEX_KEY> --reason "compromised"
cargo run -p spex-cli -- log info
cargo run -p spex-cli -- log export --path <LOG_FILE>
cargo run -p spex-cli -- log import --path <LOG_FILE>
cargo run -p spex-cli -- log gossip-verify --path <LOG_FILE>
```

### Executando a bridge HTTP

```bash
# inicia a bridge em 127.0.0.1:3000
cargo run -p spex-bridge
```

### Fluxo básico de handshake (request/grant)

1. O remetente envia um `RequestToken` (JSON base64) para o destinatário.
2. O destinatário responde com um `GrantToken` (CBOR canonical base64).
3. A thread MLS é criada usando o `ThreadConfig` com o grant recebido.

### Persistência local e fingerprints

O `spex-cli` persiste chaves, contatos e threads em `~/.spex/state.json` (ou no caminho definido por
`SPEX_STATE_PATH`). Ao resgatar um cartão, o CLI imprime o fingerprint da chave pública e alerta em
caso de mudança de chave para um contato já conhecido.

## Checkpoints, recovery e revogação de chaves

O SPEX mantém um **log append-only baseado em Merkle tree** para checkpoints de chaves públicas,
recovery keys e declarações de revogação. Esse log permite comparar consistência (prefixo) entre
réplicas e verificar integridade usando o root do Merkle tree.

### Exportação/importação do log

O CLI exporta/importa o log em **CBOR canonical codificado em base64**. Isso facilita transporte
em canais de texto e compatibilidade com o armazenamento local.

## Wire format (CBOR canonical/CTAP2)

Todos os payloads CBOR usam mapas com chaves inteiras e serialização CTAP2
para garantir canonicalização e assinaturas determinísticas.
Para detalhes completos (IDs, tipos e exemplos hex/base64), consulte
[docs/wire-format.md](docs/wire-format.md).

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

### RequestToken (JSON base64)

`RequestToken` é serializado como JSON e depois codificado em base64.

| Campo | Tipo | Descrição |
| --- | --- | --- |
| `from_user_id` | string (hex) | Usuário que solicita o grant. |
| `to_user_id` | string (hex) | Usuário que recebe o pedido. |
| `role` | uint | Papel/nível de acesso solicitado. |
| `created_at` | uint | Timestamp UNIX (segundos). |

## Extensões MLS (SPEX)

As extensões MLS usam a faixa privada (0xF0A0/0xF0A1). O wire-format usa big-endian:

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

Os detalhes do contrato HTTP, requisitos de puzzle/grant e códigos de status
estão em [docs/bridge-api.md](docs/bridge-api.md).

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

Responses:

- `200 OK`: retorna um array (pode estar vazio).
- `404 Not Found`: inbox ainda não existe.

## Notas de segurança

- **CBOR canonical**: use sempre CTAP2 canonical para assinar/verificar cards e tokens.
- **Verificação de cartão**: valide assinatura e trate mudança de chave como evento crítico.
- **PoW/anti-abuso**: valide puzzles, limite taxa e atualize parâmetros conforme a ameaça.
- **Bridge/DHT são não confiáveis**: sempre verifique hashes, assinaturas e contexto MLS.
- **TLS recomendado**: use HTTPS para evitar vazamento de metadados na bridge.
- **Expiração de grants**: rejeite grants expirados e trate revogações explicitamente.
- **Dados em repouso**: proteja o armazenamento local (`~/.spex/state.json`) com permissões restritas.

## Próximos passos

1. **Integração MLS completa** (handshake + estados reais).
2. **Bridge inbox** compatível com o fallback HTTP.
3. **Runtime libp2p** com anti-eclipse e persistência.
4. **CLI end-to-end** conectado ao transporte real.

## Test vectors v0.1.1

Os vetores de teste da versão **v0.1.1** devem ser usados como referência para validação de
compatibilidade. É importante observar que eles assumem **CBOR canonical (CTAP2)**.
