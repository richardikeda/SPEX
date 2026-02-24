# SPEX TODO List

## Backlog acionável

Esta lista contém apenas pendências atuais de implementação e hardening.
Itens já concluídos foram movidos para o [CHANGELOG.md](CHANGELOG.md).

---

## [TASK 1] Escrita de inbox via cliente/transporte (bridge HTTP)

Objective:
- Implementar APIs no `spex-client` e `spex-transport` para publicar envelopes via bridge HTTP (`PUT /inbox/:key`).
- Garantir serialização do envelope, cálculo de PoW e geração de grant no formato aceito pela bridge.
- Cobrir cenários end-to-end CLI↔bridge para envio HTTP com grant/PoW em integração automatizada.

Context:
- O endpoint `PUT /inbox/:key` já está implementado na bridge com validações de grant/PoW, TTL e limites de payload.
- Ainda não existe fluxo de alto nível completo no caminho cliente/transporte para publicação via bridge.

Scope:
- Arquivos/módulos que podem ser modificados:
  - `spex-client` (APIs de publicação via bridge).
  - `spex-transport` (integração HTTP para envio ao endpoint de inbox).
  - Testes de integração do fluxo CLI↔bridge.
- O que NÃO deve ser tocado:
  - Regras de segurança já existentes da bridge.
  - Formato de wire protocol e invariantes criptográficos.

Constraints:
- Preservar validação explícita de grants e permissões.
- Enforce de PoW mínimo e políticas de TTL.
- Manter comportamento determinístico de serialização.
- Não introduzir fallback implícito sem sinalização explícita.

Acceptance Criteria:
- Cliente/transport publicam envelopes via bridge com grant e PoW válidos.
- Requisições com grant inválido, PoW insuficiente ou TTL inválido falham com erro explícito.
- Integração automatizada CLI↔bridge cobre caminho feliz e falhas esperadas.

Tests Required:
- Testes unitários de serialização do envelope para payload HTTP.
- Testes unitários de geração/validação de grant e cálculo de PoW.
- Teste E2E CLI↔bridge para envio válido.
- Casos negativos obrigatórios: grant inválido, PoW abaixo do mínimo e TTL fora da política.

Documentation:
- Atualizar `README.md` com uso de publicação via bridge no cliente/transporte.
- Atualizar documentação em `/docs` para contrato HTTP, erros e requisitos de segurança.

Versioning:
- Confirmado: `VERSION.md` deve ser incrementado quando esta task for executada.

---

## [TASK 2] Hardening operacional do runtime P2P

Objective:
- Reduzir latências/esperas fixas (`publish_wait`, `manifest_wait`, `query_timeout`) com tuning por perfil.
- Expandir políticas de reputação para peers maliciosos além do ban temporário atual.
- Aprofundar persistência/recovery de estado entre execuções em cenários de churn prolongado.

Context:
- O runtime P2P já opera com manifestos/chunks, DHT, gossip, perfil de tempos, backoff e fallback para bridge.
- Ainda há pendências para hardening operacional em produção.

Scope:
- Arquivos/módulos que podem ser modificados:
  - Runtime P2P (timers, perfil, reputação, persistência e recovery).
  - Configuração de perfis operacionais e testes de churn.
- O que NÃO deve ser tocado:
  - Semântica do protocolo e formatos de wire.
  - Invariantes criptográficos e regras de validação de segurança.

Constraints:
- Preservar compatibilidade entre nós e comportamento determinístico.
- Evitar políticas de reputação que causem ban agressivo de peers honestos.
- Garantir recovery robusto sem depender de panic paths.

Acceptance Criteria:
- Timeouts por perfil reduzem latência média sem degradar taxa de sucesso.
- Reputação diferencia falhas transitórias de comportamento malicioso recorrente.
- Estado relevante é recuperado após restart em cenários de churn prolongado.

Tests Required:
- Testes unitários para tuning por perfil (`publish_wait`, `manifest_wait`, `query_timeout`).
- Testes de integração para reputação (peers maliciosos e peers intermitentes).
- Testes de persistência/recovery com reinício em churn prolongado.
- Casos negativos: estado corrompido ou incompleto deve retornar erro explícito.

Documentation:
- Atualizar `README.md` com parâmetros operacionais e recomendações de tuning.
- Atualizar `/docs` com política de reputação e estratégias de recovery.

Versioning:
- Confirmado: `VERSION.md` deve ser incrementado quando esta task for executada.

---

## [TASK 3] Observabilidade do transporte e ingestão

Objective:
- Adicionar métricas estruturadas para publish/recovery/fallback.
- Expor tracing de falhas de reassemble/verificação com correlação por operação.
- Incluir indicadores de saúde de rede para operação contínua.

