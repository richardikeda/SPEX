# SPEX Guia Unico para Usuarios pt-BR

## O que e o SPEX

SPEX significa Secure Permissioned Exchange.

E um protocolo de comunicacao segura com criptografia ponta a ponta (E2E), construido para ambientes onde o transporte pode ser nao confiavel.
SPEX nao e apenas um aplicativo. Ele define regras explicitas para identidade, permissao, validade temporal, verificacao criptografica e interoperabilidade.

## Contexto do Projeto

- Criacao: 2026.
- Autor: Richard Ikeda.
- Origem: protocolo inicialmente pensado para uso pessoal.
- Publicacao: open source para uso de terceiros e verificacao/auditoria de codigo.
- Desenvolvimento: apoio de extensive AI and testing tooling com validacao tecnica focada em seguranca e robustez.

## Por que o SPEX e assim

A ideia central do SPEX e simples: comunicacao critica precisa de contrato explicito, nao de suposicoes implicitas.

No SPEX:

- permissao e concedida de forma explicita
- mensagens sao validadas por assinatura/hash/contexto
- expiracao e revogacao sao parte do modelo
- transporte (bridge/DHT/P2P) nao e confiado como fronteira de seguranca

## Objetivo do SPEX

Entregar uma base aberta e auditavel para troca segura de mensagens e artefatos sensiveis, com:

- controle de permissao
- verificacao criptografica deterministica
- capacidade de operar em redes heterogeneas

## Arquitetura em alto nivel

- `spex-core`: tipos, CBOR canonical (CTAP2), assinatura, hash, PoW, validacoes.
- `spex-mls`: camada MLS para grupos, commits, epoch e recovery.
- `spex-transport`: P2P, chunking, manifests, recovery e fallback.
- `spex-bridge`: API HTTP para entrega/consulta, com validacao explicita.
- `spex-client`: biblioteca de alto nivel para integracoes.
- `spex-cli`: interface de referencia para operacao.

## Como usar (fluxo rapido)

1. Criar identidade local.
2. Criar e trocar card de contato.
3. Enviar request de permissao e receber grant assinado.
4. Criar thread e enviar mensagens cifradas.
5. Fazer polling de inbox local/P2P/bridge.

Comandos base:

```bash
cargo run -p spex-cli -- identity new
cargo run -p spex-cli -- card create
cargo run -p spex-cli -- card redeem --card <BASE64>
cargo run -p spex-cli -- request send --to <USER_ID_HEX> --role 1
cargo run -p spex-cli -- grant accept --request <BASE64>
cargo run -p spex-cli -- thread new --members <USER_ID_HEX>,<USER_ID_HEX>
cargo run -p spex-cli -- msg send --thread <THREAD_ID_HEX> --text "ola"
```

## Integracao com APIs

### Bridge HTTP

Endpoints principais:

- `PUT /cards/:card_hash`
- `GET /cards/:card_hash`
- `PUT /slot/:slot_id`
- `GET /slot/:slot_id`
- `PUT /inbox/:key`
- `GET /inbox/:key`

Regras importantes:

- payload binario em base64
- validacao de grant e PoW no servidor
- `ttl_seconds` dentro da politica da bridge para inbox
- TLS obrigatorio em ambiente real

Referencia completa:

- `docs/bridge-api.md`

### Via biblioteca (SDK)

Fluxo recomendado:

- montar dados em CBOR canonical
- validar request/grant antes de aceitar entrada externa
- usar APIs de envio/recuperacao do `spex-client` e `spex-transport`

Referencias:

- `docs/integration.md`
- `docs/wire-format.md`

## Boas praticas de seguranca

- nunca confiar no transporte
- validar assinatura/hash/cfg_hash/epoch
- tratar mudanca de chave como evento critico
- proteger `~/.spex/state.json` com criptografia e permissao restrita
- usar HTTPS/TLS para bridge e APIs externas

## Open Source e governanca

Documentos importantes para quem quer contribuir ou auditar:

- `README.md`
- `CONTRIBUTING.md`
- `SECURITY.md`
- `TESTS.md`
- `docs/release-v1-checklist.md`
- `docs/runbook-release-operations.md`

## Limites e nao-objetivos

SPEX nao promete anonimato absoluto em todos os cenarios e nao substitui higiene operacional.
Ele reduz risco via verificacao explicita e desenho de protocolo, mas nao elimina erro humano.

## Resumo final

SPEX foi feito para comunicacao segura com regras claras, validacao forte e auditabilidade.
A proposta e ser um protocolo serio, permissionado e explicitamente verificavel para casos sensiveis.

Secure.
Permissioned.
Explicit.
