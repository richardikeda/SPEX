# Contributing to SPEX

Obrigado por considerar contribuir com o **SPEX (Secure Permissioned Exchange)**.

O SPEX é um projeto focado em **segurança, correção e clareza**, não em velocidade ou quantidade de features. Contribuições são bem-vindas — desde que respeitem esses princípios.

---

## Code of Conduct

Este projeto segue um código de conduta simples:

* seja respeitoso
* seja técnico
* discuta ideias, não pessoas
* críticas são bem-vindas quando fundamentadas

Comportamentos abusivos não serão tolerados.

---

## Project Philosophy

Antes de contribuir, é importante entender **o que o SPEX é e não é**:

* SPEX é um **protocolo**, não apenas uma aplicação
* segurança vem antes de conveniência
* mudanças no core exigem justificativa técnica clara
* invariantes criptográficos **não são negociáveis**

Contribuições que enfraqueçam segurança ou clareza **não serão aceitas**.

---

## What to Contribute

### ✅ Bem-vindo

* correções de bugs
* melhorias de documentação
* testes adicionais
* melhorias de ergonomia que **não enfraqueçam segurança**
* otimizações que preservem semântica
* ferramentas auxiliares (CLI, SDKs, exemplos)

### ❌ Evite

* adicionar dependências sem justificativa
* introduzir comportamento implícito ou “mágico”
* relaxar validações de segurança
* mudanças não documentadas em wire format
* features orientadas a hype

---

## Development Setup

### Requirements

* Rust stable (última versão)
* `cargo`
* Linux ou macOS recomendado

### Build & Test

```bash
cargo build --workspace
cargo test --workspace
cargo test --workspace --all-features
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

PRs com CI quebrado **não serão aceitos**.

---

## Repository Structure

Visão geral dos principais crates:

* `spex-core`
  Primitivas criptográficas, tipos, validações e invariantes.

* `spex-mls`
  Integração MLS e extensões SPEX.

* `spex-transport`
  Transporte P2P, chunking e fallback HTTP.

* `spex-bridge`
  Bridge HTTP não confiável com validações explícitas.

* `spex-client`
  SDK de alto nível para aplicações.

* `spex-cli`
  Aplicação de referência.

---

## Security-Sensitive Changes

Mudanças que afetam:

* criptografia
* serialização CBOR
* hashing
* validação de grants
* MLS / ratchet
* anti-abuso (PoW)

devem:

* ser explicitamente marcadas no PR
* incluir justificativa técnica
* incluir testes
* atualizar documentação relevante

Essas mudanças passam por revisão mais rigorosa.

---

## Commit Guidelines

* commits pequenos e focados
* mensagens claras (imperativo)
* evite commits “misc”, “wip”, “fix stuff”

Exemplos:

```
Add grant expiration validation
Harden PoW parameter checks
Document bridge inbox endpoint
```

---

## Pull Request Process

1. Fork o repositório
2. Crie uma branch descritiva:

   ```
   feature/grant-capabilities
   fix/pow-validation
   docs/bridge-api
   ```
3. Garanta que todos os testes passem
4. Atualize documentação se necessário
5. Abra o PR descrevendo:

   * o problema
   * a solução
   * impactos de segurança (se houver)

---

## Documentation Requirements

Se o PR:

* muda comportamento → atualizar docs
* muda wire format → atualizar `docs/wire-format.md`
* muda API → atualizar exemplos

Documentação não é opcional.

---

## Licensing

Ao contribuir, você concorda que:

* sua contribuição será licenciada sob **Mozilla Public License 2.0**
* você tem direito legal de contribuir o código enviado

---

## Questions and Discussion

Para dúvidas gerais:

* use issues
* seja claro e técnico
* inclua contexto

Para questões de segurança:

* **não abra issues públicas**
* siga `SECURITY.md`

---

## Final Notes

O SPEX não busca ser o maior projeto.
Busca ser **correto, auditável e confiável**.

Contribua com cuidado.

---

**Secure.
Permissioned.
Explicit.**
