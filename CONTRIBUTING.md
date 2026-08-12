# CONTRIBUTING.md

## Escopo atual

O repositório está na fase de materialização da esteira. Nenhuma funcionalidade do navegador deve ser implementada nesta wave. Antes de trabalhar em código, consulte `AGENTS.md`, `PROJECT_PLAN.md`, `ARCHITECTURE.md`, `ROADMAP.md`, `PR_PLAN.md`, o Issue correspondente e a Definition of Ready.

## Fluxo obrigatório

1. Ler `AGENTS.md`, `CURRENT_STATE.md` e `docs/development/CRITICAL_PATH.md`.
2. Escolher uma única Issue `status:ready` ou obter coordenação explícita para uma Issue `status:blocked`.
3. Verificar `Depends on`, ADR gates, milestone e ownership.
4. Assumir a Issue com comentário/label `status:in-progress`; não trabalhar em Issue já assumida sem coordenação.
5. Usar uma branch nomeada com o ID estável, por exemplo `docs/pr-001-repository-governance`.
6. Manter uma mudança lógica por Draft PR e declarar o número real da Issue.
7. Implementar apenas o Scope da Issue; decisões abertas viram ADR, não código especulativo.
8. Executar testes locais e registrar comandos, exit codes, SHA/tree e artifacts.
9. Abrir/atualizar Draft PR até todos os gates aplicáveis passarem.
10. Só solicitar review/merge quando Definition of Done e dependências estiverem satisfeitas.

## Definition of Ready

Uma Issue só entra em trabalho ativo quando possui:

- objetivo e escopo claros;
- Out of scope explícito;
- dependências resolvidas ou sequência autorizada no DAG;
- ADRs necessárias ratificadas;
- critérios de aceite verificáveis;
- estratégia de testes e evidência definida;
- owner humano/agente declarado sem conflito.

## Definition of Done

- [ ] implementação ou artefato normativo real entregue;
- [ ] testes exigidos executados com resultados reais;
- [ ] quality gates aplicáveis verdes no SHA atual;
- [ ] security/privacy review realizado quando aplicável;
- [ ] documentação, manifests e ADRs atualizados;
- [ ] critérios de aceite atendidos;
- [ ] rollback/failure path documentado;
- [ ] review exigido concluído;
- [ ] PR mergeada somente pelo fluxo protegido.

## Regras de branch e PR

- Não fazer push direto em `main` depois do bootstrap inicial.
- Toda PR deve ser Draft até estar pronta para review.
- Toda PR deve incluir `Related Issue`, Objective, Context, Dependencies, Scope, Out of Scope, Implementation Plan, Tests, Acceptance Criteria, Security, Documentation, Rollback, Risks e CI Gates.
- Toda PR deve usar o ID estável do plano no título, por exemplo `[PR-001] ...`.
- Não rebasear/forçar a branch de outro agente; coordene antes de alterar dependências.
- Não adicionar secrets, tokens, credenciais, dumps de perfil, chaves ou connection strings.
- Falhas de CI são blockers: não desabilitar gates, mascarar skips ou converter ausência de evidência em sucesso.

## Testes e evidência

O nível de teste é definido por `TESTING_STRATEGY.md` e pelo card. Para documentação e policy, validar links, headings, schemas, fixtures negativas e consistência com os manifests. Para código futuro, aplicar RED-GREEN-REFACTOR quando houver comportamento. Evidência deve identificar repository, evento, base/head/tree SHA, run/attempt, policy revision e artifact digest quando aplicável.

## Merge e release

Nenhum agente ou modelo é reviewer, CODEOWNER, required check, bypass actor ou autoridade de merge. IA pode resumir logs, mas não aprova, altera Rulesets, publica artefatos ou relaxa gates. Releases devem reutilizar o artefato construído, testado, assinado e atestado; Stable permanece bloqueado até os gates de M8.

### Modo de mantenedor autônomo

Este repositório opera com um único mantenedor e não terá intervenções humanas adicionais. A política de revisão é, portanto, `zero approvals`, sem simular aprovação humana: nenhum bot, LLM ou agente será reviewer, CODEOWNER, required reviewer ou bypass actor. A ausência de aprovação só pode ser aceita quando o Ruleset/ADR correspondente estiver efetivamente ratificado; até lá, o estado permanece `UNVERIFIED`/`NO_GO`. O Quality Gate determinístico, a validação de segurança, os testes, a identidade exata de SHA/tree e a política de dependências continuam obrigatórios e não podem ser relaxados.

## Security reporting

Vulnerabilidades não devem ser abertas em Issue pública. Use GitHub Security Advisories quando habilitado; se o canal privado ainda não estiver habilitado, pare e registre o blocker sem publicar detalhes exploráveis. Consulte `SECURITY.md`.
