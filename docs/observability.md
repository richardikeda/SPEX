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
- Em ausência de contexto mínimo, usa-se `derive_minimal_correlation_id(operation)`.
- O contexto é hashado e truncado (sem payload bruto, sem chaves privadas), evitando vazamento sensível.

Campos de trace relevantes:
- `operation`: classe operacional (`publish_manifest`, `recovery_inbox`, `fallback_bridge`, `manifest_parse`, `chunk_verify`, `reassemble`).
- `correlation_id`: identificador determinístico para correlacionar eventos da mesma operação.
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
