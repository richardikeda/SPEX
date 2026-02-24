# Operações: abuso, revogação e recovery

Este guia cobre fluxos operacionais para ambientes heterogêneos com foco em auditabilidade e segurança.

## Exportação de logs de abuso

Use o comando abaixo para exportar eventos do banco SQLite da bridge em formato JSON Lines estável.

```bash
cargo run -p spex-cli -- log export-abuse \
  --db-path <BRIDGE_DB_PATH> \
  --path <ABUSE_LOGS.jsonl> \
  --request-kind inbox \
  --outcome rejected \
  --since 1700000000 \
  --until 1700003600 \
  --limit 1000
```

Campos exportados:
- `timestamp`
- `identity_hash_hex` (hash SHA-256 da identidade; não exporta identidade em claro)
- `ip_prefix` (já mascarado na persistência)
- `request_kind`
- `outcome`
- `bytes`

## Revogação de chave com trilha de auditoria

1. Gere e registre uma recovery key:

```bash
cargo run -p spex-cli -- log create-recovery-key
```

2. Revogue a chave comprometida com justificativa operacional:

```bash
cargo run -p spex-cli -- log revoke-key --key-hex <HEX_KEY> --recovery-hex <RECOVERY_HEX> --reason "compromised"
```

Regras aplicadas:
- revogação exige identidade local autenticada;
- chave alvo deve existir no histórico de checkpoints;
- operação é idempotente (revogação repetida não duplica entrada);
- `recovery-hex`, quando informado, precisa corresponder a recovery key válida/não expirada no log.

## Recovery para integrações externas

Para interoperabilidade entre ambientes:

1. Exporte o log no ambiente de origem:

```bash
cargo run -p spex-cli -- log export --path <CHECKPOINT_LOG.b64>
```

2. Importe no ambiente de destino:

```bash
cargo run -p spex-cli -- log import --path <CHECKPOINT_LOG.b64>
```

3. Verifique consistência por gossip/prova de prefixo:

```bash
cargo run -p spex-cli -- log gossip-verify --path <CHECKPOINT_LOG.b64>
```
