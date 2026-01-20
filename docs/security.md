# Segurança

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
