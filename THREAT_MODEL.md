# THREAT_MODEL.md — baseline STRIDE

## Escopo

Este threat model cobre o MVP até Stable e registra o risco residual da arquitetura inicial. Ele deve ser refeito quando Servo revision, process model, storage schema, Tauri capabilities, updater, extension model ou trust boundary mudar. O uso de capabilities/process boundaries do shell [4][5][8] e das políticas web de CSP, Secure Contexts, Permissions e Fetch [23][24][25][26] é considerado controle de referência, não prova de implementação.

## Método

STRIDE é usado como checklist por boundary, complementado por abuse cases e release/supply-chain scenarios. Um cenário só está tratado quando existe controle **e** teste/telemetry/rollback que evidencie o controle.

## Cenários

| ID | Boundary/ativo | Ameaça | Impacto | Controle planejado | Teste/gate | Estado |
|---|---|---|---|---|---|---|
| TM-001 | página → engine | renderer compromise por conteúdo malicioso | leitura/execução no processo do browser | MVP: risco explicitamente in-process/thread-affine; Stable: engine host separado, launch restrictions e recovery | crash corpus, cross-origin compromise, process isolation ADR, PR-064–066/069–070 | blocker de Stable até evidência |
| TM-002 | UI → core | spoofed/malformed IPC | comandos privilegiados, filesystem/process | schema, allowlist, capability/window/tab scope, bounded payload | malformed IPC/fuzz | PR-024 negative suite; expansão futura |
| TM-003 | engine → core | evento stale/falso/unknown | tab errado, grant errado, state corruption | request/event IDs, navigation generation, versioned envelopes; PR-028 adiciona epoch fence | contract negative suite; `crash_recovery` stale epoch/generation tests | control implemented; engine artifact still required |
| TM-004 | página → navigation | scheme/protocol abuse | leitura local, shell/external handler | parser, scheme allowlist, broker, no shell interpolation | URL corpus, `file/data/javascript` cases | planejado |
| TM-005 | página → download | malicious filename/path | overwrite, traversal, executable delivery | temp root, canonicalization, sanitization, quarantine, atomic rename | OS path corpus, interrupted downloads | planejado |
| TM-006 | origin A → permission | confused deputy/grant reuse | camera/mic/notification/storage abuse | grant bound to origin/top-level/profile/tab context | cross-origin permission tests | planejado |
| TM-007 | popup/opener → browser | popup escape/spoof | unwanted windows, policy bypass | core decides target/window/profile and user gesture | popup storm/deny/allow | planejado |
| TM-008 | network → engine | MITM/cert misuse/mixed content | content/data compromise | TLS/cert policy delegated to trusted engine/platform, secure context tests, no ignore-cert default | local TLS fixtures + invalid cert | gate Servo support |
| TM-009 | web → privileged UI | page loaded into Tauri WebView | full Tauri privilege compromise | fixed local UI origin; page surface separate; CSP/capabilities | navigation attempt from fixture | blocker |
| TM-010 | local process → profile | profile theft/lock bypass | history/cookies/credentials loss | OS permissions, profile lock, keychain, diagnostics redaction | concurrent process/lock/backup tests | residual user-local |
| TM-011 | update server → client | metadata/key compromise | arbitrary signed software or rollback | signed metadata, key separation, revocation/stop, last-known-good | bad signature/downgrade/channel/revoke | planejado |
| TM-012 | CI PR/fork → workflow | secret exfiltration/command injection | release/GitHub compromise | no secrets in fork, pinned actions, least permissions, no target checkout | hostile PR fixtures, integrity gate | planejado |
| TM-013 | dependency → build | compromised crate/action | supply-chain backdoor | cargo-deny/audit, lockfile, review, SBOM, provenance, dependency policy | advisory/license/source violations | planejado |
| TM-014 | extension → core | excessive extension privilege | data exfiltration/automation | extensions out of MVP; manifest/capability/process boundary | extension threat suite | deferred |
| TM-015 | diagnostics → telemetry | sensitive URL/PII leakage | privacy breach | fixed `[REDACTED]` detail marker; no raw crash/hang detail retained by recovery policy | `crash_records_redacted_diagnostics_without_raw_detail`; security gate | control implemented; telemetry integration pending |
| TM-016 | engine hang/crash | denial/data loss | tab loss/app unavailable | watchdog/hang classification, bounded restart attempts, checkpoint retention, epoch/generation fencing, terminal result; no automatic form resubmission | `crash_recovery` crash/hang/restart/abrupt-shutdown tests | control implemented; real engine E2E pending |
| TM-017 | storage migration | malformed/corrupt profile | data loss/startup failure | versioned session schema, transactional journal, last-valid-snapshot recovery; full profile migration remains separate | torn/failed session commit, restore validation; old-version migration remains a downstream gate | control implemented for PR-036; full migration pending |
| TM-018 | external protocol | handler abuse | shell command execution/phishing | explicit allowlist/confirmation/no interpolation | protocol corpus | planejado |

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
