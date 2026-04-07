# SPEX TODO List

## Status de fechamento v1.0 (revisado)

- ✅ `TASK 4` (readiness de release) foi concluída nesta execução com checklist, runbook e gates de CI/documentação.
- 🔒 Itens ainda pendentes para fechamento completo da v1.0:
  - `TASK 1` Hardening final do runtime P2P + observabilidade operacional.
  - `TASK 2` Conformidade MLS avançada (epochs, commits e ressincronização).
  - `TASK 3` Expansão de robustez adversarial (fuzz + property tests em superfícies críticas).

A publicação definitiva da v1.0 permanece bloqueada até conclusão validada de `TASK 1-3`.

## Roadmap de execução (SPEX only, tarefa por tarefa)

Este roadmap organiza a entrega da v1.0 em sequência operacional, sempre com testes obrigatórios por etapa.

### Fase 0 - Norte e congelamento de escopo v1.0

Objetivo:
- Consolidar SPEX como protocolo E2E baseado em MLS e agnóstico de rede (`HTTP`, `P2P`, `WebSocket`).
- Congelar escopo da v1.0 para o fluxo essencial: identidade -> grupo -> membro -> envio -> consumo.

Entregáveis:
- Declaração oficial de posicionamento no `README.md`.
- Escopo v1.0 fechado e itens não essenciais movidos para backlog pós-v1.

Testes obrigatórios:
- Regressão do fluxo feliz via CLI (duas identidades trocando payload válido).
- Caso negativo de autorização (membro não autorizado não descriptografa).

Critério de conclusão:
- Escopo aprovado e documentado sem ambiguidade.

Status de execução da fase:
- ✅ Fase 0 concluída: declaração oficial consolidada em `README.md` e escopo v1.0 congelado.
- ✅ Itens não essenciais ficam direcionados para backlog pós-v1.

Evidência de testes da fase:
- ✅ Regressão de fluxo feliz: `cargo test -p spex-cli --test planned_cli_flow test_cli_message_send -- --exact`.
- ✅ Negativo de autorização: `cargo test -p spex-client --test security_replay_tamper non_member_sender_is_rejected_before_decryption -- --nocapture`.

Backlog pós-v1 (fora do escopo de fechamento v1.0):
- Evoluções de UX e integrações opcionais não necessárias ao fluxo essencial E2E.
- Expansões de features que não impactam diretamente identidade -> grupo -> membro -> envio -> consumo.

### Fase 1 - Interface de integração SPEX (SDK/Bindings)

Objetivo:
- Expor uma interface estável de cliente para integrações (incluindo web) sem alterar invariantes de segurança.

Entregáveis:
- API mínima estável para inicialização, criação de grupo, adição de membro, envio e recebimento.
- Contrato de erros explícitos para falhas de epoch, assinatura e autorização.

Testes obrigatórios:
- Unit tests para API pública e mapeamento de erros.
- Testes de integração ponta a ponta usando a API (init -> create_group -> add_member -> send/receive).
- Testes negativos para chave inválida, assinatura inválida e epoch inconsistente.

Critério de conclusão:
- API de integração estável e coberta por suíte determinística em CI.

### Fase 2 - Fechamento do Core (wire + compatibilidade)

Objetivo:
- Congelar wire format e alinhar documentação/código com verificação automatizada.

Entregáveis:
- `docs/wire-format.md` 100% aderente ao código.
- Política de versionamento para mudanças de formato.

Testes obrigatórios:
- Vetores fixos de serialização/assinatura.
- Regressão de compatibilidade para encode/decode canônico.
- Casos negativos com payload truncado, tipo inválido e hash divergente.

Critério de conclusão:
- Qualquer desvio wire/documentação falha em CI.

### Fase 3 - Robustez operacional (offline, inbox, recovery)

Objetivo:
- Garantir operação confiável com usuários offline e recuperação determinística de estado.

Entregáveis:
- Inbox persistente com pull posterior sem perda silenciosa.
- Procedimento de backup/restore criptografado do estado do cliente.

Testes obrigatórios:
- Integração offline->online com retenção e entrega tardia.
- Testes de restart/recovery com estado íntegro.
- Negativos para estado parcial/corrompido com erro explícito.

Critério de conclusão:
- Recovery reproduzível sob churn sem violar invariantes.

### Fase 4 - Hardening final de segurança

Objetivo:
- Fechar pendências de robustez adversarial e observabilidade sem vazar segredos.

Entregáveis:
- Expansão de fuzz targets nas superfícies críticas restantes.
- Property tests de determinismo/idempotência.
- Catálogo operacional final de métricas/traces em `docs/observability.md`.

Testes obrigatórios:
- Fuzz smoke em targets críticos.
- Property tests para invariantes de epoch/cfg_hash/assinatura.
- Testes negativos para replay, reorder e metadado de tracing ausente.

