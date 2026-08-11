# CI_CD_STRATEGY.md — esteira fail-closed

## 1. Objetivo e autoridade

A esteira deve impedir merge quando qualquer evidência necessária estiver ausente, stale, skipped, cancelled, timed out, neutral, duplicada, malformada ou vinculada a outro SHA/tree/evento. Um check verde isolado prova apenas o producer naquele identity; não prova que o GitHub o exige. A autoridade final é a combinação de quality gate determinístico + GitHub Ruleset/Branch Protection. Reporter de IA é apenas explicação.

GitHub Rulesets oferecem regras e enforcement que deverão ser consultados e canarizados quando o repositório existir.[12][13] Se Merge Queue for habilitado, o workflow requerido também deve lidar com `merge_group`; resultado de `pull_request` não é reutilizado para uma árvore sintética diferente.[14][15]

## 2. Topologia proposta

```text
pull_request / push main / merge_group
          │
          ▼
  CI / Quality Gate (required)
    ├── PR metadata/spec policy
    ├── toolchain + locked dependency resolution
    ├── fmt/lint/docs/architecture
    ├── unit + integration
    ├── security/dependency/supply chain
    ├── build + platform smoke
    ├── E2E/WPT applicability
    └── bounded producer attestations
          │
          ▼
  QA / Merge Eligibility (required after control-plane enforcement)
          │ exact head/base/tree/policy/review state
          ▼
  GitHub Ruleset / Merge Queue / native Auto-Merge
```

Workflows internos podem ser separados para observabilidade, mas apenas um conjunto allowlisted alimenta o check estável. Não tornar cada job matricial um required context evita drift e checks pendentes durante refactors.

## 3. Workflows

| Workflow | Trigger | Conteúdo | Required? |
|---|---|---|---|
| `ci-quality-gate.yml` | PR, protected push, `merge_group` se habilitado | chama reusable workflows, valida manifest/evidence e agrega | sim, nome estável `CI / Quality Gate` |
| `ci-static.yml` | PR/push | fmt, clippy, `unwrap_used`/`expect_used`/`panic` policy, rustdoc, metadata, architecture-check, actionlint | producer do QG |
| `ci-tests.yml` | PR/push | unit, contract, integration com fixtures | producer do QG |
| `ci-platform.yml` | PR e push conforme applicability | build/smoke Linux/Windows/macOS | producer do QG quando impactado; baseline sempre |
| `ci-e2e.yml` | PR quando UI/core/engine muda, nightly | desktop smoke, local HTTP/HTTPS, critical flows | producer; N/A só por policy protegida |
| `ci-security.yml` | PR sempre, nightly | cargo-deny/audit, CodeQL, secret scanning, dependency review, unsafe policy | producer do QG |
| `ci-wpt.yml` | engine/core changes, nightly, release | WPT subset/full selected + expectations | producer de compatibilidade |
| `ci-performance.yml` | nightly, release candidate | startup/navigation/tab/memory benchmarks | informativo até budget ratificado; bloqueia release após ADR |
| `ci-integrity.yml` | PR em `.github`, `Cargo.toml`, policy, CODEOWNERS | action pinning, permissions, workflow schema, manifest self-test | required para trust-boundary changes |
| `release.yml` | protected tag/manual environment | build, sign, SBOM, attest, publish, verify | release only |
| `nightly.yml` | schedule | stress, fuzz corpus, WPT, dependency refresh analysis | não autoriza merge |
| `security-report.yml` | workflow_run/protected | relatório redigido opcional | nunca required/authoritative |

## 4. Quality Gate contract

O manifest é versionado e protegido. Cada producer emite uma attestation bounded com:

```text
repository, PR, event, base_ref/base_sha, head_sha, tested_tree,
workflow path/revision, run_id/attempt, producer/check identity,
policy/manifest revision, conclusion, artifact id/digest, captured_at
```

O agregador:

1. lê somente producers allowlisted;
2. valida schema, repository, event, SHA/tree/base, workflow revision e digest;
3. distingue `status` de `conclusion`;
4. rejeita missing/duplicate/malformed/oversized/stale/mismatched;
5. aplica applicability policy protegida, nunca labels ou script editável pela PR;
6. retorna sucesso somente quando todas as categorias esperadas têm `SUCCESS` explícito ou N/A comprovado;
7. executa com `always()` para transformar falha de dependency em evidence `BLOCKED`, não em skip verde;
8. ignora completamente output de IA.

### Categorias mínimas

- metadata/PR policy/spec traceability;
- locked toolchain/dependencies;
- format/lint/docs;
- architecture dependency graph;
- unit/contract/integration;
- platform build/smoke;
- security/dependency/secrets/SAST;
- build artifact;
- critical E2E/WPT quando aplicável;
- performance/stress conforme milestone policy.

