# Observabilidade de transporte e ingestão

Este documento define métricas, traces e indicadores de saúde para operação contínua do `spex-transport`.

## Métricas estruturadas

As métricas são expostas por `P2pMetricsSnapshot`.

### Publish
- `publish_attempts`: total de operações de publish iniciadas.
- `publish_success`: publishes concluídos.
- `publish_timeout`: publish que expirou por falta de peers.
- `publish_retries`: retries por `InsufficientPeers`.
- `publish_latency_ms`: histograma de latência por sucesso.
- `publish_success_rate_bps()`: taxa de sucesso em basis points.

### Recovery
- `recovery_attempts`: total de recoveries iniciadas.
- `recovery_success`: recoveries com payload recuperado.
- `recovery_timeout`: recoveries sem payload até deadline.
- `recovery_retries`: re-subscribes do tópico de inbox.
- `recovery_latency_ms`: histograma de latência por sucesso.
- `recovery_timeout_rate_bps()`: taxa de timeout em basis points.

### Fallback bridge
- `fallback_attempts`: número de ativações do fallback HTTP.
- `fallback_success`: fallbacks bem sucedidos.
- `fallback_failure`: fallbacks com erro.
- `fallback_frequency_bps()`: frequência do fallback relativa a `recovery_attempts`.

### Erros de reassemble/verificação
- `reassemble_failures`: falhas ao remontar payload por manifest/chunks.
- `verification_failures`: falhas de parsing de manifesto ou verificação de chunks.

## Tracing e correlação

- Correlation IDs são derivados deterministicamente por operação com
  `derive_operation_correlation_id(operation, context)`.
- Em ausência de contexto mínimo (metadado de tracing ausente), usa-se `derive_minimal_correlation_id(operation)` para manter correlação determinística sem vazar payload.
- O contexto é hashado e truncado (sem payload bruto, sem chaves privadas), evitando vazamento sensível.

Campos de trace relevantes:
- `operation`: classe operacional (`publish_manifest`, `recovery_inbox`, `fallback_bridge`, `manifest_parse`, `chunk_verify`, `reassemble`).
- `correlation_id`: identificador determinístico para correlacionar eventos da mesma operação.
- Backoff adaptativo possui teto por perfil (incluindo cenários de timeout extremo) para preservar previsibilidade operacional.
- `latency_ms`, `attempt`, `delay_ms`, `items`.

## Indicadores de saúde de rede

`network_health_indicators(thresholds)` expõe:
- `connected_peers`
- `known_peers`
- `banned_peers`
- `timeout_ratio_bps`
- `fallback_failure_ratio_bps`
- `status`: `healthy`, `degraded`, `critical`

### Limiar padrão (`NetworkHealthThresholds::default`)
- `min_connected_peers = 2`
- `max_timeout_ratio_bps = 2500` (25%)
- `max_fallback_failure_ratio_bps = 3000` (30%)

Interpretação operacional:
- **Healthy**: conectividade e razões de erro dentro do esperado.
- **Degraded**: sinais intermediários, pede investigação.
- **Critical**: conectividade insuficiente ou erro acima do limite.

## Plano incremental por subtarefa (TASK 1)

Para evitar atualização "Big Bang" no fim, este documento deve ser atualizado ao término de cada subtarefa abaixo.

### Subtarefa 1.1 — Reputação

- Atualização obrigatória ao concluir:
  - política de score/penalidade e thresholds de promoção/quarentena/ban;
  - métricas de falso positivo/falso negativo operacional;
  - eventos de trace para transição de estado de reputação.
- Critério de aceite documental:
  - seção de troubleshooting contendo como diferenciar peer intermitente de abuso recorrente.

### Subtarefa 1.2 — Recovery/Snapshot

- Atualização obrigatória ao concluir:
  - sinais de integridade de snapshot e resultado de validação no boot;
  - contadores de quarentena para estado parcial/corrompido;
  - fluxo de diagnóstico para recovery após restart.
- Critério de aceite documental:
  - runbook com decisão explícita entre retry, isolamento e recuperação limpa.

### Subtarefa 1.3 — Churn testing

- Atualização obrigatória ao concluir:
  - SLOs de publish/recovery sob churn e respectivos limiares;
  - indicadores de flapping, saturação de retry e impacto em latência;
  - critérios de status (`healthy`, `degraded`, `critical`) calibrados para churn.
- Critério de aceite documental:
  - matriz de sintomas versus ação corretiva para incidentes de churn prolongado.

### Subtarefa 1.4 — Observabilidade

- Atualização obrigatória ao concluir:
  - catálogo final de métricas/traces por operação (`publish`, `recovery`, `fallback`, `ingest`, `reassemble`);
  - política de correlação determinística com fallback para metadado ausente;
  - checklist de campos obrigatórios para auditoria operacional.
- Critério de aceite documental:
  - seção de readiness com critérios objetivos para liberar operação contínua em produção.
