# Visão geral da arquitetura

O SPEX é organizado em camadas que se complementam: tipos e formatos (core), integração MLS
(mls), transporte (transport/bridge) e ferramentas de uso (cli). A arquitetura busca modularidade
para permitir diferentes backends e integrações, mantendo wire-format estável e auditável.

## Fluxo de handshake (request/grant)

1. **Contato inicial**: o remetente compartilha um `ContactCard` (CBOR base64) ou um `RequestToken`
   (JSON base64) com o destinatário.
2. **Request**: o remetente envia um `RequestToken` ao destinatário, indicando a função/role
   solicitada.
3. **Grant**: o destinatário valida o pedido (e a identidade do emissor) e responde com um
   `GrantToken` (CBOR canonical base64).
4. **Thread MLS**: o `GrantToken` é inserido em um `ThreadConfig` para criação/inicialização
   da thread MLS.

Esse fluxo é o ponto de partida para estabelecer grupos, enviar mensagens e derivar chaves
compartilhadas. O wire-format canônico é importante para assinaturas determinísticas e
interoperabilidade entre implementações.

## Visão dos crates

- **spex-core**: tipos de dados (cards, tokens, envelopes), CBOR canonical (CTAP2), hashes,
  assinaturas e provas de trabalho (PoW) usadas para validação de requests.
- **spex-mls**: estruturas mínimas para contexto MLS + extensões SPEX e operações básicas de
  grupo/commit (baseado em mls-rs).
- **spex-transport**: chunking por hash, publicação/replicação em DHT/Kademlia, gossip, random
  walks e inbox scanning derivado de `inbox_scan_key`.
- **spex-bridge**: bridge HTTP com armazenamento SQLite (cards/slots) e validações básicas;
  pode funcionar como fallback para descoberta e inbox quando o P2P não está disponível.
- **spex-cli**: CLI de referência para identidades, cartões e fluxo básico de request/grant,
  além de operações de thread, mensagens e log append-only.

## Componentes e integrações

- **Cards e tokens**: permitem troca de identidades e autorização de participação.
- **MLS**: fornece segurança de grupo e gerenciamento de membros/epochs.
- **Log append-only**: adiciona auditabilidade e mecanismos de recovery e revogação.
- **Transporte**: desacopla a entrega (P2P, bridge HTTP, etc.) do modelo de mensagens.