Critério de conclusão:
- Entradas malformadas retornam erro explícito sem panic path.

### Fase 5 - Release readiness v1.0

Objetivo:
- Fechar gate final de release com qualidade, documentação e segurança verificadas.

Entregáveis:
- `cargo clippy` e `cargo deny` sem bloqueadores.
- `README.md` + `docs/*` com fluxo de integração e operação atualizado.
- Checklist de release validado ponta a ponta.

Testes obrigatórios:
- Execução completa de unit, integração, e2e, negativos, property e fuzz smoke.
- Regressão final do fluxo de ponta a ponta em ambiente limpo.

Critério de conclusão:
- Todos os gates verdes e sem bloqueadores críticos abertos.

### Ordem de execução imediata (próximas tarefas)

1. Executar `TASK 1` (hardening/observabilidade P2P) em subtarefas `1.1 -> 1.4`.
2. Executar `TASK 2` (conformidade MLS avançada).
3. Executar `TASK 3` (robustez adversarial com fuzz/property).
4. Rodar gate final consolidado da Fase 5.

## Backlog acionável para fechamento da v1

Esta lista contém apenas pendências **não implementadas** após revisão do estado atual em `README.md`, `docs/*` e suites de testes.
Itens concluídos foram removidos desta lista e mantidos no histórico (`CHANGELOG.md`).

---

## [TASK 1] Hardening final do runtime P2P + observabilidade operacional

Objective:
- Fechar o hardening de produção do runtime `spex-transport` com foco em cenários de operação contínua.
- Completar instrumentação operacional para diagnóstico de degradação em tempo real sem expor dados sensíveis.

Context:
- O runtime já possui perfis (`Dev/Test/Prod`), tuning básico, reputação e snapshots de estado.
- Ainda faltam cenários avançados de operação contínua e cobertura de telemetria para incidentes prolongados.

Scope:
- Arquivos/módulos que podem ser modificados:
  - `crates/spex-transport/src/*` (timers, reputação, recovery, métricas/tracing).
  - `crates/spex-transport/tests/*` (integração churn/latência/falhas).
  - `docs/observability.md`, `README.md` (parâmetros, indicadores, troubleshooting).
- O que NÃO deve ser tocado:
  - Formato wire do protocolo.
  - Invariantes criptográficos e de autenticação já definidos.

Constraints:
- Preservar determinismo e compatibilidade entre nós.
- Não registrar segredos, payloads brutos ou material criptográfico em logs/traces.
- Falhas de entrada externa devem retornar erro explícito (sem panic paths).

Acceptance Criteria:
- Perfis operacionais demonstram redução de latência sem regressão de taxa de sucesso.
- Políticas de reputação distinguem falhas transitórias de abuso recorrente com baixa taxa de falso positivo.
- Métricas/traces de publish/recovery/fallback permitem correlação operacional ponta a ponta.
- Recovery após restart em churn prolongado é validado com estado íntegro e quarentena explícita para estado corrompido.

Subtarefas incrementais obrigatórias (sem Big Bang):

### [TASK 1.1] Reputação

Status:
- ✅ Concluída com thresholds explícitos de reputação, ban determinístico para abuso recorrente e transições observáveis.

Evidência de testes (TASK 1.1):
- `cargo test -p spex-transport --test p2p_backoff_churn intermittent_peer_timeout_penalties_do_not_immediately_ban -- --nocapture`
- `cargo test -p spex-transport --test p2p_manifest_delivery recurring_invalid_payload_escalates_to_ban -- --nocapture`
- `cargo test -p spex-transport --test security_replay_tamper inconsistent_responses_escalate_probation_then_ban -- --nocapture`

- Arquivos-alvo (`crates/spex-transport/src/*`):
  - `crates/spex-transport/src/p2p.rs`
  - `crates/spex-transport/src/transport.rs`
  - `crates/spex-transport/src/telemetry.rs`
  - `crates/spex-transport/src/error.rs`
- Testes-alvo (`crates/spex-transport/tests/*`):
  - `crates/spex-transport/tests/p2p_backoff_churn.rs`
  - `crates/spex-transport/tests/security_replay_tamper.rs`
  - `crates/spex-transport/tests/p2p_manifest_delivery.rs`
- Critério objetivo:
  - Reputação diferencia peer intermitente de peer malicioso recorrente com thresholds explícitos;
  - Evidência via suíte de teste: cenário intermitente não gera ban permanente e cenário malicioso recorrente gera quarentena/ban determinístico com telemetria associada.
- Fechamento de documentação incremental:
  - Atualizar `docs/observability.md` ao concluir esta subtarefa com métricas/trace de reputação e alertas de falso positivo.

### [TASK 1.2] Recovery/Snapshot

Status:
- ✅ Concluída com status explícito de recovery/snapshot, contadores de restauração e quarentena determinística para snapshot corrompido.

