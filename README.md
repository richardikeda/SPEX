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

Implementação inicial em andamento com os seguintes componentes e nível atual:
- **spex-core**: primitives de tipos SPEX, CBOR canonical (CTAP2), hashes, assinatura, PoW,
  log append-only e validações compartilhadas de grants/PoW.
- **spex-mls**: integração MLS completa via `mls-rs`, com TreeKEM, commits, updates, add/remove e
  fluxos de ressincronização para grupos multi-membros.
- **spex-transport**: base libp2p com chunking e manifestos, DHT/Kademlia, gossip, rotinas
  de recuperação e fallback bridge. Inclui perfis explícitos (`P2pRuntimeProfile`) para tempos de publish/query/recovery, backoff adaptativo com jitter e métricas de latência/sucesso/timeout/retry.
  de inbox scanning e validação de ingestão P2P (grant/PoW), com snapshot persistente
  para peers/bootstrap (gravação atômica) e peer scoring anti-eclipse com ban temporário.
  de reassemblagem/verificação e helpers de publicação/recuperação via manifestos, incluindo
  publicação direta de chunks e recuperação de payloads a partir de manifestos compartilhados.
  - **spex-bridge**: bridge HTTP com SQLite para cards/slots/inbox, rate limit e validações de grant/PoW,
    com endpoint de escrita (`PUT /inbox/:key`) e leitura (`GET /inbox/:key`) para ingest/scan.
- **spex-cli**: CLI de referência para identidades, cartões, request/grant, threads, envio de
  mensagens (gera envelope + chunks/manifestos) e polling de inbox via transporte, bridge ou rede P2P.
- **spex-client**: biblioteca de alto nível que padroniza criação de estado local, cifragem MLS,
  validações de request/grant e helpers de chunking usados pelo CLI.

## Roadmap (pendências atuais)

- **Escrita de inbox via cliente/transporte**: concluída com API de alto nível para publicar envelopes na bridge (`publish_via_bridge` + `build_bridge_publish_request`) e cobertura de integração CLI↔bridge.
  - Referências: [docs/bridge-api.md](docs/bridge-api.md), `crates/spex-client/tests/bridge_publish.rs`, `crates/spex-transport/tests/bridge_publish_http.rs`.
- **Hardening/observabilidade do runtime P2P**: reduzir latências operacionais, ampliar reputação anti-eclipse e métricas/tracing de publish/recovery/fallback.
  - Referências: [TODO.md](TODO.md), testes de transporte em `crates/spex-transport/tests/p2p_manifest_delivery.rs`.
- **Conformidade MLS avançada**: suíte ampliada para reorder/replay/epoch gap, ressincronização determinística e robustez de parsing.
  - Referências: [docs/integration.md](docs/integration.md), testes em `crates/spex-mls/tests/planned_concurrent_updates.rs` e `crates/spex-mls/tests/epoch_recovery_properties.rs`.

## Documentação

- [Visão geral da arquitetura](docs/overview.md)
- [CLI (spex-cli)](docs/cli.md)
- [Integração](docs/integration.md)
- [Wire format (CBOR)](docs/wire-format.md)
- [Bridge HTTP API](docs/bridge-api.md)
- [Segurança](docs/security.md)
- [Observabilidade de transporte/ingestão](docs/observability.md)
- [Checklist de release v1.0 (go/no-go)](docs/release-v1-checklist.md)
- [Matriz de cenários MLS avançados](docs/mls-advanced-scenarios-matrix.md)
- [Runbook operacional de release/incidentes](docs/runbook-release-operations.md)

Os documentos acima detalham arquitetura, wire format com tabelas de IDs/tipos CBOR, bridge HTTP
com exemplos de payloads e status codes, fluxo request/grant, armazenamento local (`~/.spex/state.json`),
fingerprints, requisitos de TLS e práticas de segurança recomendadas.


### Observabilidade de transporte/ingestão

O `spex-transport` expõe métricas estruturadas para publish/recovery/fallback, tracing com `correlation_id` determinístico por operação (incluindo fallback estável quando metadados mínimos estão ausentes) e indicadores contínuos de saúde de rede.

Referências operacionais:
- catálogo de métricas/traces: [docs/observability.md](docs/observability.md)
- snapshot de métricas: `P2pMetricsSnapshot`
- saúde de rede: `network_health_indicators(NetworkHealthThresholds)`

## Fluxo de release candidate (v1.0)

### Gates de CI/release (fonte de verdade)

