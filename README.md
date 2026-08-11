# Browser Planning Workspace

Este diretório contém apenas o planejamento e os contratos de governança de um navegador open source multiplataforma em Rust, com Tauri como shell e Servo como engine web inicial.

## Escopo desta entrega

- arquitetura e boundaries;
- pesquisa de viabilidade Tauri + Servo;
- modelo de ameaça e controles de segurança;
- estratégia de testes;
- CI/CD fail-closed e governança GitHub;
- releases, assinatura, SBOM e atualização;
- governança para agentes de IA;
- roadmap e DAG de Pull Requests pequenas.

Nenhuma funcionalidade do navegador foi implementada. Este workspace contém o baseline documental e a esteira de materialização; o estado operacional atual de Issues, Draft PRs, milestones e blockers vive em `CURRENT_STATE.md`. Enforcement externo só é considerado verificado após consulta autenticada ao GitHub.

## Documentos

1. [PROJECT_PLAN.md](PROJECT_PLAN.md) — decisão executiva, escopo, premissas, gates e critérios de produto.
2. [ARCHITECTURE.md](ARCHITECTURE.md) — arquitetura completa, engine contract, data flows, concorrência e workspace.
3. [ROADMAP.md](ROADMAP.md) — milestones, dependências, parallel tracks e exit criteria.
4. [PR_PLAN.md](PR_PLAN.md) — DAG de PRs futuras e cards de implementação.
5. [CI_CD_STRATEGY.md](CI_CD_STRATEGY.md) — workflows, quality gate, Rulesets, merge e supply chain.
6. [TESTING_STRATEGY.md](TESTING_STRATEGY.md) — pirâmide de testes, WPT, E2E, performance e regressão.
7. [SECURITY_MODEL.md](SECURITY_MODEL.md) — trust boundaries, threat model, políticas e controles.
8. [THREAT_MODEL.md](THREAT_MODEL.md) — ativos, atores, cenários STRIDE e acceptance tests de segurança.
9. [RELEASE_STRATEGY.md](RELEASE_STRATEGY.md) — canais, artefatos, assinatura, provenance e updater.
10. [AI_AGENT_GOVERNANCE.md](AI_AGENT_GOVERNANCE.md) — protocolo operacional para agentes e PRs.
11. [SOURCES.md](SOURCES.md) — fontes primárias e limites de evidência.
12. [DOCUMENTATION_PLAN.md](DOCUMENTATION_PLAN.md) — mapa canônico dos documentos futuros.
13. [docs/decisions/README.md](docs/decisions/README.md) — ADRs que devem ser ratificados antes da implementação.
14. [docs/architecture-graph.yaml](docs/architecture-graph.yaml) — manifest machine-readable de packages, edges e transições.
15. [docs/document-authority.yaml](docs/document-authority.yaml) — autoridade, presença e owners documentais.
16. [docs/gates/release-gates.yaml](docs/gates/release-gates.yaml) — critérios machine-readable e política NO_GO de releases.
17. [docs/contracts/runtime-lifecycle.md](docs/contracts/runtime-lifecycle.md) — lifecycle, fencing, cancelamento, backpressure e recovery.
18. [docs/contracts/runtime-manifest.yaml](docs/contracts/runtime-manifest.yaml) — quotas, lanes, wake, fencing, cancellation e shutdown; contrato ainda proposto.
19. [docs/contracts/engine-contract-manifest.yaml](docs/contracts/engine-contract-manifest.yaml) — contrato comum fake/Servo, non-vacuity e identidade de artefatos; ainda proposto.
20. [docs/ci/control-plane-runbook.md](docs/ci/control-plane-runbook.md) — bootstrap UNVERIFIED/OFF/SHADOW/ENFORCED e recuperação do control-plane.
21. [docs/pr-dag.yaml](docs/pr-dag.yaml) — DAG machine-readable e revalidação de dependências.
22. [CURRENT_STATE.md](CURRENT_STATE.md) — snapshot operacional de milestone, wave, blockers e próximo gate.
23. [docs/development/CRITICAL_PATH.md](docs/development/CRITICAL_PATH.md) — caminho crítico até o MVP e blockers.
24. [docs/development/EXECUTION_MAP.md](docs/development/EXECUTION_MAP.md) — milestones, Issues, Draft PRs, dependências e gates.
<<<<<<< HEAD

25. [CONTRIBUTING.md](CONTRIBUTING.md) — Definition of Ready/Done e fluxo de contribuição.
26. [SECURITY.md](SECURITY.md) — reporte privado e escopo de segurança por fase.
27. [docs/LICENSE_POLICY.md](docs/LICENSE_POLICY.md) — política de licença ainda não ratificada.
28. [docs/development/WAVE-001.md](docs/development/WAVE-001.md) — primeira wave e evidence mapping.
29. [docs/development/github-issue-map.json](docs/development/github-issue-map.json) — IDs estáveis para Issues GitHub.
30. [docs/development/github-pr-map.json](docs/development/github-pr-map.json) — IDs estáveis para Draft PRs, branches e SHAs.
31. [docs/development/STATE_SYNC.md](docs/development/STATE_SYNC.md) — estratégia de snapshot/derivação de estado a partir do GitHub.
32. [docs/agents/HERMES_VPS.md](docs/agents/HERMES_VPS.md) — contrato operacional específico do executor Hermes na VPS; não substitui as fontes normativas.
33. [docs/specs/README.md](docs/specs/README.md) — regras para especificações e acceptance criteria.
34. [docs/specs/SPEC-000-template.md](docs/specs/SPEC-000-template.md) — template de specification; não é autoridade.
35. [.github/pull_request_template.md](.github/pull_request_template.md) — contrato/checklist de PR.
36. [.github/CODEOWNERS](.github/CODEOWNERS) — ownership do mantenedor nos trust paths.
37. [docs/development/GITHUB_LABELS.md](docs/development/GITHUB_LABELS.md) — taxonomia de labels semânticas.
38. [docs/security/GITHUB_ACTIONS_SECURITY.md](docs/security/GITHUB_ACTIONS_SECURITY.md) — policy de Actions, tokens, forks, artifacts e rollout.

## Ordem de leitura

`PROJECT_PLAN.md` → `ARCHITECTURE.md` → `SECURITY_MODEL.md` → `TESTING_STRATEGY.md` → `CI_CD_STRATEGY.md` → `ROADMAP.md` → `PR_PLAN.md`.

## Regra de interpretação

Um documento de planejamento é intenção, não prova de implementação. Cada futura PR só poderá ser considerada concluída com diff real, teste executado, evidência vinculada ao SHA correto e todos os gates automatizados verdes. O MVP é experimental; Stable permanece bloqueado até o engine host separado e os gates de isolamento definidos em `docs/gates/release-gates.yaml`.
