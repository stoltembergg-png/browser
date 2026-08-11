# Critical Path — caminho crítico até o primeiro navegador funcional

## Autoridade

Este documento é uma visão operacional derivada de `ROADMAP.md`, `PR_PLAN.md` e `docs/pr-dag.yaml`. Os IDs abaixo são IDs estáveis do plano, não números do GitHub. O caminho só é comprovado quando cada predecessor tem Issue/PR, dependências resolvidas e evidência no SHA atual.

## Fundação de governança e convergência do quality gate

```text
PR-001 Repository governance → PR-002 ADR/spec templates → PR-003 policy contracts
PR-001 Repository governance → PR-004 workspace → PR-005 toolchain/dependencies
PR-003 + PR-005 → PR-006 CI trust baseline
PR-005 + PR-006 → PR-007 format/lint/docs gate
PR-005 + PR-006 → PR-008 dependency/security gate
PR-004 + PR-005 → PR-009 architecture validator
PR-007 + PR-008 + PR-009 → PR-010 Quality Gate aggregator
```

Este grafo é pré-requisito para tratar qualquer Draft PR como executável. `PR-007`, `PR-008` e `PR-009` são paralelos depois de seus predecessores, mas todos convergem em `PR-010`. A ausência de qualquer um mantém o quality gate bloqueado.

## Spine técnico até MVP

```text
PR-004 + PR-007 → PR-011 Tauri shell
PR-011 → PR-013 Servo embedding spike → PR-014 render surface spike
PR-009 + PR-013 + PR-014 → PR-015 provisional engine contract/fake
PR-015 → PR-019 domain IDs → PR-020 core lifecycle
PR-019 + PR-020 + PR-021 → PR-022 navigation state machine
PR-011 + PR-012 + PR-021 + PR-022 → PR-024 typed Tauri IPC
PR-022 + PR-023 + PR-024 → PR-025 fake-engine vertical slice
PR-014 + PR-016 + PR-025 → PR-026 real Servo/surface thin integration
PR-025 + PR-026 → PR-027 navigation controls/error UX
PR-023 + PR-025 → PR-028 crash/restart policy
PR-017 + PR-027 + PR-028 → PR-029 reference-platform MVP smoke
```

`PR-010` é o aggregator/gate operacional de `PR-007`/`PR-008`/`PR-009`, mas não é uma edge direta de implementação para `PR-011` no DAG atual. A integração real não pode iniciar antes de `PR-013`, `PR-014`, `PR-015` e os adapters/smokes correspondentes produzirem evidência. O fake engine não substitui `PR-026`.

## Predecessores paralelos que convergem no MVP

- `PR-006`, `PR-008` e `PR-009`: trust, dependency/security e architecture gates.
- `PR-016` e `PR-017`: Servo pinned smoke e matriz de OS.
- `PR-021` e `PR-023`: envelopes e lifecycle do engine host.
- `PR-028`: crash/restart policy antes de `PR-029`.
- `PR-018`: package/release skeleton, sem publicação.

Paralelismo só é permitido quando não há contrato compartilhado, migration/schema owner, engine revision, workflow trust root ou estado divergente.

## Caminho pós-MVP

```text
PR-029
  → PR-030..041 browser state/persistence
  → PR-042..048 security/privacy/threat regression
  → PR-049..056 WPT/platform/performance/stress
  → PR-057..060 diagnostics/release/update
  → PR-061/062 Alpha/Beta gates
  → PR-064..070 isolation/process/recovery
  → PR-063 Stable gate (M8)
```

`PR-063` é M8 porque depende da prova de isolamento e dos drills `PR-064`–`PR-070`; M7 prepara Alpha/Beta, não libera Stable.

## Blockers do caminho crítico

- ADRs de Servo, engine contract, concorrência, storage, IPC e process model ainda não ratificadas;
- superfície Tauri↔Servo não comprovada;
- storage/profile schema e HTTP/TLS policy ainda abertos;
- boundary entre página hostil e UI privilegiada exige fixture/negative IPC;
- CI/Rulesets/required checks reais ainda não verificados no GitHub;
- engine host separado e evidência adversarial por OS bloqueiam Stable.
