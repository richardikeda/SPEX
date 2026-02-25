# SPEX TODO List

## Status de fechamento v1.0 (revisado)

- ✅ `TASK 4` (readiness de release) foi concluída nesta execução com checklist, runbook e gates de CI/documentação.
- 🔒 Itens ainda pendentes para fechamento completo da v1.0:
  - `TASK 1` Hardening final do runtime P2P + observabilidade operacional.
  - `TASK 2` Conformidade MLS avançada (epochs, commits e ressincronização).
  - `TASK 3` Expansão de robustez adversarial (fuzz + property tests em superfícies críticas).

A publicação definitiva da v1.0 permanece bloqueada até conclusão validada de `TASK 1-3`.

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
