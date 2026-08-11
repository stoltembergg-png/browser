# TESTING_STRATEGY.md — pirâmide e evidência executável

## 1. Princípio

Teste é contrato executável, não decoração de cobertura. Para cada comportamento novo: RED — teste falha pelo motivo esperado; GREEN — implementação mínima passa; REFACTOR — limpeza sem mudar comportamento. O runner é a autoridade do resultado. Skip/todo, zero testes ou mock que nunca atravessa o boundary não são prova.

`@spec:AC-xxx`/IDs equivalentes devem ligar requisito, teste, tarefa, PR e quality gate quando o repositório passar a usar ONP SDD. Testes de CI policy também usam TDD: missing, stale e malformed são casos de primeira classe.

## 2. Test layers

```text
                 ┌──────────────────────────┐
                 │ E2E / platform / WPT     │  poucos, caros, reais
                 ├──────────────────────────┤
                 │ integration / fixtures   │  boundaries
                 ├──────────────────────────┤
                 │ contract / fake engine   │  engine-neutral
                 ├──────────────────────────┤
                 │ unit / property / fuzz   │  muitos, rápidos
                 └──────────────────────────┘
```

### Unit

Crates/módulos de domínio, IDs, URL normalization, navigation state machine, permission policy, download filename/path policy, session migration, capability matrix, error taxonomy, evidence parser e applicability classifier. Sem runtime externo e sem depender de Servo.

### Contract

`engine-api` tem uma suite mínima que todo adapter precisa passar:

- lifecycle create/ready/close/shutdown;
- command ordering e correlation IDs;
- navigation started/committed/failed/cancelled/stale;
- frame/input/resize;
- timeout, cancellation, queue saturation e reliable-control admission;
- crash/restart and capability negotiation;
- unknown enum/event version rejection.

Popup/dialog/permission/download/authentication e DevTools têm suites de workflow do core, mas não entram no primeiro engine-neutral contract até capability, threat model e ADR próprios.

O `fake-engine` produz eventos controlados e falha de modo injetável. Não é substituto do Servo; é prova do core contra o contrato.

O `engine-contract-manifest` é único para fake e Servo: lista comandos/eventos obrigatórios, unknown-version/unsupported-command rejection, non-vacuity checks, frame/input/shutdown evidence e campos de identidade `(repository, commit_sha, tree_sha, engine_revision, OS, surface, thread_affinity, artifact_digest)`. PR-016 e PR-026 devem publicar `pass`, `fail`, `skip` ou `no-run` por caso; `skip`, `no-run`, adapter ausente, frame ausente ou fixture ausente é `NO_GO` e não pode ser convertido em pass pelo green do fake slice.

### Integration

| Boundary | Cenários |
|---|---|
| UI ↔ core | commands válidos, malformed, unauthorized, duplicate, stale response |
| core ↔ engine-host | lifecycle, backpressure, cancellation, crash, request correlation |
| core ↔ storage | clean profile, lock, migration, interruption, recovery, atomic save |
| core ↔ platform | paths, keychain abstraction, window/input/render handles |
| servo-engine ↔ Servo | pinned revision, load, frame, input, resize, delegates, shutdown |
| security ↔ engine | scheme, permission, popup, download and dialog decisions |

### E2E

Cada E2E inicia um app limpo e um servidor HTTP local controlado pelo test suite; casos que precisam HTTPS usam uma CA de teste gerada e trust explícito somente no fixture. Nenhum E2E depende de internet pública. O MVP executa o fluxo real na plataforma de referência; os demais OS só entram como suporte depois de seus gates de packaging/input.

Fluxo crítico:

```text
open app → create clean profile → type URL → load fixture
→ assert navigation/commit/frame → click link → assert new URL
→ back/forward/reload → close → restore → assert state
```

Outros: tab creation/switch/close, popup deny/allow, permission default deny/grant/expiry, download temp/final/quarantine, crash/restart, malformed IPC, shutdown with pending work.

### Regression

Cada bug corrigido adiciona um teste que falhava antes, com reprodução mínima e referência ao issue/PR. Regressões de engine usam fixture web reduzida; regressões de core não dependem de Servo se um fake engine reproduz o contrato.

## 3. Browser tests and WPT

WPT é uma suíte cross-browser e fornece `wpt run`, `wpt lint`, manifest e infraestrutura de execução.[21][22] Integração progressiva:

1. PR-049 fixa uma revisão WPT e cria runner/manifest reproduzível.
2. Começar com subset que toca navigation, URL/origin, fetch/CORS, CSP, permissions e rendering básico.
3. Resultados têm `pass/fail/timeout/notrun/expected-fail` e são ligados à revisão Servo, OS, GPU/software mode e commit do navegador.
4. `expected-fail` exige owner, referência, motivo e data/condição de reavaliação; não pode esconder novo failure.
5. Nightly amplia o subset; release usa a política ratificada em ADR, não “full WPT” indefinido.
6. WPT upstream contribution fica separado de regressão interna e nunca altera expectations para fazer o gate passar.

## 4. Visual regression

A UI Tauri usa snapshots por OS, escala, fonte e tema em fixtures determinísticas. A surface de página usa checks semânticos (frame existe, tamanho, input, navigation) e snapshots somente onde o renderer é estável. Não exigir igualdade pixel-a-pixel entre Windows/Linux/macOS: GPU, fontes e WebRender podem variar. Baselines são versionadas e qualquer atualização requer diff revisável, motivo e teste não visual correspondente. Ambiente, fontes, captura, feature flags e baseline revision são fixados; baseline ausente, artifact faltante ou diff sem triagem é `NO_GO`, não aprovação automática.

