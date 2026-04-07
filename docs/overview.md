# Visao geral da arquitetura

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

O SPEX é organizado em camadas para garantir modularidade e interoperabilidade entre
implementações. A separação por crates permite evoluir a especificação de dados e de
transporte sem quebrar o wire format ou o fluxo de autenticação.

## Camadas principais

1. **Core (tipos e formatos)**
   - Define tipos canônicos (cards, tokens, envelopes, log) e regras de serialização.
   - Garante CBOR canonical (CTAP2) para assinaturas determinísticas.
2. **Segurança e identidade**
   - Assinaturas Ed25519, hashes, prova de trabalho (PoW) e validações de grants.
   - Log append-only com Merkle tree para checkpoints e revogações.
3. **MLS e mensagens**
   - Criação de threads, commits e cifragem baseada em MLS.
4. **Transporte**
   - Chunking por hash, DHT/Kademlia, gossip e inbox scanning.
   - Bridge HTTP como fallback para descoberta e inbox.
5. **Ferramentas**
   - CLI e bibliotecas de integração para fluxo request/grant.

## Visão dos crates

- **spex-core**: tipos de dados (cards, tokens, envelopes, log), CBOR canonical (CTAP2), hashes,
  assinaturas, provas de trabalho e validações.
- **spex-mls**: integração MLS (mls-rs) com APIs `MlsRsClient`/`MlsRsGroup` para TreeKEM completo, commits, updates, add/remove, ressincronização via external commit, validação de `cfg_hash`/`proto_suite` e extensões SPEX.
- **spex-transport**: chunking, publicação/replicação DHT, gossip com recuperação de manifestos, reassemblagem com verificação de hash, random walks robustos e inbox scanning.
- **spex-bridge**: bridge HTTP com armazenamento SQLite e validações de rate limit/PoW.
- **spex-cli**: CLI de referência para identidades, cartões, fluxo request/grant e mensagens.
- **spex-client**: biblioteca de alto nível para integrações.

## Fluxo de handshake (request/grant)

1. **Contato inicial**: o remetente compartilha um `ContactCard` (CBOR base64) ou recebe um
   `InviteToken` embutido no card.
2. **Request**: o remetente envia um `RequestToken` (JSON base64) ao destinatário.
   - Se o `InviteToken` exigir PoW, o request inclui o puzzle resolvido.
3. **Grant**: o destinatário valida o request, confirma identidade e emite um `GrantToken`
   **assinado** (CBOR canonical base64).
4. **Thread MLS**: o grant é usado para inicializar o `ThreadConfig`, permitindo criação da
   thread MLS e envio de mensagens seguras.

Esse fluxo estabelece autorização mínima antes de criar grupos MLS ou publicar mensagens.

## Persistência local

O CLI persiste chaves, contatos e threads em `~/.spex/state.json` (ou `SPEX_STATE_PATH`).
O arquivo é criptografado com uma chave no keychain do SO; quando o keychain não está disponível,
defina `SPEX_STATE_PASSPHRASE` para manter o estado protegido (o cliente não salva o arquivo sem
proteção).

## Fingerprints

Sempre que um card é importado, o CLI exibe o fingerprint da chave pública para verificação
manual. Mudanças inesperadas de fingerprint devem ser tratadas como evento crítico e exigir
confirmação explícita do usuário.

## Transporte e TLS

Toda integração externa (bridge HTTP, APIs ou serviços terceiros) deve usar TLS. A criptografia
em trânsito protege metadados e evita adulteração de payloads, complementando (mas não substituindo)
as assinaturas e validações do SPEX.
