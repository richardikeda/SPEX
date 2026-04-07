# Observabilidade de transporte e ingestao

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

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

Política implementada:
- Penalidades por evento: timeout `-8`, resposta inconsistente `-18`, payload inválido `-30`.
- Limiar de probation por score: `<= -35`.
- Limiar de ban por score: `<= -70`.
- Ban determinístico por recorrência maliciosa:
  - `invalid_payload_penalties >= 2`.
  - `inconsistent_response_penalties >= 4`.
- Recuperação gradual por interação bem sucedida: `+6` por sucesso, com limpeza de probation ao ultrapassar `-25`.

Métricas/traces adicionados para reputação:
- `reputation_probation_transitions`: total de transições para probation.
- `reputation_ban_transitions`: total de transições para ban.
- Evento de trace `operation="peer_reputation_transition"` com campos:
  - `peer_id`, `reason`, `previous_state`, `current_state`, `score`.
  - `timeout_penalties`, `invalid_payload_penalties`, `inconsistent_response_penalties`.

Troubleshooting (intermitente vs abuso recorrente):
- Peer intermitente:
  - cresce em `timeout_penalties`, pode entrar em probation, mas não deve banir rapidamente.
  - sinais: `reason=timeout`, score recupera com sucessos, `reputation_ban_transitions` estável.
- Peer malicioso recorrente:
  - repete `invalid_payload` ou `inconsistent_response` e aciona ban determinístico.
  - sinais: aumento de `invalid_payload_penalties`/`inconsistent_response_penalties` seguido de `current_state=banned`.

### Subtarefa 1.2 — Recovery/Snapshot

Sinais implementados de integridade e recuperação:
- `snapshot_recovery_status().load_state` com estados explícitos:
  - `NotConfigured` (sem persistência configurada),
  - `Missing` (arquivo ausente),
  - `Loaded` (snapshot íntegro restaurado),
  - `QuarantinedRecovered` (snapshot corrompido isolado e fallback limpo aplicado).
- Contadores de restauração:
  - `restored_known_peers`, `restored_manifests`, `restored_index_keys`.
- Contadores de isolamento/quarentena:
  - `quarantined_snapshots`, `last_quarantined_path`.

Comportamento de erro determinístico:
- Snapshot parcial/corrompido é movido para quarentena (`*.corrupt-<unix>.json`) e o runtime inicia com snapshot vazio seguro.
- A inicialização retorna warning explícito em `persistence_warnings()` com mensagem determinística de corrupção.

Runbook de decisão (restart/recovery):
1. Verificar `snapshot_recovery_status().load_state`.
2. Se `Loaded`: seguir operação normal e comparar contadores restaurados com baseline esperado.
3. Se `QuarantinedRecovered`: preservar arquivo em quarentena para análise forense e operar em modo degradado até re-hidratação.
4. Se `Missing`/`NotConfigured`: iniciar bootstrap padrão e validar convergência de peers/manifests antes de promover para healthy.

### Subtarefa 1.3 — Churn testing

SLOs implementados para churn prolongado:
- Publish sob churn sem peers deve falhar de forma explícita dentro da janela alvo: 400ms a 8000ms.
- Recovery sob churn sem dados deve encerrar em timeout controlado: 2000ms a 5000ms.
- Pressão de retry deve permanecer abaixo de saturação:
  - `publish_retry_pressure_bps <= 50000`;
  - `recovery_retry_pressure_bps <= 50000`.

Indicadores de flapping/saturação:
- `publish_retries`, `recovery_retries`, `publish_timeout`, `recovery_timeout` monitorados por `metrics_snapshot()`.
- Classificação de saúde de rede com limiares explícitos via `network_health_indicators(...)`:
  - `healthy`: conectividade e ratios dentro da meta;
  - `degraded`: conectividade limítrofe ou metade dos limiares atingida;
  - `critical`: conectividade insuficiente ou violação direta dos limiares.

Matriz de sintomas versus ação corretiva:
1. Sintoma: `publish_timeout` crescente com `connected_peers` baixo.
  Ação: ampliar bootstrap peers, manter backoff padrão e evitar retry agressivo manual.
2. Sintoma: `recovery_retry_pressure_bps` acima de baseline.
  Ação: verificar disponibilidade de provedores DHT/gossip e aplicar re-hidratação progressiva.
3. Sintoma: status `critical` recorrente.
  Ação: isolar nós degradados, preservar evidências de métricas e executar recovery controlado.
4. Sintoma: alternância `degraded`/`critical` em intervalos curtos (flapping).
  Ação: reduzir churn de peers de borda, revisar thresholds operacionais e estabilizar janela de timeout.

### Subtarefa 1.4 — Observabilidade

Catálogo final por operação:
- `publish`:
  - contadores: `publish_attempts`, `publish_retries`, `publish_timeout`, `publish_success`;
  - latência: `publish_latency_ms`;
  - correlação: `derive_operation_correlation("publish", context)` com fallback explícito quando contexto ausente.
- `recovery`:
  - contadores: `recovery_attempts`, `recovery_retries`, `recovery_timeout`, `recovery_success`;
  - latência: `recovery_latency_ms`;
  - sinais de integridade: parse de manifest, verificação de chunk e reassemble com erro explícito.
- `fallback`:
  - contadores globais: `fallback_attempts`, `fallback_success`, `fallback_failure`;
  - ratio operacional: `fallback_failure_ratio_bps` em `network_health_indicators(...)`.
- `ingest`:
  - validação determinística de payload (`grant`/`puzzle`) com erro explícito e sem panic;
  - correlação: `ingest_validation_correlation_id(...)` sem uso de payload bruto.
- `reassemble`:
  - correlação por formato: `reassemble_correlation_id(manifest)` baseado em forma do manifest (`total_len` + quantidade de chunks), sem bytes sensíveis.

Política de correlação determinística:
1. Contexto disponível e não vazio: usar `derive_operation_correlation_id(operation, context)`.
2. Contexto ausente/vazio: usar fallback determinístico `derive_minimal_correlation_id(operation)`.
3. Toda aplicação de fallback deve ser auditável via `used_minimal_context` na estrutura de correlação.

Checklist operacional de readiness:
1. Taxa de timeout (`timeout_ratio_bps`) dentro dos limiares de `NetworkHealthThresholds`.
2. `fallback_failure_ratio_bps` abaixo do limite configurado para produção.
3. Ausência de `critical` sustentado em janelas contínuas de observação.
4. Erros de ingest/reassemble permanecem explícitos, determinísticos e sem panic paths.
5. IDs de correlação permanecem estáveis para o mesmo contexto e diferentes entre contextos distintos.