Context:
- Faltam sinais operacionais completos para diagnosticar degradação de transporte e ingestão.

Scope:
- Arquivos/módulos que podem ser modificados:
  - Camadas de transporte e ingestão para instrumentação.
  - Módulos de telemetria/logging e testes relacionados.
- O que NÃO deve ser tocado:
  - Lógica funcional/criptográfica além dos pontos de instrumentação.

Constraints:
- Não vazar dados sensíveis (segredos, payloads brutos) em métricas/traces/logs.
- Correlation IDs devem ser explícitos e seguros.
- Overhead de observabilidade deve ser controlado por configuração.

Acceptance Criteria:
- Métricas de publish/recovery/fallback disponíveis e documentadas.
- Tracing de falhas de reassemble/verificação com correlação por operação.
- Indicadores de saúde de rede acessíveis para operação contínua.

Tests Required:
- Testes unitários para emissão de métricas em caminhos de sucesso e falha.
- Testes de integração para correlação ponta a ponta em tracing.
- Casos negativos: falhas sem contexto completo não devem causar panic.

Documentation:
- Atualizar `README.md` com orientação de observabilidade.
- Atualizar `/docs` com catálogo de métricas, traces e indicadores de saúde.

Versioning:
- Confirmado: `VERSION.md` deve ser incrementado quando esta task for executada.

---

## [TASK 4] Cobertura MLS de conformidade avançada

Objective:
- Expandir testes MLS com cenários avançados de permutação de commits e ressincronização.
- Incluir casos negativos adicionais para ordem de epochs e recuperação parcial.

Context:
- A integração MLS já suporta TreeKEM, commits, updates, add/remove e fluxos de ressincronização.
- A pendência é ampliar cobertura de conformidade e interoperabilidade.

Scope:
- Arquivos/módulos que podem ser modificados:
  - Suites de teste MLS.
  - Fixtures/helpers de cenários avançados de commit/epoch/recovery.
- O que NÃO deve ser tocado:
  - Design criptográfico MLS e formato de mensagens sem aprovação humana explícita.

Constraints:
- Preservar invariantes de ordem de epoch e aplicação de commits.
- Entradas externas malformadas devem retornar erro explícito (sem panic).
- Garantir determinismo e reprodutibilidade dos testes.

Acceptance Criteria:
- Cobertura ampliada para permutação de commits com ressincronização.
- Casos de epoch fora de ordem e recuperação parcial inconsistente são rejeitados explicitamente.
- Testes de conformidade executam com resultados determinísticos.

Tests Required:
- Novos testes de integração MLS para cenários avançados de commits/ressincronização.
- Casos negativos para ordem de epochs e recuperação parcial.
- Property-based tests para determinismo/idempotência quando viável.
- Fuzz targets para parsing/decoding MLS quando viável.

Documentation:
- Atualizar `/docs` com matriz de conformidade MLS coberta.
- Atualizar `README.md` se houver impacto no comportamento exposto.

Versioning:
- Confirmado: `VERSION.md` deve ser incrementado quando esta task for executada.

---

## [TASK 5] Ferramentas de operação (logs/revogação/recovery)

Objective:
- Melhorar exportação e análise de logs de abuso.
- Consolidar fluxos de revogação de chaves e gerenciamento de recovery keys para integrações externas.

Context:
- Bridge e CLI já possuem base de rate limiting/logs e comandos de recuperação/revogação.
- Ainda faltam fluxos operacionais completos para ambientes heterogêneos.

Scope:
- Arquivos/módulos que podem ser modificados:
  - Bridge/CLI para exportação de logs e comandos operacionais.
  - Fluxos de revogação/recovery e integrações externas.
- O que NÃO deve ser tocado:
  - Políticas de autenticação/autorização de forma a enfraquecer segurança.

Constraints:
- Revogação deve ser autenticada, auditável e idempotente.
- Exportação de logs deve minimizar exposição de dados sensíveis.
- Fluxo de recovery keys deve ser explícito e compatível com integrações externas.

Acceptance Criteria:
- Logs de abuso exportáveis com filtros e formato estável.
- Revogação de chaves com trilha de auditoria verificável.
- Gestão de recovery keys funcional para integrações externas previstas.

Tests Required:
- Testes unitários para exportação/filtros de logs.
- Testes de integração para revogação/recovery em cenários heterogêneos.
- Casos negativos: revogação sem permissão, chave inexistente, recovery key inválida/expirada.

Documentation:
- Atualizar `README.md` com fluxos operacionais suportados.
- Atualizar `/docs` com runbooks de abuso, revogação e recovery.

Versioning:
- Confirmado: `VERSION.md` deve ser incrementado quando esta task for executada.