- **`Rust CI`** (`.github/workflows/rust.yml`): workflow principal para PR/push em `main`, com foco em feedback rápido e validações essenciais.
  - `build-and-test`: valida `cargo build --workspace --locked` em `ubuntu-latest` (stable/beta) e `macos-latest` (stable), mantendo os testes do workspace (`default` e `all-features`) apenas em `ubuntu-latest` + stable para evitar duplicidade de carga.
  - `lint`: valida `cargo fmt --all -- --check` e `cargo clippy --workspace --locked -- -D warnings` em `ubuntu-latest` + stable.
- **`Release Readiness`** (`.github/workflows/release-readiness.yml`): mantém gates obrigatórios de release em PR/push (`release-critical-tests`, `release-docs-and-quality`, `release-negative-gate`) e move suites pesadas de robustez/supply-chain para execução agendada semanal (`cron`) ou manual (`workflow_dispatch`).

Para fechamento de versão, execute os gates objetivos abaixo:

```bash
cargo test --workspace --locked --verbose
cargo test --workspace --locked --all-features --verbose
cargo fmt --all -- --check
cargo clippy --workspace --locked -- -D warnings
./scripts/release_gate_docs.sh
./scripts/release_gate_negative_test.sh
```

Critério de aprovação:
- **GO**: todos os gates passaram.
- **NO-GO**: qualquer falha em teste crítico, robustez, lint/format ou documentação.

Referências detalhadas:
- [Checklist de release v1.0](docs/release-v1-checklist.md)
- [Matriz de cenários MLS avançados](docs/mls-advanced-scenarios-matrix.md) (**critério de pronto da v1 para robustez MLS**)
- [Runbook operacional de release](docs/runbook-release-operations.md)

## Avisos de segurança

- **TLS obrigatório** em qualquer integração HTTP/externa (bridge e serviços de terceiros).
- **Validação de grants e PoW** deve ser aplicada em todos os pontos de entrada.
- **Proteção do estado local** é mandatória (criptografia e/ou keychain para `~/.spex/state.json`).

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

### CI (matrix)

O pipeline em `.github/workflows/rust.yml` executa uma matrix com:

- **Sistemas operacionais**: `ubuntu-latest` e `macos-latest`
- **Toolchains Rust**: `stable` e `beta`

Escopo por job:

- **build-and-test**: `cargo build` em toda a matrix; suíte de testes (`default` e `all-features`) somente em `ubuntu-latest` + `stable` para controlar custo de CI sem perder cobertura principal.
- **lint**: `cargo fmt --check` e `cargo clippy -D warnings` somente em `ubuntu-latest` + `stable`.

O cache é segregado por job/SO/toolchain para evitar colisões entre ambientes.

### MLS external commit handling (deterministic)

O `spex-mls` agora expõe APIs de alto nível para:

- processamento explícito de commits externos com `epoch` informado pela aplicação;
- detecção de `epoch gap` (`Current`, `Next`, `Stale`, `Gap`);
- ressincronização determinística via fetch/aplicação sequencial de commits faltantes.

Commits fora de ordem são rejeitados com erro estruturado e a recuperação só é aceita quando os epochs faltantes são fornecidos exatamente na sequência esperada.

### Robustness Strategy

Para reforçar parsing/validação contra entradas arbitrárias, o repositório inclui:

- **Fuzz targets (`cargo-fuzz`)** em `fuzz/fuzz_targets/` para:
  - `GrantToken::decode_ctap2`
  - `ContactCard::decode_ctap2`
  - `parse_cbor_payload`
  - parsing de payload de bridge (`parse_storage_request_bytes`, `parse_inbox_store_request_bytes`)
  - validação adversarial de payloads P2P (`validate_p2p_grant_payload`, `validate_p2p_puzzle_payload`)
- **Property tests (`proptest`)** em `spex-core`, `spex-bridge` e `spex-transport` para:
  - estabilidade/idempotência da canonicalização CTAP2
  - rejeição segura de base64 inválido
  - ausência de `panic` para entradas arbitrárias
  - determinismo da validação P2P para payload idêntico (mesmo resultado/erro)
- **Property tests (`proptest`)** em `spex-mls` para:
  - determinismo de ressincronização sob permutações de commits faltantes
  - rejeição explícita de sequências parciais/incompatíveis sem mutação de epoch local
- **Fuzz target MLS** para:
  - `parse_external_commit` com payload arbitrário sem panic path

Comandos úteis:

```bash
cargo test -p spex-core
cargo test -p spex-bridge
```

Comando oficial de fuzz smoke (curto e determinístico):

