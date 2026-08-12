# STATE_SYNC.md — estratégia de estado operacional

## Objetivo

Manter `CURRENT_STATE.md` útil sem transformá-lo em uma segunda fonte de verdade. Milestones, Issues, labels, PRs, checks e branches devem ser lidos do GitHub autenticado; decisões, DAG e arquitetura continuam nos documentos normativos.

## Fonte e snapshot

- Fonte live: GitHub API do repositório, com `repository`, default branch, milestones, Issues, labels, PRs, reviews e checks.
- Fonte local: `PR_PLAN.md`, `docs/pr-dag.yaml`, `ROADMAP.md`, `docs/development/CRITICAL_PATH.md`.
- Snapshot: `CURRENT_STATE.md`, `github-issue-map.json` e `github-pr-map.json` registram o momento consultado, SHA/tree e limitações.
- Um snapshot não pode aprovar merge, release, enforcement ou readiness se divergir do estado live.

## Workflow futuro

Depois de `PR-006`/`PR-010` existirem, planejar um workflow `state-sync.yml` com:

- `workflow_dispatch` autenticado e execução agendada de baixa frequência;
- permissões mínimas, sem secrets de produto e sem `pull_request_target` sobre código não confiável;
- leitura de Issues/milestones/labels/PRs/checks e validação contra `docs/pr-dag.yaml`;
- geração determinística, ordenada e redigida de `CURRENT_STATE.md`;
- artifact com repository, event, run/attempt, base/head/tree SHA, evaluator revision e digest;
- PR automatizada para mudanças de estado, nunca push direto em `main`;
- falha fechada se API indisponível, identidade divergente, DAG inválido, Issue duplicada, Draft ausente ou check stale;
- nenhum fechamento/reabertura/label mutation automático sem policy e canário explícitos.

O workflow futuro deve separar observação de autoridade: publicar um snapshot não altera status de Issue, Ruleset ou release.

## Reconciliation manual atual

Até o workflow existir, o agente responsável deve:

1. consultar o GitHub live;
2. comparar números/estados com o mapa local;
3. verificar heads e bases das Draft PRs;
4. atualizar snapshot em branch/PR, nunca fingir que o arquivo é live;
5. reportar divergências como blockers.

Nenhuma falha de consulta pode ser convertida em `N/A` ou em sucesso.
