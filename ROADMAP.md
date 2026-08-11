# ROADMAP.md — Milestones e critérios de saída

## Como ler

Milestone não é uma promessa de data. É um conjunto de contratos que pode ser executado em PRs pequenas. Cada saída depende de evidência do SHA atual, não de uma caixa marcada em documentação. A integração Servo permanece condicionada à evidência da documentação/API observada e à revisão fixada [9][10]; a validação web usa WPT progressivo [21][22]. O DAG detalhado está em [PR_PLAN.md](PR_PLAN.md) e sua representação machine-readable em [`docs/pr-dag.yaml`](docs/pr-dag.yaml).

## Parallel tracks

```text
Track A Governance/CI ───────┐
Track B Tauri/Surface ───────┼─> M2 single-tab vertical slice
Track C Core/Engine contract ┘
M2 ──> Track D tabs/data ──> Track E security/privacy
                    └──────> Track F WPT/platform/performance
M5 + M6 ──> Track G release/channels ──> Alpha/Beta/Stable gates
M6 + threat evidence ──> Track H multiprocess evolution
```

Parallelismo é permitido apenas quando não há contrato semântico compartilhado, migration/schema owner, workflow trust root, engine revision ou estado que possa divergir. File disjointness sozinha não prova segurança para paralelo.

## M0 — Governance, repository and trust foundation

**Objetivo:** fazer o repositório ser operável por humanos e agentes antes do produto.

**Inclui:** governance docs, ADR template, CODEOWNERS, PR policy, Cargo workspace vazio porém compilável, toolchain/lockfile/dependency policy, action pinning, format/lint/test/security/architecture quality gate.

**Entrada:** diretório vazio.

**Saída obrigatória:**

- control-plane documentado em `OFF`, `SHADOW` ou `ENFORCED`; sem snapshot autenticado o estado é `UNVERIFIED`, não “main protegida”;
- `CI / Quality Gate` definido como check estável;
- workflows sem secrets em PR/fork;
- dependências e actions com política explícita;
- `xtask architecture-check` com fixture negativa e manifest em `docs/architecture-graph.yaml`;
- authority manifest documental e lint que bloqueia referências ausentes/ADRs não ratificados;
- documentação de como um agente começa, valida e cria uma PR.

**Não inclui:** Servo, Tauri, UI funcional ou browser behavior.

## M1 — Feasibility and shell/engine contract

**Objetivo:** provar a superfície de integração e criar o contrato substituível.

**Spikes obrigatórios:**

- Servo revision pinned e build reproducível;
- `EventLoopWaker`/`spin_event_loop` com o event loop escolhido;
- `RenderingContext` e apresentação de frame;
- input/resize;
- Tauri shell e surface composition em Windows/Linux/macOS;
- fake engine usando o mesmo contract;
- crash/hang/close lifecycle mínimo.

**Saída:** decisão ADR-002/003/004/005 ratificada ou architecture pivot explícito; o contrato de engine continua provisório até que a surface e o modelo de execução sejam comprovados.

**Falha:** se uma única janela não for comprovada, o plano adota a alternativa de surface nativa coordenada/child window sem acoplar o core.

## M2 — Browser core and MVP vertical slice

**Objetivo:** uma navegação real de ponta a ponta, pequena e observável na plataforma de referência; os demais OS ficam em build/feasibility até a matriz de suporte.

**Inclui:** domain IDs, core actor, typed bridge, engine host, single tab, navigation policy inicial, render/input, back/forward/reload/stop, error UI, crash state e capability/CSP/negative-IPC boundary mínima.

**Saída:** MVP definido em `PROJECT_PLAN.md` passa na plataforma de referência, com build/feasibility explícito nos demais OS, fixture HTTP/HTTPS local, E2E e logs redigidos.

**Bloqueadores:** surface não comprovada, event loop bloqueado, commands sem validação, capability/CSP/negative-IPC ausentes, engine types vazando para core, teste que só renderiza shell sem página.

## M3 — Browser state and persistence

**Objetivo:** transformar a tracer bullet em browser state durável.

**Inclui:** tab domain/manager, tab strip, popup policy, sessions, profile lock, storage schema/migrations, history, bookmarks, downloads e recovery.

