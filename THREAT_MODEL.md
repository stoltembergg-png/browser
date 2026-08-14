# THREAT_MODEL.md — baseline STRIDE

## Escopo

Este threat model cobre o MVP até Stable e registra o risco residual da arquitetura inicial. Ele deve ser refeito quando Servo revision, process model, storage schema, Tauri capabilities, updater, extension model ou trust boundary mudar. O uso de capabilities/process boundaries do shell [4][5][8] e das políticas web de CSP, Secure Contexts, Permissions e Fetch [23][24][25][26] é considerado controle de referência, não prova de implementação.

## Método

STRIDE é usado como checklist por boundary, complementado por abuse cases e release/supply-chain scenarios. Um cenário só está tratado quando existe controle **e** teste/telemetry/rollback que evidencie o controle.

## Cenários

| ID | Boundary/ativo | Ameaça | Impacto | Controle planejado | Teste/gate | Estado |
|---|---|---|---|---|---|---|
| TM-001 | página → engine | renderer compromise por conteúdo malicioso | leitura/execução no processo do browser | MVP: risco explicitamente in-process/thread-affine; Stable: engine host separado, launch restrictions e recovery | crash corpus, cross-origin compromise, process isolation ADR, PR-064–066/069–070 | blocker de Stable até evidência |
| TM-002 | UI → core | spoofed/malformed IPC | comandos privilegiados, filesystem/process | schema, allowlist, PR-045 capability/window/origin/frame/generation scope, bounded payload | malformed IPC/fuzz; `ipc_bridge` caller-context negative suite | PR-024/045 negative suite; real Tauri command adapter pending |
| TM-003 | engine → core | evento stale/falso/unknown | tab errado, grant errado, state corruption | request/event IDs, navigation generation, versioned envelopes; PR-028 adiciona epoch fence | contract negative suite; `crash_recovery` stale epoch/generation tests | control implemented; engine artifact still required |
| TM-004 | página → navigation | scheme/protocol abuse | leitura local, shell/external handler | PR-044: conservative URL parser (no userinfo, no control chars, valid host/port), scheme allowlist (http/https/about only), redirect re-evaluation with https-downgrade refusal, file access rooted and traversal-checked, external handler allowlist with no shell interpolation | `navigation_policy` URL corpus, downgrade, traversal, handler-injection tests; state machine re-evaluates redirects | control implemented for PR-044; engine enforcement and confirmation UI pending |
| TM-005 | página → download | malicious filename/path | overwrite, traversal, executable delivery | PR-040: profile-bound canonical root, filename/ADS/device-name validation, quota, temp/quarantine, collision-safe finalization, cancel/interruption cleanup | `download_broker` traversal/ADS/device-name/collision/quota/interruption/profile tests; OS-specific expansion remains | control implemented for PR-040; OS-specific executable marking/checksum pending |
| TM-006 | origin A → permission | confused deputy/grant reuse | camera/mic/notification/storage abuse | PR-042: default deny, exact origin/top-level/opener/profile/tab/permission key, user-gesture grant, expiry, one-shot/session/persistent lifetime, revoke and scoped clear. PR-043: prompt-only grants, typed resolution (allow one-shot/session/persistent, deny/cancel), origin/top-level display from verified request, context-matched resolution, duplicate/stale prompt rejection | `permission_policy` origin-confusion/default-deny/gesture/expiry/one-shot/revoke/clear tests; `prompt_coordinator` spoofing/duplicate/stale/cancel/gesture tests; hardware integration remains | control implemented for PR-042/043; engine/hardware adapter pending |
| TM-007 | popup/opener → browser | popup escape/spoof | unwanted windows, policy bypass | core decides target/window/profile and user gesture | popup storm/deny/allow | planejado |
| TM-008 | network → engine | MITM/cert misuse/mixed content | content/data compromise | TLS/cert policy delegated to trusted engine/platform, secure context tests, no ignore-cert default | local TLS fixtures + invalid cert | gate Servo support |
| TM-009 | web → privileged UI | page loaded into Tauri WebView | full Tauri privilege compromise | PR-045: fixed local origin, main-window empty capability baseline, no remote URL, external-asset CSP, top-level caller/generation checks; page surface remains separate | config/capability/CSP fixture; origin/window/iframe/generation negative IPC | local shell control implemented; page/process isolation remains blocker |
| TM-010 | local process → profile | profile theft/lock bypass | history/cookies/credentials loss | OS permissions, profile lock, keychain, diagnostics redaction | concurrent process/lock/backup tests | residual user-local |
| TM-011 | update server → client | metadata/key compromise | arbitrary signed software or rollback | signed metadata, key separation, revocation/stop, last-known-good | bad signature/downgrade/channel/revoke | planejado |
| TM-012 | CI PR/fork → workflow | secret exfiltration/command injection | release/GitHub compromise | no secrets in fork, pinned actions, least permissions, no target checkout | hostile PR fixtures, integrity gate | planejado |
| TM-013 | dependency → build | compromised crate/action | supply-chain backdoor | cargo-deny/audit, lockfile, review, SBOM, provenance, dependency policy | advisory/license/source violations | planejado |
| TM-014 | extension → core | excessive extension privilege | data exfiltration/automation | extensions out of MVP; manifest/capability/process boundary | extension threat suite | deferred |
| TM-015 | diagnostics → telemetry | sensitive URL/PII leakage | privacy breach | fixed `[REDACTED]` detail marker; no raw crash/hang detail retained by recovery policy | `crash_records_redacted_diagnostics_without_raw_detail`; security gate | control implemented; telemetry integration pending |
| TM-016 | engine hang/crash | denial/data loss | tab loss/app unavailable | watchdog/hang classification, bounded restart attempts, checkpoint retention, epoch/generation fencing, terminal result; no automatic form resubmission | `crash_recovery` crash/hang/restart/abrupt-shutdown tests | control implemented; real engine E2E pending |
| TM-017 | storage migration | malformed/corrupt profile | data loss/startup failure | versioned session schema, transactional journal, last-valid-snapshot recovery; full profile migration remains separate | torn/failed session commit, restore validation; old-version migration remains a downstream gate | control implemented for PR-036; full migration pending |
| TM-018 | external protocol | handler abuse | shell command execution/phishing | PR-044: explicit handler allowlist, alphanumeric handler names, shell-metachar rejection for handler and args, confirmation required unless explicitly disabled | `navigation_policy` protocol corpus and shell-injection tests | control implemented for PR-044; OS handler launching adapter pending |
| TM-019 | extension → browser | extension boundary abuse | privileged API/loader/process compromise | PR-058: `extensions=false` no MVP/Alpha, capability gate fail-closed, manifest id/name/version validation, permission allowlist (storage only), no loader/API/isolated world/process model; ADR-008 go/no-go = NO_GO | `extensions` capability gate, malicious manifest/permission, privilege matrix tests | control implemented for PR-058; gate stays closed until ADR review |

