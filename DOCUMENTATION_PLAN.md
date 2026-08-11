# DOCUMENTATION_PLAN.md — mapa normativo do futuro repositório

## Documentos canônicos

| Arquivo | Deve ser criado/atualizado por | Conteúdo | Fonte de autoridade |
|---|---|---|---|
| `README.md` | PR-001 | proposta, quickstart futuro e links | índice, não regra detalhada |
| `CONTRIBUTING.md` | PR-001/003 | branch, TDD, PR, review, evidence | `AI_AGENT_GOVERNANCE.md` para agentes |
| `SECURITY.md` | PR-001/048 | disclosure, contato e policy pública | `SECURITY_MODEL.md` interno |
| `ARCHITECTURE.md` | PR-002 e ADRs | boundaries, ownership, contracts | ADRs ratificados |
| `DEVELOPMENT.md` | PR-004/005 | toolchain, comandos locais, fixtures | CI manifest |
| `TESTING.md` | PR-007/049 | comandos e matriz prática | `TESTING_STRATEGY.md` |
| `RELEASING.md` | PR-018/059 | checklist operacional de release | `RELEASE_STRATEGY.md` |
| `DEPENDENCIES.md` | PR-005/008 | policy, inventory, exceptions, licenses | cargo-deny/audit output |
| `THREAT_MODEL.md` | PR-002/048 | assets, abuse cases, residual risk | security ADRs |
| `AGENTS.md` | PR-001 | onboarding mínimo e links | `AI_AGENT_GOVERNANCE.md` |
| `ROADMAP.md` | PR-002 | milestones/exit gates | `PR_PLAN.md` para execução |
| `CHANGELOG.md` | release PRs | mudanças por canal/versão | release manifest |
| `SOURCES.md` | research/ADR PRs | fontes e limites de evidência | nenhuma fonte substitui teste |
| `docs/document-authority.yaml` | PR-001/002 | presença, owner, autoridade e regras de ausência | este manifest + lint |
| `docs/architecture-graph.yaml` | PR-004/009 | packages, edges, fases e critérios de extração | `ARCHITECTURE.md` + ADRs ratificados |
| `docs/gates/release-gates.yaml` | PR-061/062/063 | critérios machine-readable e política NO-GO | `PROJECT_PLAN.md` + evidência do SHA |
| `docs/contracts/runtime-lifecycle.md` | PR-020/023/028 | estados, fencing, interleavings, canais e recovery | ADR-005 + testes determinísticos |
| `docs/contracts/runtime-manifest.yaml` | PR-020/023/025/028 | quotas, lanes, wake, cancellation, fencing e shutdown | ADR-005 + baseline executável |
| `docs/contracts/engine-contract-manifest.yaml` | PR-015/016/026 | contrato comum fake/Servo, non-vacuity e identidade de evidência | ADR-004 + contract suite |
| `docs/ci/control-plane-runbook.md` | PR-006/010 | bootstrap, UNVERIFIED/OFF/SHADOW/ENFORCED, kill switch e rollback | snapshot GitHub autenticado |
| `docs/pr-dag.yaml` | PR-009/010 e cada mudança do plano | DAG machine-readable, dependências expandidas e invalidação por SHA/tree | `PR_PLAN.md` + checker |
| `CURRENT_STATE.md` | materialization/operations | milestone atual, wave, blockers, Draft PRs e próximo gate | GitHub API + `PR_PLAN.md` |
| `docs/development/CRITICAL_PATH.md` | materialization/operations | caminho crítico, paralelismo e blockers até MVP | `ROADMAP.md` + `docs/pr-dag.yaml` |
| `docs/development/EXECUTION_MAP.md` | materialization/operations | mapa milestone → Issue → Draft PR → gates | `PR_PLAN.md` + GitHub mapping |
| `docs/contracts/workspace-contract.md` | PR-004 | packages/edges planejados e invariantes do workspace | `cargo metadata` futuro |

## O que não deve entrar no repositório

- `MEMORIES.md`: memória episódica do agente, potencialmente stale ou sensível; usar memória do ambiente/agente.
- `SOUL.md`: persona/instruções de runtime; não é contrato de engenharia.
- dumps de conversas, tokens, profiles, logs não redigidos ou snapshots de CI sem identity.

## Regra de não duplicação

Se dois documentos discordarem, a decisão fica bloqueada até um ADR resolver a autoridade. Não corrigir uma cópia silenciosamente. Links devem apontar para o documento canônico e toda mudança de contrato deve atualizar testes/ADR junto. Referência a arquivo ausente, owner inexistente, ADR apenas proposto ou gate sem manifest é falha de documentação, não `N/A`.