```bash
for target_file in fuzz/fuzz_targets/*.rs; do
  target_name="$(basename "${target_file}" .rs)"
  cargo +nightly fuzz run "${target_name}" --fuzz-dir fuzz -- -max_total_time=30 -seed=1
done
```


Os testes incluem vetores, integrações (handshake, PoW, MLS, DHT/bridge) e validações de
assinatura para `ContactCard` e `GrantToken`.

### Uso rápido (CLI)

Os comandos abaixo usam o fluxo consolidado do `spex-client` para criação de estado local, cifragem
MLS e validações de request/grant.

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

# rotacionar chaves locais (revoga a anterior no log)
cargo run -p spex-cli -- identity rotate

# criar thread local com membros (hex separados por vírgula)
cargo run -p spex-cli -- thread new --members <USER_ID_HEX>,<USER_ID_HEX>

# enviar mensagem para uma thread via MLS + transporte
cargo run -p spex-cli -- msg send --thread <THREAD_ID_HEX> --text "Olá"
# enviar também para bridge HTTP com TTL opcional
cargo run -p spex-cli -- msg send --thread <THREAD_ID_HEX> --text "Olá" \
  --bridge-url <URL> --ttl-seconds 120

# enviar mensagem via rede P2P libp2p (bootstrap/peers)
cargo run -p spex-cli -- msg send --thread <THREAD_ID_HEX> --text "Olá" --p2p \
  --bootstrap /ip4/127.0.0.1/tcp/9001/p2p/<PEER_ID>

# verificar inbox local, via cache P2P ou via bridge HTTP (com decifragem MLS)
cargo run -p spex-cli -- inbox poll
cargo run -p spex-cli -- inbox poll --inbox-key <HEX_KEY>
cargo run -p spex-cli -- inbox poll --bridge-url <URL> --inbox-key <HEX_KEY>

# recuperar inbox via rede P2P libp2p (manifestos + DHT) com fallback HTTP opcional
cargo run -p spex-cli -- inbox poll --p2p --inbox-key <HEX_KEY> \
  --peer /ip4/127.0.0.1/tcp/9001/p2p/<PEER_ID> \
  --bridge-url <URL>

# checkpoints de chaves e log append-only
cargo run -p spex-cli -- log append-checkpoint
cargo run -p spex-cli -- log create-recovery-key
cargo run -p spex-cli -- log revoke-key --key-hex <HEX_KEY> --reason "compromised"
cargo run -p spex-cli -- log info
cargo run -p spex-cli -- log export --path <LOG_FILE>
cargo run -p spex-cli -- log export-abuse --db-path <BRIDGE_DB> --path <ABUSE.jsonl> \
  --request-kind inbox --outcome rejected --since <UNIX_TS> --until <UNIX_TS> --limit 500
cargo run -p spex-cli -- log import --path <LOG_FILE>
cargo run -p spex-cli -- log gossip-verify --path <LOG_FILE>
```

### Executando a bridge HTTP

```bash
# inicia a bridge em 0.0.0.0:3000
cargo run -p spex-bridge
```

Obs.: o bind padrão `0.0.0.0` expõe a bridge em todas as interfaces de rede. Para uso local restrito,
use proxy reverso e/ou firewall; no futuro, uma flag/env de bind explícito pode oferecer controle direto.

Para integração via biblioteca, `spex_bridge::init_state` e `spex_bridge::init_state_with_clock`
retornam `BridgeError` em falhas de inicialização.

### Fluxo básico de handshake (request/grant)

1. O remetente envia um `RequestToken` (JSON base64) com puzzle Argon2id para o destinatário.
2. O destinatário responde com um grant **assinado** (JSON base64 com `verifying_key` + `signature`).
3. A thread MLS é criada usando o `ThreadConfig` com o grant recebido.

### Persistência local e fingerprints

O `spex-cli` persiste chaves, contatos e threads em `~/.spex/state.json` (ou no caminho definido por
`SPEX_STATE_PATH`). O arquivo é criptografado com chave armazenada no keychain do SO; se não houver
keychain disponível, defina `SPEX_STATE_PASSPHRASE` para usar uma passphrase (o CLI se recusa a
salvar o estado sem proteção). Ao resgatar um cartão, o CLI imprime o fingerprint da chave pública e
alerta em caso de mudança de chave para um contato já conhecido.

## Checkpoints, recovery e revogação de chaves

O SPEX mantém um **log append-only baseado em Merkle tree** para checkpoints de chaves públicas,
recovery keys e declarações de revogação. Esse log permite comparar consistência (prefixo) entre
réplicas e verificar integridade usando o root do Merkle tree.

### Exportação/importação do log

O CLI exporta/importa o log em **CBOR canonical codificado em base64**. Isso facilita transporte
em canais de texto e compatibilidade com o armazenamento local.

Para fluxos operacionais de abuso/revogação/recovery em ambientes heterogêneos, consulte
[`docs/operations-revocation-recovery-abuse.md`](docs/operations-revocation-recovery-abuse.md).

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
    "expires_at": 1700000200,
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

```http
GET /cards/<SHA256_HEX> HTTP/1.1
```

```json
{ "data": "<BASE64_CBOR_CARD>" }
```

### PUT/GET /slot/:slot_id

O `slot_id` deve ser o SHA-256 hex do blob armazenado.

```http
PUT /slot/<SHA256_HEX> HTTP/1.1
Content-Type: application/json