## 5. Forks, secrets e untrusted code

- PRs de fork executam sem secrets de release, provider, signing, telemetry ou modelos.
- Não usar `pull_request_target` para checkout/executar código da PR.
- Workflows privilegiados `workflow_run` consomem somente artifacts bounded e identity-validated; não fazem checkout da branch não confiável.
- PR title/body, diff, logs e artifacts são dados não confiáveis; não são instruções para shell, prompt ou policy.
- `GITHUB_TOKEN` default `contents: read`; permissões elevadas somente no job que precisa e com environment protegido.
- Actions de terceiros pinadas por full commit SHA, com comentário de versão e allowlist.
- Caches não podem ser compartilhados de forma que código de PR injete conteúdo em release.

## 6. GitHub Ruleset e merge

### Política `main`

- nenhum push direto;
- PR obrigatória;
- force-push e branch deletion proibidos;
- required check estável `CI / Quality Gate`;
- branch up to date ou Merge Queue comprovada;
- conversations resolvidas;
- rules/workflows/CODEOWNERS protegidos por review apropriado;
- review policy deve ser explícita: aprovação de autor, bot ou agente não conta; aprovação stale é descartada; qualquer novo commit invalida a elegibilidade anterior; paths cobertos por CODEOWNER exigem aprovação humana elegível quando essa capacidade existir;
- se não houver segundo humano elegível no repositório solo, não simular aprovação com bot: usar a política de zero approvals somente quando ratificada por ADR/Ruleset, mantendo o Quality Gate determinístico como autoridade técnica;
- bypass vazio para operação normal; break-glass separado, temporário e auditado;
- signed commits exigidos somente se os atores de automação suportarem a regra sem deadlock; essa condição nunca relaxa assinatura de artefato, provenance, attestation ou verificação de publicação;
- branch deletion automática apenas após merge e policy explícita.

### Estratégia de merge

Recomendação: **native GitHub Auto-Merge com squash** para PRs normais, após a proteção e o gate estarem realmente provados. Se Merge Queue estiver disponível e testada, ela é preferível para combinar PRs concorrentes e validar a árvore sintética. Não implementar controller próprio no início.

O merge commit é rejeitado como padrão porque acumula histórico de integração ruidoso; rebase merge exige que todos os commits sejam revisáveis e assináveis e aumenta custo operacional. Squash mantém `main` linear e cada PR lógica/reversível.

Auto-merge não é permitido para mudanças em quality gate, Rulesets, permissions, secrets, CODEOWNERS, release authority ou controller até existir root of trust protegido + fixtures adversariais + canário autenticado. Nessas paths, o resultado automático deve ser `OFF/SHADOW`, não bypass.

### Control-plane bootstrap

O estado operacional é `UNVERIFIED → OFF → SHADOW → ENFORCED`, conforme [`docs/ci/control-plane-runbook.md`](docs/ci/control-plane-runbook.md). A política local, um YAML válido ou um teste em `act` nunca provam proteção efetiva. Antes do primeiro auto-merge ou release automatizado, um bootstrap fora do fluxo de PR deve validar Ruleset, required check, bypass actors, CODEOWNERS, permissions, evaluator revision e canários negativos em um snapshot autenticado. Sem esse snapshot o estado permanece `UNVERIFIED` e toda decisão de merge/release automático é `NO_GO`.

Mudança em workflow, evaluator, policy, manifest, CODEOWNERS, permissions, base/head/tree, engine revision ou Ruleset invalida evidências downstream. O evaluator protegido deve revalidar a identidade completa imediatamente antes da elegibilidade. Bots/LLMs nunca são required reviewers, CODEOWNERS ou bypass actors.

## 7. CI self-test

O gate e o parser terão fixtures para:

- all-success;
- producer failure, missing, skipped, cancelled, timeout, neutral, unknown;
- duplicate/malformed/oversized evidence;
- wrong repository/run/event/SHA/base/tree/digest;
- stale A após B;
- classifier mixed/docs-only/unknown/error;
- fork com metadata hostil e nenhum secret;
- reporter outage/duplicate/stale;
- `merge_group` se habilitado;
- policy/evaluator revision changed.

Use `actionlint` e `act` apenas como smoke local; enforcement real exige snapshot autenticado e canários no GitHub. Local YAML válido nunca prova branch protection. Falha do control-plane não pode ser convertida em `N/A`; mantém o sistema em `OFF`/`SHADOW`.

## 7.1 Sequência de ativação

Os controles não entram como um mega-gate no primeiro bootstrap. A ordem mínima é:

