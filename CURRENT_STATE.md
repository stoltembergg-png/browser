# CURRENT_STATE.md — estado operacional

> Snapshot operacional: 2026-08-12T12:00:00Z; confirmar SHA/PR após push.
> Este arquivo é operacional, não uma fonte de decisão arquitetural. Após cada mudança de Issues/Draft PRs/milestones, atualize-o ou gere uma nova versão vinculada ao SHA.

## Estado atual

- **Milestone:** M0 — Governance, repository and trust foundation.
- **Modo:** IMPLEMENTATION / M0 BOOTSTRAP.
- **Objetivo atual:** transformar `ROADMAP.md` e `PR_PLAN.md` em milestones, Issues, labels, contratos de Draft PR e documentação operacional no GitHub.
- **Produto:** bootstrap Cargo M0 implementado; nenhum comportamento de navegador ainda.
- **Repository:** `stoltembergg-png/browser`, branch de estado `docs/pr-001-repository-governance`, base `b6cfede…`.
- **Issues:** 70 materializadas; `PR-001`→`#1` até `PR-070`→`#70` (`docs/development/github-issue-map.json`).
- **Draft PRs:** 5 abertas em Draft, `#71`–`#75`; o mapa local contém apenas snapshots de parent verificado (`docs/development/github-pr-map.json`).
- **Quality Gate:** planejado; nenhum required check deve ser configurado antes de existir e passar por canários.
- **Control-plane:** `UNVERIFIED`; sem merge/release automático.
- **Review policy:** mantenedor autônomo / `zero approvals` ratificada pelo Ruleset `autonomous-main-protection` (`20723028`); sem simular review humano.
- **Ruleset:** ativo em `main`, exige PR, bloqueia deletion/force-push e não exige checks enquanto CI não existir; o control-plane permanece `UNVERIFIED` para merge/release automático.

## Issues ready

- `#1` / `PR-001` — Repository governance.

## Issues in progress

- `#4` / `PR-004` — Rust workspace skeleton; workspace implementado localmente, aguardando push/validação remota.

## Ready / Wave 001

A Wave 001 será limitada a cinco contratos de planejamento:

- `PR-001` / Issue `#1` / Draft `#71` — Repository governance, em progresso;
- `PR-002` / Issue `#2` / Draft `#72` — ADR and specification templates;
- `PR-003` / Issue `#3` / Draft `#73` — PR/CODEOWNERS/policy contracts;
- `PR-004` / Issue `#4` / Draft `#74` — Rust workspace skeleton, implementação M0 em andamento;
- `PR-005` / Issue `#5` / Draft `#75` — Toolchain and dependency policy.

As Draft PRs podem ser abertas antes do merge de suas predecessoras para tornar o DAG visível, mas devem declarar `Depends on`/`Blocked until` e não são merge-eligible por isso.

## Blockers

- Draft PRs dependentes ainda não podem ser mergeadas fora da ordem declarada;
- Nenhum workflow executável/required check existe; Rulesets e branch protection permanecem ausentes;
- ADRs de produto ainda são propostas;
- CI/Rulesets/required checks e canários GitHub ainda não foram executados;
- Tauri/Servo continuam fora desta execução; o workspace mínimo já existe.

## Último estágio concluído

Bootstrap Rust M0 implementado localmente: `Cargo.toml`, `Cargo.lock`, cinco packages, `cargo metadata --locked`, `cargo fmt --all -- --check`, `cargo check --workspace` e `cargo test --workspace` passaram.

## Próximo gate

Push e validação remota da Draft PR `#74` / `PR-004`; depois avançar para PR-005 e CI conforme o DAG.