## Regression gate (PR-048)

The machine-readable inventory at `docs/security/threat-regression-manifest.json` is the
release-gate source for TM-001 through TM-018. Every scenario must declare a control,
a test reference, an evidence note and an explicit `release_blocker` value. A status of
`partial`, `planned`, `deferred` or `blocked` is never silently treated as green.
TM-019 is tracked in this document only (the manifest is contract-fixed at TM-001..TM-018
by PR-048); it is non-blocking for release only because extensions are disabled, and it
becomes release-blocking if the capability gate is ever opened without an ADR review.

Run the inventory check on the current checkout with:

```text
python3 scripts/threat_regression_gate.py --validate-only
```

The release evaluator is fail-closed. `--channel alpha` and `--channel stable` return
`NO_GO` and a non-zero exit code while any scenario lacks complete evidence. The current
manifest deliberately reports `NO_GO`: the suite records available engine-neutral negative
controls but does not substitute fake-engine tests for Servo, process-isolation, TLS or
OS-specific artifacts.

## Abuse-case acceptance tests

1. Uma página fixture tenta `invoke`/command no frontend: não existe bridge de página e nenhum comando é executado.
2. Uma página na surface de conteúdo tenta redirect para a origem privilegiada, abrir popup/opener ou navegar um iframe: a surface, origem, capabilities e processo permanecem no contexto não privilegiado; nenhum comando é executado.
3. Um payload IPC com campo extra, tamanho excessivo, tab de outro profile e request duplicado: cada caso é rejeitado com erro observável.
4. Um event stream envia `NavigationFinished` para generation antiga: core não altera o tab atual.
5. Uma URL `file://`, `data:`, `javascript:` top-level e custom protocol: cada uma segue decisão explicitamente testada; default deny onde não há contrato.
6. Um download tenta `../../`, nome reservado Windows, ADS, colisão e cancelamento: nenhum arquivo sai do root/broker.
7. Origin A obtém grant; origin B solicita a mesma capability: B não herda a decisão.
8. Update com assinatura errada, hash errado, downgrade, canal incompatível, metadata expirada e key revogada: instalação falha e last-known-good permanece.
9. PR de fork tenta ler secret, alterar workflow gate, injetar texto no reporter e publicar artifact: não recebe secret; gates/identity rejeitam a evidência.
10. Processo do engine morre durante navegação/download/form submission: tab entra em Crash; nenhum POST/form é reenviado automaticamente.
11. Migration é interrompida em cada passo: reinício recupera snapshot válido ou executa rollback próprio; nunca marca sucesso sem schema completo.

## Security claims permitidos

### MVP

+- “UI local privilegiada separada conceitualmente do conteúdo web”;
+- “commands e navigation policy são validados”;
- “engine crash é observável e tab pode ser recuperado conforme policy”.

Não dizer “sandbox”, “site isolation” ou “renderer seguro” sem prova.

### Stable

Claims só podem mencionar propriedades cobertas por matriz de OS/engine revision e testes. Um documento de release deve apontar para os cenários TM que sustentam cada claim e para os residual risks aceitos. Sem TM-001 resolvido por engine host separado e evidência adversarial por OS, não há release Stable nem claim de sandbox/site isolation/secure renderer; o canal permanece Beta/experimental.

## Review triggers

Reabrir o modelo quando houver:

- novo engine ou Servo upgrade;
- nova Tauri capability/plugin/command;
- process/network/GPU split;
- download/update/profile migration;
- extension/devtools privilege;
- mudança de workflow, signer, Ruleset, artifact publisher ou secret scope;
- incidente, CVE relevante ou falha de WPT com impacto de segurança.

## Sources

[4] https://v2.tauri.app/security
[5] https://v2.tauri.app/security/capabilities
[8] https://v2.tauri.app/concept/process-model
[23] https://w3c.github.io/webappsec-csp
[24] https://w3c.github.io/webappsec-secure-contexts
[25] https://w3c.github.io/permissions
[26] https://fetch.spec.whatwg.org