1. `OFF/SHADOW`: branch policy documentada, action pinning, permissões mínimas, compilação, fmt/lint, testes contratuais e smoke local; somente estes checks determinísticos podem bloquear o bootstrap.
2. Após um artefato reproduzível: SBOM e attestation vinculados a digest, repositório, ref, workflow, run e identidade esperados; positive e negative fixtures passam no parser.
3. Após a matriz de packaging: assinaturas por canal/OS, verificação fora do runner e release canary protegido.
4. Somente quando os checks forem rápidos, estáveis e autenticados: required check efetivo, canário de bypass e Merge Queue/native Auto-Merge em `ENFORCED`.

WPT amplo, performance budgets, fuzz/soak e matrizes caras permanecem lanes versionadas até possuírem baseline, owner, duração e política de regressão. Isso não relaxa segurança: capability/CSP lint, negative IPC, dependency policy e integrity checks continuam bloqueantes quando aplicáveis.

Comparative performance budgets podem permanecer informativos antes de ADR; safety floors não. Desde M2, bounded queue admission, command timeout, frame/input progress, shutdown boundedness, ausência de starvation sob carga mínima e falha explícita de watchdog são gates bloqueantes quando aplicáveis.

## 8. Matrix de execução

| Mudança | Linux PR | Windows PR | macOS PR | Nightly | Release |
|---|---:|---:|---:|---:|---:|
| Cargo/core/domain | sim | smoke/build | smoke/build | full stress | full |
| `servo-engine`/render/platform | sim | sim | sim | WPT/full matrix | full |
| frontend/Tauri | sim E2E | sim E2E | sim E2E | visual/accessibility | full |
| `.github`, policies, Cargo locks | sim + integrity | — | — | canary | block |
| security/privacy/permissions | sim negative | smoke negative | smoke negative | fuzz/soak | block |
| docs-only | metadata + policy | — | — | — | — |

“Docs-only” é uma classificação protegida e conservadora: mudança mista, renomeada, generated, submodule, test deletion ou classifier failure executa suite completa ou bloqueia.

## 10. Control-plane recovery

O kill switch pode desligar auto-merge/queue e congelar release/update, mas nunca torna o required check opcional. O rollback restaura evaluator, manifest, workflow, CODEOWNERS e Ruleset para uma revisão protegida, invalida evidências anteriores, executa canário negativo e reabre em `SHADOW` antes de `ENFORCED`. Cada ação registra actor autenticado, motivo, SHA/Ruleset e evidência redigida.

## 11. Releases e artifacts

O workflow de release deve ser separado de PR validation e usar protected environment. Ele produz installers por OS, checksums, SBOM, provenance/attestation, signatures e metadata de canal. GitHub artifact attestations fazem parte do desenho de provenance, mas a assinatura do produto e a verificação no cliente são decisões próprias.[16]

Nenhuma attestation é prova por si só: um gate independente deve rejeitar ausência, signer/workflow/ref/repository incorretos, digest divergente, artifact substituído e attestation stale. A mesma identidade deve aparecer no artifact, no relatório de testes, na assinatura e na evidência de publicação.

## 12. Reporter de IA

Se houver reporter:

- recebe somente evidência estruturada, limitada e redigida;
- revalida SHA/PR/run antes de comentar;
- atualiza um comentário marcador, sem spam;
- rotula hipóteses como hipóteses;
- nunca diz `APPROVED`, `READY TO MERGE`, nunca é CODEOWNER e nunca entra no manifest requerido;
- outage ou prompt injection do reporter não altera nenhum status.

## 13. Sources

[12] [Rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets) · [13] [Available rules](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets) · [14] [Merge Queue](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue) · [15] [`merge_group`](https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#merge_group) · [16] [Artifact attestations](https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations) · [17] [Security hardening](https://docs.github.com/en/actions/security-for-github-actions/security-hardening-your-deployments) · [18] [Secret scanning](https://docs.github.com/en/code-security/secret-scanning/introduction/about-secret-scanning) · [19] [Code scanning](https://docs.github.com/en/code-security/code-scanning/introduction-to-code-scanning/about-code-scanning) · [20] [Dependabot](https://docs.github.com/en/code-security/dependabot/dependabot-security-updates/about-dependabot-security-updates).

## Sources

[12] https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets
[13] https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets
[14] https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue
[15] https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows
[16] https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations
[17] https://docs.github.com/en/actions/security-for-github-actions/security-hardening-your-deployments
[18] https://docs.github.com/en/code-security/secret-scanning/introduction/about-secret-scanning
[19] https://docs.github.com/en/code-security/code-scanning/introduction-to-code-scanning/about-code-scanning
[20] https://docs.github.com/en/code-security/dependabot/dependabot-security-updates/about-dependabot-security-updates
