# CURRENT_STATE.md — estado operacional

> Snapshot GitHub da materialização: 2026-08-11T17:45:59-03:00.
> Este arquivo é operacional, não uma fonte de decisão arquitetural. Após cada mudança de Issues/Draft PRs/milestones, atualize-o ou gere uma nova versão vinculada ao SHA.

## Estado atual

- **Milestone:** M0 — Governance, repository and trust foundation.
- **Modo:** MATERIALIZATION / PLANNING-ONLY.
- **Objetivo atual:** transformar `ROADMAP.md` e `PR_PLAN.md` em milestones, Issues, labels, contratos de Draft PR e documentação operacional no GitHub.
- **Produto:** nenhum código de navegador implementado.
- **Repository:** `stoltembergg-png/browser`, branch de estado `docs/pr-001-repository-governance`, base `b6cfede…`.
- **Issues:** 70 materializadas; `PR-001`→`#1` até `PR-070`→`#70` (`docs/development/github-issue-map.json`).
- **Draft PRs:** 5 abertas em Draft, `#71`–`#75` (`docs/development/github-pr-map.json`).
- **Quality Gate:** planejado; nenhum required check deve ser configurado antes de existir e passar por canários.
- **Control-plane:** `UNVERIFIED`; sem merge/release automático.

## Issues ready

- `#1` / `PR-001` — Repository governance.

## Issues in progress

- Nenhuma Issue foi assumida para implementação.

## Ready / Wave 001

A Wave 001 será limitada a cinco contratos de planejamento:

- `PR-001` / Issue `#1` / Draft `#71` — Repository governance;
- `PR-002` / Issue `#2` / Draft `#72` — ADR and specification templates;
- `PR-003` / Issue `#3` / Draft `#73` — PR/CODEOWNERS/policy contracts;
- `PR-004` / Issue `#4` / Draft `#74` — Rust workspace contract, sem implementação;
- `PR-005` / Issue `#5` / Draft `#75` — Toolchain and dependency policy.

As Draft PRs podem ser abertas antes do merge de suas predecessoras para tornar o DAG visível, mas devem declarar `Depends on`/`Blocked until` e não são merge-eligible por isso.

## Blockers

- Draft PRs dependentes ainda não podem ser mergeadas fora da ordem declarada;
- Nenhum workflow executável/required check existe; Rulesets e branch protection permanecem ausentes;
- ADRs de produto ainda são propostas;
- CI/Rulesets/required checks e canários GitHub ainda não foram executados;
- PRs futuras que implementam Rust/Tauri/Servo continuam fora desta execução.

## Último estágio concluído

Planejamento mestre revisado e materialização inicial publicada: 70 unidades `PR-001`–`PR-070`, DAG sem dependências ausentes/ciclos, gates de segurança/release documentados, contratos machine-readable presentes e 5 Draft PRs abertas.

## Próximo gate

Review e validação da Draft PR `#71` / `PR-001`; somente depois resolver a cadeia `#72`→`#73` e `#74`→`#75`. Não iniciar implementação de Rust/browser nesta execução.