Evidência de testes (TASK 1.2):
- `cargo test -p spex-transport --test planned_p2p_persistence test_snapshot_integrity_status_reports_restored_counts -- --nocapture`
- `cargo test -p spex-transport --test planned_p2p_persistence test_corrupted_snapshot_is_quarantined_with_explicit_warning -- --nocapture`
- `cargo test -p spex-transport --test p2p_manifest_recovery reassemble_rejects_partial_manifest_with_explicit_error -- --nocapture`
- `cargo test -p spex-transport --test p2p_ingest_validation rejects_malformed_base64_payload_with_explicit_error -- --nocapture`

- Arquivos-alvo (`crates/spex-transport/src/*`):
  - `crates/spex-transport/src/inbox.rs`
  - `crates/spex-transport/src/ingest.rs`
  - `crates/spex-transport/src/p2p.rs`
  - `crates/spex-transport/src/lib.rs`
- Testes-alvo (`crates/spex-transport/tests/*`):
  - `crates/spex-transport/tests/p2p_manifest_recovery.rs`
  - `crates/spex-transport/tests/planned_p2p_persistence.rs`
  - `crates/spex-transport/tests/p2p_ingest_validation.rs`
- Critério objetivo:
  - Recovery após restart com snapshot íntegro restaura estado operacional sem perda silenciosa;
  - Estado parcial/corrompido entra em quarentena explícita e retorna erro determinístico;
  - Evidência via testes de restart + persistência + validação negativa de corrupção.
- Fechamento de documentação incremental:
  - Atualizar `docs/observability.md` ao concluir esta subtarefa com sinais de integridade de snapshot, contadores de quarentena e runbook de recuperação.

### [TASK 1.3] Churn testing

Status:
- ✅ Concluída com SLOs explícitos de churn, limites objetivos de retry pressure e classificação determinística de saúde (`degraded`/`critical`).

Evidência de testes (TASK 1.3):
- `cargo test -p spex-transport --test p2p_backoff_churn -- --nocapture`
- `cargo test -p spex-transport --test dht_gossip_random_walk -- --nocapture`
- `cargo test -p spex-transport --test stress_chunking -- --nocapture`

- Arquivos-alvo (`crates/spex-transport/src/*`):
  - `crates/spex-transport/src/p2p.rs`
  - `crates/spex-transport/src/transport.rs`
  - `crates/spex-transport/src/chunking.rs`
  - `crates/spex-transport/src/telemetry.rs`
- Testes-alvo (`crates/spex-transport/tests/*`):
  - `crates/spex-transport/tests/p2p_backoff_churn.rs`
  - `crates/spex-transport/tests/dht_gossip_random_walk.rs`
  - `crates/spex-transport/tests/stress_chunking.rs`
- Critério objetivo:
  - Sob churn prolongado, publish/recovery mantêm SLO mínimo definido para sucesso e latência;
  - Backoff converge sem flapping e sem explosão de retries;
  - Evidência via testes de churn/estresse reproduzíveis com thresholds objetivos.
- Fechamento de documentação incremental:
  - Atualizar `docs/observability.md` ao concluir esta subtarefa com painéis/SLO de churn, thresholds de degradação e procedimentos de mitigação.

### [TASK 1.4] Observabilidade

Status:
- ✅ Concluída com catálogo operacional final por operação, correlação determinística com fallback explícito e checklist objetivo de readiness.

Evidência de testes (TASK 1.4):
- `cargo test -p spex-transport --test p2p_manifest_delivery publish_correlation_fallback_is_deterministic_without_inbox_context -- --nocapture`
- `cargo test -p spex-transport --test p2p_manifest_recovery reassemble_correlation_is_deterministic_for_manifest_shape -- --nocapture`
- `cargo test -p spex-transport --test p2p_ingest_property ingest_correlation_is_deterministic -- --nocapture`
- `cargo test -p spex-transport --test p2p_ingest_validation ingest_correlation_fallback_is_deterministic -- --nocapture`

- Arquivos-alvo (`crates/spex-transport/src/*`):
  - `crates/spex-transport/src/telemetry.rs`
  - `crates/spex-transport/src/p2p.rs`
  - `crates/spex-transport/src/transport.rs`
  - `crates/spex-transport/src/ingest.rs`
- Testes-alvo (`crates/spex-transport/tests/*`):
  - `crates/spex-transport/tests/p2p_manifest_delivery.rs`
  - `crates/spex-transport/tests/p2p_manifest_recovery.rs`
  - `crates/spex-transport/tests/p2p_ingest_property.rs`
  - `crates/spex-transport/tests/p2p_ingest_validation.rs`
