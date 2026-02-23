# Security Policy

## Supported Versions

O projeto **SPEX (Secure Permissioned Exchange)** segue versionamento semântico.

| Version    | Supported |
| ---------- | --------- |
| `main`     | ✅ Yes     |
| `v0.1.x`   | ✅ Yes     |
| `< v0.1.0` | ❌ No      |

Apenas a branch `main` e a última versão estável recebem correções de segurança.

---

## Reporting a Vulnerability

O SPEX leva segurança **extremamente a sério**.
Se você acredita ter encontrado uma vulnerabilidade, **não abra uma issue pública**.

### Como reportar

Envie um e-mail para:

**📧 [security@spex.dev](mailto:security@spex.dev)**
*(substitua pelo e-mail real antes de publicar)*

Inclua, se possível:

* descrição clara da vulnerabilidade
* componentes afetados (ex: `spex-core`, `spex-bridge`, `spex-transport`)
* impacto potencial
* passos para reprodução
* prova de conceito (PoC), se existir
* versão/commit afetado

Você receberá uma confirmação em até **72 horas**.

---

## Disclosure Process

1. O time SPEX confirma o recebimento do relatório.
2. A vulnerabilidade é analisada e classificada (impacto/gravidade).
3. Um patch é desenvolvido e revisado.
4. Uma correção é publicada.
5. Após o patch estar disponível, a vulnerabilidade pode ser divulgada publicamente de forma coordenada.

**Divulgação responsável é fortemente incentivada.**

---

## Threat Model (Visão Geral)

O SPEX foi projetado assumindo **ambientes hostis**:

* redes não confiáveis
* bridges e DHTs comprometidos
* interceptação de tráfego
* tentativas de spam e DoS
* chaves expostas ou rotacionadas
* peers maliciosos

O protocolo **não confia** em:

* transporte
* servidores
* infraestrutura intermediária

Toda segurança é garantida por:

* criptografia ponta-a-ponta
* assinaturas digitais
* hashes canônicos
* validação explícita de contexto

---

## Security Guarantees

O SPEX **visa garantir**:

* **Confidencialidade**: apenas participantes autorizados podem ler mensagens.
* **Integridade**: mensagens adulteradas são rejeitadas.
* **Autenticidade**: identidade criptográfica verificável.
* **Permissão explícita**: comunicação exige grants válidos.
* **Expiração e revogação**: acesso é limitado no tempo.
* **Resistência a spam**: PoW e limites explícitos.
* **Forward secrecy**: uso de MLS para grupos.
* **Auditabilidade**: logs e checkpoints sem vazamento de conteúdo.

---

## Non-Goals (O que o SPEX NÃO garante)

É importante ser explícito sobre limites:

* ❌ anonimato absoluto em todos os cenários
* ❌ proteção contra comprometimento total do endpoint
* ❌ imunidade a correlação de tráfego avançada
* ❌ proteção contra engenharia social
* ❌ substituição de práticas operacionais seguras

O SPEX **reduz riscos**, mas não elimina falhas humanas ou ambientais.

---

## Cryptography

O SPEX **não inventa criptografia**.

Primitivas utilizadas:

* **Ed25519** – assinaturas
* **SHA-256 / BLAKE3** – hashing
* **AEAD** (ex: AES-GCM / ChaCha20-Poly1305)
* **MLS (RFC 9420)** – comunicação em grupo
* **Argon2id** – prova-de-trabalho (anti-abuso)
* **CBOR CTAP2 canonical** – serialização estável

Mudanças nessas primitivas são tratadas como **breaking security changes**.

---

## Secure Defaults

O SPEX define **defaults seguros**, incluindo:

* parâmetros mínimos de PoW (memória ≥ 64 MiB, iterações ≥ 3)
* rejeição de grants expirados
* rejeição de mensagens com `cfg_hash` ou epoch inválidos
* tratamento de mudança de chave como evento crítico
* separação explícita entre dados confiáveis e não confiáveis

---

## Storage Security

Implementações devem:

* proteger o estado local com permissões restritas
* criptografar snapshots e exports
* detectar rollback e corrupção de estado
* evitar logs contendo plaintext sensível

Arquivos de estado **nunca devem ser publicados**.

---

## Transport Security

* Bridges HTTP devem usar **HTTPS/TLS**
* DHT e gossip são considerados **não confiáveis**
* Toda mensagem deve ser validada independentemente do transporte
* Padding e jitter são recomendados para reduzir correlação de tráfego

---

## Supply Chain Security

Recomendações:

* uso de `cargo audit`
* revisão de dependências criptográficas
* CI com `cargo clippy` e `cargo fmt`
* evitar dependências não auditadas em componentes críticos

---

## Security Updates

Correções de segurança:

* são priorizadas
* podem resultar em releases fora do ciclo normal
* são documentadas no `CHANGELOG.md`

---

## Final Notes

SPEX trata comunicação como **infraestrutura crítica**.
Segurança é uma propriedade do **protocolo**, não da infraestrutura.

Se algo parecer inseguro, provavelmente **é**.
Reporte.

---

**Secure.
Permissioned.
Explicit.**


## Robustness Testing

Para reduzir risco de DoS por parsing patológico e falhas por entradas maliciosas, SPEX adota:

- fuzzing contínuo dos limites críticos de parsing/decodificação CBOR e payloads bridge;
- property-based tests para invariantes de canonicalização CTAP2 e tratamento de input inválido;
- política de erro explícito (`Result`) em vez de `panic` para dados externos.

Execução manual recomendada:

```bash
cargo test -p spex-core
cargo test -p spex-bridge
cargo fuzz run parse_cbor_payload --manifest-path fuzz/Cargo.toml
```