{ "...mesmo payload do /cards..." }
```

```http
GET /slot/<SHA256_HEX> HTTP/1.1
```

```json
{ "data": "<BASE64_BLOB>" }
```

### PUT /inbox/:key (bridge ingest)

Armazena envelopes destinados ao inbox scanning via bridge. O payload segue o mesmo formato de
`/cards` e `/slot`, com `ttl_seconds` opcional para controlar expiração.

```http
PUT /inbox/<HEX_KEY> HTTP/1.1
Content-Type: application/json

{
  "...mesmo payload do /cards...",
  "ttl_seconds": 3600
}
```

- `ttl_seconds` é opcional (padrão 86.400s, máximo 604.800s).
- `data` limitado a 262.144 bytes por envelope.

### GET /inbox/:key (bridge fallback)

O cliente de inbox do transporte espera um endpoint simples:

```http
GET /inbox/<HEX_KEY> HTTP/1.1
```

```json
{
  "items": ["<BASE64_ENVELOPE>", "..."],
  "next_cursor": 42
}
```

Query params:

- `limit` (opcional): máximo de itens por página (padrão 100, máximo 500).
- `cursor` (opcional): retorna itens com `id` maior que o cursor informado.
- `max_bytes` (opcional): limite total de bytes retornados por página.

Itens expirados (`expires_at`) são omitidos do resultado.

Responses:

- `200 OK`: retorna um array (pode estar vazio).
- `404 Not Found`: inbox ainda não existe.

## Notas de segurança

- **CBOR canonical**: use sempre CTAP2 canonical para assinar/verificar cards e tokens.
- **Verificação de cartão**: valide assinatura e trate mudança de chave como evento crítico.
- **PoW/anti-abuso**: valide puzzles, limite taxa e exija parâmetros mínimos (memória ≥64 MiB,
  iterações ≥3) antes de aceitar requests.
- **Bridge/DHT são não confiáveis**: sempre verifique hashes, assinaturas e contexto MLS.
- **TLS recomendado**: use HTTPS para evitar vazamento de metadados na bridge.
- **Expiração de grants**: rejeite grants expirados e trate revogações explicitamente.
- **Dados em repouso**: proteja o armazenamento local (`~/.spex/state.json`) com permissões restritas.


## Runtime P2P operacional (hardening)

O runtime `spex-transport` agora aplica tuning operacional explícito por perfil (`Dev`, `Test`, `Prod`) para reduzir esperas fixas:

- `publish_wait`, `query_timeout` e `manifest_wait` são definidos por `P2pNodeConfig::for_profile`.
- Em runtime, os timeouts são ajustados por conectividade (`P2pTransport::tuned_timeouts`) para reduzir latência média sem quebrar compatibilidade de rede.
- Em cenários extremos, o backoff adaptativo permanece limitado por perfil para evitar esperas desproporcionais e manter convergência determinística.
- Políticas de reputação distinguem falhas transitórias (probation) de abuso recorrente (ban temporário).
- Snapshots de estado persistem reputação e metadados para recovery seguro após churn prolongado.
- Estado corrompido é quarentenado automaticamente, com aviso explícito (`persistence_warnings`).

## Próximos passos

1. **Ampliar hardening e observabilidade do runtime libp2p** para operação contínua em produção.
2. **Expandir suíte MLS avançada** com cenários adicionais de conformidade e recuperação.
3. **Expandir testes adversariais/fuzz** para parsing de payload HTTP e superfícies bridge/P2P.

## Test vectors v0.1.1

Os vetores de teste da versão **v0.1.1** devem ser usados como referência para validação de
compatibilidade. É importante observar que eles assumem **CBOR canonical (CTAP2)**.
