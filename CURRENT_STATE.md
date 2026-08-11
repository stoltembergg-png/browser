# CURRENT_STATE.md — estado operacional

> Snapshot inicial da materialização: 2026-08-11T17:15:07-03:00.
> Este arquivo é operacional, não uma fonte de decisão arquitetural. Após cada mudança de Issues/Draft PRs/milestones, atualize-o ou gere uma nova versão vinculada ao SHA.

## Estado atual

- **Milestone:** M0 — Governance, repository and trust foundation.
- **Modo:** MATERIALIZATION / PLANNING-ONLY.
- **Objetivo atual:** transformar `ROADMAP.md` e `PR_PLAN.md` em milestones, Issues, labels, contratos de Draft PR e documentação operacional no GitHub.
- **Produto:** nenhum código de navegador implementado.
- **Repository:** ainda não há `origin` nem repositório remoto neste snapshot.
- **Issues:** ainda não materializadas neste snapshot.
- **Draft PRs:** ainda não materializadas neste snapshot.
- **Quality Gate:** planejado; nenhum required check deve ser configurado antes de existir e passar por canários.
- **Control-plane:** `UNVERIFIED`; sem merge/release automático.

## Ready / próxima wave

A Wave 001 será limitada a cinco contratos de planejamento:

- `PR-001` — Repository governance;
- `PR-002` — ADR and specification templates;
- `PR-003` — PR/CODEOWNERS/policy contracts;
- `PR-004` — Rust workspace skeleton;
- `PR-005` — Toolchain and dependency policy.

As Draft PRs podem ser abertas antes do merge de suas predecessoras para tornar o DAG visível, mas devem declarar `Depends on`/`Blocked until` e não são merge-eligible por isso.

## Blockers

- repository/remote alvo ainda não definido no baseline local;
- `CONTRIBUTING.md`, `SECURITY.md`, templates e `CODEOWNERS` são entregáveis planejados de M0, não evidência presente;
- ADRs de produto ainda são propostas;
- CI/Rulesets/required checks e canários GitHub ainda não foram executados;
- PRs futuras que implementam Rust/Tauri/Servo continuam fora desta execução.

## Último estágio concluído

Planejamento mestre revisado: 70 unidades `PR-001`–`PR-070`, DAG sem dependências ausentes/ciclos, gates de segurança/release documentados e contratos machine-readable presentes.

## Próximo gate

Bootstrap remoto autenticado, criação idempotente de milestones/labels/Issues, abertura de no máximo 5–10 Draft PRs planning-only e atualização deste arquivo com os números reais do GitHub.