## 5. Performance

O primeiro benchmark mede; não inventa budget. PR-055 cria manifest com cenários e registra p50/p95, cauda, memória inicial, memória por tab, startup cold/warm, nova tab, navigation fixture, frame responsiveness e shutdown. O baseline inclui hardware/OS/engine revision/feature flags, warmup, número de amostras, ruído permitido e comparação somente com baseline do mesmo OS/engine/flags.

Depois de dados suficientes, um ADR define budgets por cenário e tolerância de regressão. Antes disso, performance é artifact informativo para PR, mas ausência de baseline bloqueia Beta/Stable; não há número arbitrário. Release bloqueia regressão contra o budget vigente, falha/ausência de dados e regressão não triada, e exige que mudança de engine/pipeline atualize o baseline com justificativa.

## 6. Stress, fuzz e soak

- muitos tabs configuráveis em perfis S/M/L;
- navegações concorrentes e cancelamento;
- downloads grandes/pausados/interrompidos;
- popup storms e permission storms;
- engine hang/crash/restart;
- profile lock e abrupt termination;
- fuzz de URL/scheme, IPC envelopes, event parser, storage migration e policy inputs;
- soak noturno com leak/handle/thread counters.

Fuzz reproduzível gera corpus e seed como artifact; não envia dados reais ao CI.

## 7. Security tests

Casos mínimos:

- UI tenta chamar comando inexistente, sem capability, com campo extra ou ID de outra tab;
- página tenta navegar para scheme proibido, `file://`, `javascript:` top-level, path traversal ou download fora do broker;
- permission grant de origin A é apresentado a origin B;
- popup tenta escapar do profile/opener policy;
- download com filename malicioso, content-disposition enganoso, interrupção ou colisão;
- renderer emits event de versão desconhecida, stale navigation ou falsa success;
- Stop/close/cancel compete com commit, completion, prompt, restart, migration e shutdown;
- update metadata inválida, assinatura errada, downgrade, canal errado ou key revogada;
- PR/fork injeta prompt/log/artifact para reporter ou tenta alterar gate;
- action/workflow não pinado ou `permissions: write` inesperado;
- dependency advisory/licença/source fora da policy.

CSP, Secure Contexts, Permissions e Fetch/CORS são referências da plataforma web; o browser não deve substituir o engine por regex de segurança caseira.[23][24][25][26]

## 8. Coverage e qualidade

Não fixar um percentual global antes do baseline. A política deve evoluir em três estágios:

1. M0: publicar cobertura e detectar ausência de testes/zero tests.
2. M1–M3: evitar regressão por crate e exigir casos de erro/negative para `security`, `engine-api`, `browser-core` e `storage`.
3. Antes de Beta: ratificar floors por crate usando dados, complexidade e risco; mutation/property/fuzz complementam percentual.

Um PR não pode apagar testes, reduzir a matriz de casos ou marcar `#[ignore]`/skip para fazer o gate passar sem uma policy explicitamente versionada.

## 9. Test evidence

Todo artifact/test report deve carregar:

```text
repository, event, base/head/tree SHA, workflow revision,
engine revision, OS/arch, test command, test manifest revision,
result counts, skipped/notrun counts, artifact digest, run id/attempt
```

Resultados de A ficam inválidos quando B muda o head, tree, policy, workflow evaluator ou engine revision. A pipeline deve publicar falhas e logs redigidos sem PII/secrets.

## 10. Matriz de execução

| Categoria | PR | Nightly | Release |
|---|---|---|---|
| format/lint/unit/contract | todo PR | sim | sim |
| integration fixtures | core/engine/storage changes | sim | sim |
| platform build/smoke | baseline + paths | full | full |
| E2E critical | UI/core/engine/security paths | full | full |
| WPT subset | engine/security/navigation paths | ampliar | policy release |
| visual | frontend/Tauri changes | temas/OS | baseline check |
| performance | informativo ou budget path | sim | bloqueante com baseline/ADR; ausência é NO_GO |
| fuzz/soak | — | sim | pre-release sample |
| security/supply chain | todo PR | sim | sim + attest |

## 11. Sources

[3] [cargo test](https://doc.rust-lang.org/cargo/commands/cargo-test.html) · [9] [Servo Embedding](https://book.servo.org/embedding/overview.html) · [10] [Servo lib.rs](https://github.com/servo/servo/blob/main/components/servo/lib.rs) · [21] [WPT README](https://github.com/web-platform-tests/wpt/blob/master/README.md) · [22] [WPT Running Tests](https://web-platform-tests.org/running-tests/) · [23] [CSP](https://w3c.github.io/webappsec-csp/) · [24] [Secure Contexts](https://w3c.github.io/webappsec-secure-contexts/) · [25] [Permissions](https://w3c.github.io/permissions/) · [26] [Fetch](https://fetch.spec.whatwg.org/).

## Sources

[3] https://doc.rust-lang.org/cargo/commands/cargo-test.html
[9] https://book.servo.org/embedding/overview.html
[10] https://github.com/servo/servo/blob/main/components/servo/lib.rs
[21] https://github.com/web-platform-tests/wpt/blob/master/README.md
[22] https://web-platform-tests.org/running-tests
[23] https://w3c.github.io/webappsec-csp
[24] https://w3c.github.io/webappsec-secure-contexts
[25] https://w3c.github.io/permissions
[26] https://fetch.spec.whatwg.org
