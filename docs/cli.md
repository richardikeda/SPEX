# CLI (spex-cli)

Esta página descreve os subcomandos principais do `spex-cli`, o formato do estado local e a
interpretação de fingerprints.

## Estado local

Por padrão, o CLI persiste chaves, contatos e threads em:

- `~/.spex/state.json`

O caminho pode ser sobrescrito definindo `SPEX_STATE_PATH`.

## Subcomandos

### `identity`

- `identity new`: gera uma identidade local (chave Ed25519 e metadados básicos). Usado como base
  para criação de cards e assinatura de mensagens.
- `identity rotate`: gira a chave de assinatura local, registra a rotação no log de checkpoints e
  revoga a chave anterior com motivo `key rotation`.

### `card`

- `card create`: cria um `ContactCard` em CBOR base64, contendo dados públicos da identidade.
- `card redeem --card <BASE64>`: valida e importa um card, salvando como contato local.
  - Se o contato já existir e a chave pública divergir, o CLI alerta sobre **mudança de chave**.

### `request`

- `request send --to <USER_ID_HEX> --role <N>`: gera um `RequestToken` (JSON base64) para solicitar
  acesso/participação. O token inclui um puzzle Argon2id (entrada/saída + parâmetros) validado no
  momento da criação.

### `grant`

- `grant accept --request <BASE64>`: valida um request, verifica puzzle (quando presente) e emite
  um grant **assinado** (JSON base64 com `verifying_key` e `signature`).
- `grant deny --request <BASE64>`: rejeita o request (sem gerar grant).

### `thread`

- `thread new --members <USER_ID_HEX>,<USER_ID_HEX>`: cria uma thread local com membros conhecidos.

### `msg`

- `msg send --thread <THREAD_ID_HEX> --text "..."`: envia mensagem para uma thread existente usando
  MLS + AEAD, fragmentando o envelope e registrando a publicação no outbox local.

### `inbox`

- `inbox poll`: busca mensagens pendentes no modo local.
- `inbox poll --inbox-key <HEX_KEY>`: consulta inbox via cache P2P local.
- `inbox poll --bridge-url <URL> --inbox-key <HEX_KEY>`: consulta inbox via bridge HTTP.

### `log`

- `log append-checkpoint`: registra checkpoint de chaves no log append-only.
- `log create-recovery-key`: gera uma recovery key e a registra no log.
- `log revoke-key --key-hex <HEX_KEY> --reason "..."`: adiciona revogação de chave.
- `log info`: exibe status e metadados do log.
- `log export --path <LOG_FILE>`: exporta o log em CBOR canonical base64.
- `log import --path <LOG_FILE>`: importa o log previamente exportado.
- `log gossip-verify --path <LOG_FILE>`: valida consistência de uma réplica do log.

## Fingerprints

Ao resgatar um `ContactCard`, o CLI imprime o fingerprint da chave pública do contato para
verificação manual. Se a mesma identidade for importada novamente com uma chave diferente, o
CLI emite um alerta. Essa verificação manual é o principal mecanismo para detectar trocas
maliciosas ou acidentes de rotação de chaves.
