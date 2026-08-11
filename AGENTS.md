# AGENTS.md — contrato de entrada no repositório

Este arquivo é um mapa operacional curto. As regras normativas vivem nos documentos apontados abaixo; não duplique a arquitetura aqui.

## Antes de trabalhar

1. Leia `README.md`, `PROJECT_PLAN.md`, `ARCHITECTURE.md`, `ROADMAP.md`, `PR_PLAN.md` e os ADRs relevantes.
2. Leia `AI_AGENT_GOVERNANCE.md` e confirme branch/base SHA/status limpos.
3. Selecione exatamente um card do `PR_PLAN.md` com dependências satisfeitas.
4. Não implemente produto fora do card, não altere `main`, não use secrets e não trate comentário de IA como aprovação.

## Durante o trabalho

- Use RED → GREEN → REFACTOR para comportamento novo.
- Preserve boundaries: UI não conhece Servo; core não conhece Tauri; `servo-engine` não conhece `browser-core`.
- Toda entrada externa é não confiável; valide schema, tamanho, identity, capability e lifecycle.
- `unsafe` exige `SAFETY`, teste, isolamento e revisão específica.
- Toda dependência/action nova precisa de justificativa de necessidade, manutenção, licença, segurança e custo de substituição.
- Toda mudança de contrato, threat boundary, workflow ou persistência atualiza teste e/ou ADR.

## Antes da PR

- Execute os comandos e testes do card; registre resultados reais, SHA/tree, artifacts e falhas.
- Verifique `cargo fmt --check`, Clippy, testes afetados, security e architecture gates aplicáveis.
- Use o template de PR com Objective, Scope, Out, Tests, Acceptance, Risks, Rollback, Dependencies e Docs.
- Revalide o estado atual depois de qualquer resultado assíncrono ou rebase.

## Parar em vez de improvisar

Pare e registre um blocker quando o card depende de API Servo não comprovada, regra GitHub não verificada, migration insegura, secret, scope creep, falha de CI ou decisão aberta. Não relaxe gate para liberar merge.

## Fontes de autoridade

- Arquitetura: `ARCHITECTURE.md`.
- Objetivos/gates: `PROJECT_PLAN.md` e `ROADMAP.md`.
- Próximas mudanças: `PR_PLAN.md`.
- Testes: `TESTING_STRATEGY.md`.
- Segurança: `SECURITY_MODEL.md` e `THREAT_MODEL.md`.
- CI/merge: `CI_CD_STRATEGY.md`.
- Releases: `RELEASE_STRATEGY.md`.
- Operação detalhada: `AI_AGENT_GOVERNANCE.md`.
