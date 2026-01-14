# SPEX

## Visão geral

**Objetivo**: fornecer uma base aberta e modular para mensagens seguras e interoperáveis, com foco em
criptografia de ponta a ponta, auditabilidade e integridade dos dados.

**Escopo**:
- Protocolos e formatos de mensagens.
- Camadas de transporte e integração com outros componentes.
- Ferramentas e bibliotecas para desenvolvimento e testes.

**Princípios**:
- **Segurança por padrão** (criptografia, autenticidade e integridade).
- **Interoperabilidade** (interfaces claras e camadas desacopladas).
- **Simplicidade e rastreabilidade** (implementações pequenas, testes claros e documentação objetiva).

## Estado atual

Ainda não há implementação. Os componentes alvo são:
- **spex-core**
- **spex-bridge**
- **spex-transport**
- **spex-cli**

## O que falta implementar

1. **CBOR canonical (CTAP2)**.
2. **Integração MLS** (extensões).
3. **Bridge Axum** (PUT/GET + cards + validação puzzle/grant).
4. **DHT Kademlia** + anti-eclipse.
5. **CLI end-to-end**.

## Test vectors v0.1.1

Os vetores de teste da versão **v0.1.1** devem ser usados como referência para validação de
compatibilidade. É importante observar que eles assumem **CBOR canonical (CTAP2)**.
