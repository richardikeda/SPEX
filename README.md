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

- Estrutura inicial do workspace e crates base.
- Componentes principais **a implementar**.

## Próximas etapas (v0.1.1)

- CBOR CTAP2 canonical.
- Integração MLS.
- spex-bridge (PUT/GET + validação puzzle/grant).
- spex-transport (DHT Kademlia).
- spex-cli (fluxo end-to-end).

## Build e testes

Quando o workspace estiver disponível, rode:

```bash
cargo test -p spex-core
```
