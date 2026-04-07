# Seguranca

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

Este documento resume práticas obrigatórias e recomendações para integrações SPEX.

## CBOR canônico (CTAP2)

O SPEX exige **CBOR canonical (CTAP2)** para garantir que serialização e assinatura sejam
**determinísticas**. Isso evita divergências de bytes entre implementações, melhora a
interoperabilidade e impede que um atacante altere a representação do payload sem mudar
os campos lógicos. Todos os cards, tokens e checkpoints exportados/importados devem ser
serializados de forma canônica antes de assinar/verificar.

## Validação de cartões e mudança de chave

Cards (ex.: `ContactCard`) devem passar por validações estritas:

- Verifique a assinatura (quando presente) com a chave pública correspondente.
- Rejeite cards com campos inválidos, formatos não-canônicos ou timestamps incoerentes.
- Trate **mudança de chave** como evento crítico: compare o fingerprint da chave já
  registrada com a nova e exija confirmação explícita do usuário ou fluxo de revogação.

Mudanças inesperadas de chave podem indicar comprometimento ou ataque de substituição.

## Request/grant e permissões

- Sempre valide `RequestToken` antes de emitir um grant.
- Enforce o nível mínimo de PoW quando `requires_puzzle` estiver ativo (memória ≥64 MiB,
  iterações ≥3).
- Restrinja roles/flags com base em políticas locais e evite permissões excessivas.


## Persistência P2P e anti-eclipse

Para reduzir perda de estado e risco de eclipse no transporte P2P:

- Persista peers conhecidos, bootstrap e índices mínimos em snapshot determinístico com escrita atômica (`temp + rename`).
- Trate snapshot corrompido como entrada não-confiável: retorne erro explícito ou faça fallback seguro para store vazio.
- Aplique peer scoring com penalidades para payload inválido, timeout recorrente e resposta inconsistente.
- Isole peers com score crítico via disconnect e ban temporário para limitar influência por origem.

## TLS obrigatório

Transporte deve usar **TLS** (HTTPS) sempre que houver tráfego em rede (bridge, APIs ou
qualquer canal externo). O objetivo é proteger metadados e reduzir riscos de interceptação
ou modificação de payloads. TLS não substitui assinaturas, mas é obrigatório para reduzir
vazamento de informações e ataques ativos na camada de transporte.

## Expiração de grants

Grants devem possuir expiração (`expires_at`) sempre que possível. Regras recomendadas:

- Rejeite tokens expirados imediatamente.
- Aplique janelas curtas para permissões temporárias.
- Revogue ou reemita grants quando houver mudança de chave ou perda de confiança.

Tokens sem expiração devem ser tratados como exceção e ter revisão periódica.

## Proteção de `~/.spex/state.json`

O estado local contém chaves, contatos e threads. Proteção recomendada:

- Permissões restritas no arquivo e diretório (`chmod 600` para o arquivo e `chmod 700` para o diretório).
- Criptografia do arquivo com chave do keychain do SO; caso não haja keychain, use `SPEX_STATE_PASSPHRASE`
  para proteger o estado (o cliente não grava o arquivo sem proteção).
- Backups seguros e armazenamento fora do alcance de outros usuários do sistema.

Em ambientes compartilhados, evite armazenar o arquivo em locais expostos.

## Revogação e recuperação via log de checkpoints

O log append-only de checkpoints permite **revogação e recuperação verificáveis**:

- Publique checkpoints de chaves e declarações de revogação no log.
- Distribua o log por canais redundantes (bridge/gossip) e verifique consistência (prefixo).
- Use recovery keys registradas no log para recuperar identidades comprometidas.

Clientes devem validar o root da Merkle tree e rejeitar logs com inconsistências, garantindo
que revogações e recuperações sejam auditáveis.


## Robustez adversarial (fuzz + property tests)

A superfície de entrada externa do SPEX deve ser validada com testes adversariais contínuos:

- Execute fuzz targets para decoding/parsing crítico (CTAP2/CBOR, bridge HTTP e payloads P2P).
- Mantenha property tests para invariantes de determinismo/idempotência em validadores de grant/PoW.
- Cubra casos negativos obrigatórios: truncamento, tipos inesperados e encodings inválidos (base64/hash/assinatura).
- Toda entrada não-confiável deve falhar com erro explícito e auditável, nunca via panic path.

Comandos de smoke recomendados antes de release:

```bash
cargo test -p spex-transport p2p_ingest_property
cargo test -p spex-bridge adversarial_parsing
cargo test -p spex-core --test ctap2_cbor_vectors -- --nocapture
cargo test -p spex-transport --test p2p_manifest_recovery -- --nocapture
for target_file in fuzz/fuzz_targets/*.rs; do
  target_name="$(basename "${target_file}" .rs)"
  cargo +nightly fuzz run "${target_name}" --fuzz-dir fuzz -- -max_total_time=30 -seed=1
done
```

Cobertura adicional aplicada nesta fase:

- Fuzz target `transport_manifest_gossip_parse` para parsing/recovery de manifests de gossip em fronteira não-confiável.
- Testes adversariais reforçados para bridge (`adversarial_parsing`), transporte (`p2p_manifest_recovery`) e core (`ctap2_cbor_vectors`).

Política de fuzz smoke para release readiness:

- O pipeline deve instalar `cargo-fuzz` explicitamente no job de robustez.
- Todos os alvos em `fuzz/fuzz_targets/*.rs` devem rodar com limite determinístico (`-max_total_time=30` e `-seed=1`).
- Qualquer crash/panic em fuzzing deve falhar o job imediatamente (exit code não-zero).


## Resposta a achados de advisory (cargo-audit / cargo-deny)

Quando o pipeline sinalizar advisory, trate como incidente de segurança e siga um fluxo explícito:

1. **Triagem imediata**
   - Identifique o advisory (`RUSTSEC-xxxx-xxxx`), pacote afetado, versão vulnerável e severidade.
   - Determine exposição real no SPEX (binário, feature flag, caminho de execução).

2. **Contenção**
   - Bloqueie release enquanto o advisory estiver aberto no branch de release.
   - Se necessário, desative feature opcional dependente do pacote vulnerável até correção.

3. **Remediação**
   - Priorize atualização para versão corrigida no `Cargo.lock`/`Cargo.toml`.
   - Quando não houver patch upstream, aplique mitigação temporária documentada e registre risco residual.
   - Exceções (`ignore`) em `deny.toml` só podem ser temporárias, justificadas e com prazo de remoção.

4. **Validação**
   - Reexecute `cargo audit` e `cargo deny check` localmente e no CI.
   - Execute suites de regressão relevantes para confirmar ausência de quebra funcional.

5. **Rastreabilidade e comunicação**
   - Documente causa raiz, impacto, mitigação e hash do commit de correção.
   - Registre follow-up para remover workarounds e revisar dependências correlatas.

Critério de saída: release somente com pipeline de supply chain em verde e sem advisory aberto sem exceção formal aprovada.
