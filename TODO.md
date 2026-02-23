# SPEX TODO List

## Pendências de Implementação

Esta lista contém as tarefas pendentes identificadas durante a revisão do código e da documentação do protocolo SPEX.

### 2. Endpoint de inbox para escrita/ingestão

O `README.md` listava a criação do endpoint de inbox como item pendente. Contudo, a `bridge-api.md` e os testes de integração do crate `spex-bridge` mostram que o endpoint `PUT /inbox/:key` já existe, aceita envelopes base64, valida grant e PoW e permite definir TTL. A tarefa restante é finalizar a ingestão no cliente: o `spex-client` e o `spex-transport` já conseguem fazer scan (`GET /inbox`), mas ainda precisam de funções que publiquem envelopes via bridge usando esse endpoint.

**Tarefa:**
- Implementar funções no `spex-client` e `spex-transport` para publicar envelopes via bridge.
- As funções devem serializar o envelope, calcular PoW, gerar grant e enviar o payload conforme as regras de tamanho/TTL da bridge.

### 3. Integração MLS completa e sincronização entre dispositivos

A documentação de `docs/bridge-api.md` lista nos “próximos passos” a integração MLS completa. O `spex-client` já cria threads MLS a partir do `GrantToken`, mas ainda há trabalho para suportar ressincronizações, updates e remoções em grupos multi-membros.

**Tarefa:**
- Implementar APIs de alto nível no `spex-client` para atualizar grupos, lidar com commits externos e expor eventos de re-chaveamento para aplicações.

### 4. Runtime libp2p robusto com anti-eclipse e persistência

O `spex-transport` possui implementações de chunking, publicação de manifestos via gossipsub e armazenamento DHT, mas é necessário um runtime completo com proteção contra ataques de eclipse (rotinas de random walk robustas e peer scoring) e persistência de estado no disco.

**Tarefa:**
- Adaptar o transporte para executar como serviço contínuo.
- Persistir manifestos e pares conhecidos entre execuções.
- Implementar rotinas de detecção de peers maliciosos e random walks resilientes.

### 5. CLI e cliente integrados ao transporte real

As afirmações de “envio simulado” neste item ficaram desatualizadas. O fluxo principal de transporte já está integrado ao runtime real no caminho CLI + transporte.

**Funcionalidades já entregues (confirmadas em código e testes):**
- `msg send` já constrói envelope/manifest/chunks e publica via P2P com `publish_to_inboxes` quando há configuração de rede (`--p2p`, peers/bootstrap/listen).
- `inbox poll --p2p` já faz recuperação de payloads por manifest/gossip e busca de chunks no DHT (`recover_payloads_for_inbox`).
- Quando a recuperação P2P falha ou retorna vazio, já existe fallback para bridge HTTP (`BridgeClient::scan_inbox`) no CLI.
- O transporte já possui testes de integração cobrindo publicação/recuperação por manifest e fluxo com bridge (`p2p_manifest_delivery`, `p2p_manifest_recovery`, `two_identity_flow`).

**Gaps ainda em aberto (pendências reais):**
- **Otimizações operacionais:** redução de latência/espera fixa (`publish_wait`, `manifest_wait`, `query_timeout`), backoff mais fino e melhor tuning para cenários com churn.
- **Hardening de produção:** persistência de estado entre execuções, políticas anti-eclipse mais robustas e critérios adicionais de reputação/peer scoring.
- **Observabilidade:** métricas estruturadas de publish/recovery/fallback, tracing de falhas de reassemble e telemetria de saúde de rede.
- **Cobertura de integração CLI↔bridge em escrita:** apesar de existir `BridgeClient::publish_to_inbox`, faltam cenários end-to-end automatizados no CLI para envio HTTP com grant/PoW em ambiente de integração.

### 6. Refinamento de logs, rate limiting e validações

A bridge aplica limites por identidade e persiste logs de requisições, mas o sistema ainda precisa de ferramentas para exportar e analisar logs, revogar chaves e gerenciar recovery keys. O CLI já possui comandos para isso, mas integrações em outras linguagens devem replicar essas funcionalidades.

**Tarefa:**
- Desenvolver ferramentas para exportar e analisar logs.
- Melhorar mecanismos de revogação de chaves e gerenciamento de recovery keys.
