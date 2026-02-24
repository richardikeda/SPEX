# SPEX TODO List

## Backlog acionável

Esta lista contém apenas pendências atuais de implementação e hardening.
Itens já concluídos foram movidos para o [CHANGELOG.md](CHANGELOG.md).

### 1. Escrita de inbox via cliente/transporte (bridge HTTP)

`PUT /inbox/:key` já está implementado na bridge (com validação de grant/PoW, TTL e limites de payload),
mas ainda faltam fluxos de alto nível para publicação por bridge no caminho cliente/transporte.

**Pendências:**
- Implementar APIs no `spex-client` e `spex-transport` para publicar envelopes via bridge.
- Garantir serialização do envelope, cálculo de PoW e geração de grant no formato aceito pela bridge.
- Cobrir cenários end-to-end CLI↔bridge para envio HTTP com grant/PoW em integração automatizada.

### 2. Hardening operacional do runtime P2P

O runtime P2P já opera com manifestos/chunks, DHT, gossip, perfil de tempos, backoff e fallback para bridge,
mas ainda há trabalho para endurecimento operacional em produção.

**Pendências:**
- Reduzir latência/esperas fixas (`publish_wait`, `manifest_wait`, `query_timeout`) com tuning por perfil.
- Expandir políticas de reputação para peers maliciosos além do ban temporário atual.
- Aprofundar persistência/recovery de estado entre execuções em cenários de churn prolongado.

### 3. Observabilidade do transporte e ingestão

**Pendências:**
- Adicionar métricas estruturadas para publish/recovery/fallback.
- Expor tracing de falhas de reassemble/verificação com correlação por operação.
- Incluir indicadores de saúde de rede para operação contínua.

### 4. Cobertura MLS de conformidade avançada

A integração MLS já suporta TreeKEM, commits, updates, add/remove e fluxos de ressincronização;
a pendência atual é ampliar a cobertura de conformidade/interoperabilidade.

**Pendências:**
- Expandir testes MLS com cenários avançados de permutação de commits e ressincronização.
- Incluir casos negativos adicionais para ordem de epochs e recuperação parcial.

### 5. Ferramentas de operação (logs/revogação/recovery)

A bridge e o CLI já possuem base de rate limiting/logs e comandos de recuperação/revogação,
mas ainda faltam fluxos operacionais mais completos para ambientes heterogêneos.

**Pendências:**
- Melhorar exportação e análise de logs de abuso.
- Consolidar fluxos de revogação de chaves e gerenciamento de recovery keys para integrações externas.