**Saída:** reiniciar o app preserva somente o que o contrato de sessão define; corruption/kill/lock tests são negativos e não perdem perfil silenciosamente.

## M4 — Security and privacy by default

**Objetivo:** fechar as superfícies de abuso antes de chamar o produto de Alpha.

**Inclui:** scheme/navigation/file policy, permissions, Tauri capabilities/CSP, IPC abuse tests, download security, privacy clearing/partition, redacted diagnostics e threat model review.

**Saída:** threat scenarios críticos têm controle implementado e teste; residual risks estão nomeados; o modo single-process é in-process/thread-affine, não boundary de segurança, e impede qualquer claim de sandbox/site isolation.

## M5 — Compatibility, platform, performance and stress

**Objetivo:** medir comportamento real em vez de declarar “multiplataforma” por compilação local.

**Inclui:** WPT pinned subset, expectations/triage, platform input/accessibility, Windows/Linux/macOS packaging smoke, performance baseline, stress, fuzz e soak.

**Saída:** cada OS tem artifact e smoke verificável; WPT expectations têm owner/issue/status; performance manifest deriva de dados; nenhuma comparação visual cross-OS assume pixels idênticos.

## M6 — Release engineering

**Objetivo:** construir, assinar, atestar, publicar e atualizar sem confiar em arquivo manual.

**Inclui:** versioning/changelog, package matrix, SBOM, provenance, signing, release channels, updater metadata/signature, stop/last-known-good e compromise recovery.

**Saída:** release canary em ambiente protegido, artefato verificável fora do runner, update fail-closed e recovery drill documentado.

## M7 — Alpha and Beta release gates

### Alpha gate

M0–M6 verdes conforme `docs/gates/release-gates.yaml`; MVP + tabs/sessions/profiles/history/bookmarks/downloads/permissions; WPT baseline; security regression suite; signed alpha; crash diagnostics; open high risks list.

### Beta gate

Alpha exit; M5 e M6; no critical unexplained security regression; stress/soak; update recovery; platform matrix; documentation/support; release evidence tied to exact commit.

## M8 — Isolation and Stable decision

Beta exit e M8 isolation exit são pré-requisitos do Stable gate `PR-063`; engine host separado do browser core, launch restrictions por OS, cenário de compromise cross-origin/renderer, recovery e drills; critical workflows reproducible; profile migrations testadas; signed/provenanced artifacts; measured performance budgets; WPT/conformance gaps triados; security claims scoped to proven capabilities; support/rollback/incident runbooks exercitados. Ausência do processo separado não é exceção editorial: o produto permanece Beta/experimental.

M8 não é pré-requisito do MVP nem do Alpha, mas é pré-requisito do Stable decision. Enquanto M8 não estiver concluído, nenhuma documentação pode chamar o produto de production browser, sandboxed ou site-isolated.

- engine host process e versioned local IPC;
- process manager/sandbox prototype por OS;
- renderer/site isolation evaluation;
- production engine-host rollout with feature/rollback path;
- network/GPU split somente se evidência justificar;
- second engine adapter usando contract suite;
- DevTools/extensions apenas com capability/threat model próprios.

A entrada e a saída de M8 exigem ADR-008, threat model atualizado, performance evidence, cross-origin compromise scenario e evidência versionada por OS. PR-069 deve concluir o rollout de produção do engine host e PR-070 repetir os drills de recovery antes de liberar Stable.

## Exit checklist de milestone

- [ ] Todos os PRs do milestone têm diff real e foram integrados por branch protegida.
- [ ] Quality Gate passou no SHA/tree corretos.
- [ ] Testes negativos e falhas de infraestrutura não foram convertidos em N/A.
- [ ] Riscos residuais foram atualizados com owner e próximo gate.
- [ ] ADRs/Docs refletem a implementação, não intenção obsoleta.
- [ ] Evidence snapshot é atual, autenticado quando depende do GitHub e invalidado se policy/workflow/SHA mudou.

## Sources

[9] https://book.servo.org/embedding/overview.html
[10] https://github.com/servo/servo/blob/main/components/servo/lib.rs
[21] https://github.com/web-platform-tests/wpt/blob/master/README.md
[22] https://web-platform-tests.org/running-tests