- Critério objetivo:
  - Catálogo de métricas/traces cobre publish, recovery, fallback, reassemble e ingestão com correlação ponta a ponta;
  - Ausência de metadado de tracing gera fallback determinístico (sem vazar payload/segredos);
  - Evidência via testes de instrumentação/negativos e checklist de campos obrigatórios por operação.
- Fechamento de documentação incremental:
  - Atualizar `docs/observability.md` ao concluir esta subtarefa com catálogo final consolidado, SLOs recomendados e critérios de readiness operacional.

Tests Required:
- Testes unitários para tuning de timeout por perfil e conectividade.
- Testes de integração de reputação (peer intermitente vs. malicioso recorrente).
- Testes de persistência/recovery com restart sob churn.
- Casos negativos: estado parcial/corrompido, timeout extremo, metadado de tracing ausente.

Documentation:
- Atualizar `README.md` com recomendações operacionais finais de produção.
- Atualizar `docs/observability.md` com catálogo final de métricas/traces e SLOs sugeridos.

Versioning:
- Confirmado: `VERSION.md` deve ser incrementado quando esta task for executada.

---

## [TASK 2] Conformidade MLS avançada (epochs, commits e ressincronização)

Objective:
- Completar a suíte MLS com cenários avançados de ordenação/permutação de commits, epochs fora de ordem e recuperação parcial.
- Elevar a confiabilidade de interoperabilidade em cenários adversariais.

Context:
- A base MLS existente cobre fluxos principais de TreeKEM, updates e add/remove.
- Ainda faltam cenários extremos de conformidade e recuperação para fechamento de versão.

Scope:
- Arquivos/módulos que podem ser modificados:
  - `crates/spex-mls/tests/*` e helpers/fixtures MLS.
  - (se necessário) pontos de erro explícito em `crates/spex-mls/src/*` sem alterar design criptográfico.
  - `docs/*` e `README.md` para matriz de conformidade MLS final.
- O que NÃO deve ser tocado:
  - Design criptográfico MLS.
  - Formato de mensagens sem aprovação humana explícita.

Constraints:
- Preservar invariantes de `epoch`, `cfg_hash` e validação de assinatura.
- Rejeições de estado inválido devem ser explícitas e determinísticas.
- Garantir repetibilidade dos testes (sem flakiness).

Acceptance Criteria:
- Cobertura de cenários de permutação de commits com ressincronização consistente.
- Epoch fora de ordem e recuperação parcial inconsistente são rejeitados com erro explícito.
- Conjunto de testes MLS avançados executa de forma determinística em CI.

Tests Required:
- Testes de integração MLS para cenários avançados de commit/epoch/recovery.
- Casos negativos de reorder, replay e estado incompleto.
- Property-based tests de idempotência/determinismo quando viável.
- Fuzz targets para parsing/decoding MLS quando viável.

Documentation:
- Atualizar documentação de conformidade MLS em `/docs`.
- Atualizar `README.md` se houver impacto em garantias expostas.

Versioning:
- Confirmado: `VERSION.md` deve ser incrementado quando esta task for executada.

---

## [TASK 3] Expansão de robustez adversarial (fuzz + property tests em superfícies críticas)

Objective:
- Fechar lacunas de robustez em parsing/decoding para superfícies externas (HTTP bridge + transporte P2P).
- Aumentar proteção contra inputs malformados e regressões de panic.

Context:
- Já existem fuzz targets e testes property-based para partes do core/bridge.
- A pendência é ampliar cobertura para superfícies críticas restantes e garantir continuidade em CI.

Scope:
- Arquivos/módulos que podem ser modificados:
  - `fuzz/fuzz_targets/*` e harnesses relacionados.
  - Testes em `crates/spex-bridge/tests/*`, `crates/spex-transport/tests/*`, `spex-core/tests/*`.
  - `docs/security.md` e `README.md` (estratégia de testes adversariais).
- O que NÃO deve ser tocado:
  - Sem alterar regras de validação para “acomodar” inputs inválidos.

Constraints:
- Entradas não confiáveis nunca devem depender de panic path.
- Erros precisam ser explícitos e auditáveis.
- Fuzz/property tests devem ser determinísticos o suficiente para execução em pipeline.

Acceptance Criteria:
- Novos fuzz targets cobrem parsing crítico ainda sem cobertura.
- Property tests validam invariantes de determinismo/idempotência.
- Casos malformados retornam erro explícito sem queda do processo.

Tests Required:
- Execução de fuzz smoke tests para targets novos/atualizados.
- Testes unitários/integrados para rejeição de payloads inválidos.
- Casos negativos de truncamento, tipos inesperados e inconsistência de hash/assinatura.

Documentation:
- Atualizar `docs/security.md` com política de robustez e cobertura fuzz.
- Atualizar `README.md` com instruções de execução e escopo de robustez.

Versioning:
- Confirmado: `VERSION.md` deve ser incrementado quando esta task for executada.

---
