# CI control-plane bootstrap and recovery

## Status model

```text
UNVERIFIED → OFF → SHADOW → ENFORCED
```

- **UNVERIFIED:** estado inicial ou resultado de snapshot ausente/stale. É fail-closed e operacionalmente equivalente a nenhum enforcement: sem merge/release automático.
- **OFF:** nenhum auto-merge/release automático; workflows são apenas validação local/proposta.
- **SHADOW:** workflow/evaluator produz resultados, mas não autoriza merge; snapshot e canários são comparados.
- **ENFORCED:** Ruleset autenticado exige o check estável e o evaluator de elegibilidade; auto-merge/queue pode ser habilitado apenas conforme policy.

Transições permitidas: `UNVERIFIED → OFF` somente após bootstrap administrativo mínimo; `OFF → SHADOW` após snapshot autenticado e fixtures; `SHADOW → ENFORCED` somente após canário negativo atual. `SHADOW/ENFORCED → OFF` é permitido para rollback; qualquer snapshot ausente, stale, policy/evaluator alterado ou canário falho retorna a `UNVERIFIED` e invalida evidências downstream. Não existe transição direta `UNVERIFIED → ENFORCED`.

## Bootstrap fora das PRs

Antes de declarar M0 concluído, um administrador/autorizador protegido deve registrar um snapshot autenticado contendo:

- repository/default branch e Ruleset IDs/targets;
- enforcement, required status/workflow identity e up-to-date/merge queue settings;
- bypass actors e prova de que bots/LLMs não são CODEOWNERS, required reviewers ou bypass;
- permissions dos workflows, action SHAs, evaluator/manifest/policy revisions;
- canários negativos: push direto, check ausente/skipped/stale/wrong-SHA, conversa não resolvida, aprovação stale e `merge_group` quando aplicável.

Sem repositório Git e sem snapshot autenticado, o estado é `UNVERIFIED`, nunca “main protegida”. O snapshot deve carregar capture time, expiry, digest, repository/ref, Ruleset/evaluator/policy revision e identidade do canário; expirado ou divergente é inválido.

## Eligibility recheck

Imediatamente antes de auto-merge/queue, um evaluator protegido revalida repository, PR, base/head/tree, event, run/attempt, producer identities, policy/evaluator revision, required checks, reviews humanas elegíveis, conversations, mergeability e Ruleset efetivo. Mudança em workflow, evaluator, manifest, policy, CODEOWNERS, permissions, engine revision ou base invalida a elegibilidade anterior.

## Checked-in evidence contract

`docs/ci/quality-gate-manifest.json` is the versioned shape for evidence identity and required deterministic steps. `scripts/quality_gate_check.py` validates the manifest locally and in CI. This proves that the repository contract is present and fail-closed; it does **not** prove that GitHub required-check enforcement is active. That claim still requires an authenticated snapshot and negative canaries.

## Kill switch e rollback

O runbook de emergência deve poder, fora do fluxo de PR:

1. desligar auto-merge e Merge Queue;
2. congelar release/update;
3. manter required check fail-closed;
4. restaurar evaluator/manifest/workflow/CODEOWNERS/Ruleset para revisão protegida;
5. invalidar evidências anteriores;
6. executar canário negativo;
7. reabrir em SHADOW antes de ENFORCED.

O kill switch não pode tornar um required check opcional. Break-glass só pode desligar automação, congelar release/update, restaurar last-known-good, invalidar evidência ou retornar a `SHADOW`; nunca pode publicar Stable, bypassar required check ou aprovar ausência de processo, assinatura ou provenance. Exige dual control, auditoria imutável, expiry e revogação. Cada ação registra actor autenticado, timestamp, motivo, SHA/Ruleset e evidência redigida.
