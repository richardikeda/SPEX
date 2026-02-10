# Manifesto SPEX

**Secure Permissioned Exchange**

## 1. Comunicação é infraestrutura crítica

Comunicação não é um detalhe.
Não é apenas “mensagem”.
É **infraestrutura crítica** para pessoas, empresas e sistemas.

Grande parte das falhas de segurança, vazamentos de dados, fraudes e incidentes operacionais não acontece por falha de criptografia — acontece porque **a comunicação é implícita, permissiva demais e mal definida**.

O SPEX nasce do princípio de que **comunicação deve ser tratada com o mesmo rigor que código, contratos e sistemas críticos**.

---

## 2. Comunicação não é chat — é um ato

No SPEX, uma mensagem não é apenas um texto enviado de A para B.

Ela é:

* um **ato criptográfico**
* com **identidade explícita**
* **permissões claras**
* **validade temporal**
* e **contexto verificável**

Cada mensagem carrega:

* quem pode enviar
* quem pode responder
* quem pode anexar
* por quanto tempo
* sob quais regras

Comunicação sem contexto é ruído.
Comunicação sem limites é risco.

---

## 3. Segurança não é confiança — é verificação

O SPEX parte de um princípio simples:

> **Não confie no transporte. Não confie no servidor. Não confie na rede. Verifique tudo.**

Por isso:

* bridges e DHTs são considerados **não confiáveis**
* toda mensagem é validada por hash, assinatura e contexto
* mudanças de chave são eventos críticos
* expiração e revogação são obrigatórias

No SPEX, **o protocolo protege o usuário mesmo quando a infraestrutura falha**.

---

## 4. Permissão deve ser explícita, não implícita

A maioria dos sistemas assume permissões implícitas:

* “se chegou, pode responder”
* “se está no grupo, pode tudo”
* “se tem o contato, pode enviar para sempre”

O SPEX rejeita essa lógica.

Aqui:

* permissões são **concedidas explicitamente**
* são **revogáveis**
* são **limitadas no tempo**
* são **verificadas criptograficamente**

Sem permissão explícita, **não há comunicação**.

---

## 5. Anti-spam não é filtro — é custo

Filtros falham.
Listas negras falham.
Reputação centralizada falha.

O SPEX adota um modelo simples e eficaz:

* **custo assimétrico para iniciar comunicação**
* prova-de-trabalho verificável
* limites explícitos por grant

Enviar uma solicitação exige esforço real.
Receber é barato.

Spam deixa de ser um problema econômico viável.

---

## 6. Simplicidade no núcleo, flexibilidade nas bordas

O SPEX não reinventa criptografia.
Não cria algoritmos próprios.
Não depende de modismos.

Ele combina:

* primitives criptográficas bem estabelecidas
* serialização canônica
* modelos de grupo padronizados (MLS)
* transporte plugável

O núcleo é pequeno, auditável e estável.
A complexidade vive nas bordas, não no coração do sistema.

---

## 7. O transporte é um detalhe — o contrato não

SPEX funciona:

* ponto-a-ponto
* sobre DHT
* sobre bridges HTTP
* em redes confiáveis ou hostis

O transporte pode mudar.
O contrato criptográfico **não**.

Essa separação é deliberada:
**a segurança não depende do caminho que a mensagem percorre**.

---

## 8. Privacidade é controle, não anonimato absoluto

O SPEX não promete invisibilidade mágica.
Não promete anonimato total em qualquer cenário.

Ele promete algo mais realista e útil:

* **controle**
* **minimização de metadados**
* **limites claros**
* **consentimento explícito**

Privacidade não é ausência de regras.
Privacidade é **saber exatamente quais regras estão em vigor**.

---

## 9. O SPEX não é para tudo — e isso é intencional

O SPEX não tenta:

* substituir chats sociais
* competir com mensageiros casuais
* ser uma rede social
* agradar todos os casos de uso

Ele existe para:

* comunicação séria
* sistemas críticos
* ambientes corporativos
* integrações sensíveis
* contextos onde erro custa caro

Ferramentas especializadas são mais seguras que soluções genéricas.

---

## 10. Código aberto, responsabilidade compartilhada

O SPEX é open source porque:

* segurança exige auditabilidade
* protocolos fechados falham silenciosamente
* confiança se constrói com transparência

Mas código aberto não é ausência de responsabilidade.
Quem usa SPEX assume o compromisso de:

* respeitar suas invariantes
* entender seu modelo de segurança
* não enfraquecer o protocolo por conveniência

---

## 11. Comunicação como contrato

No fim, o SPEX defende uma ideia simples:

> **Comunicação é um contrato criptográfico entre partes, não um fluxo informal de dados.**

Quando tratamos comunicação com esse respeito:

* reduzimos risco
* aumentamos clareza
* protegemos pessoas e sistemas
* e criamos infraestrutura mais honesta

---

## SPEX existe porque comunicação merece ser levada a sério.

**Secure.
Permissioned.
Explicit.**
