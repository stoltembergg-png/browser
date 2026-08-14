# SECURITY_MODEL.md — modelo de segurança e controles

## 1. Postura

O navegador processa conteúdo hostil por definição. Segurança é uma propriedade de boundaries e invariantes, não apenas de uma lista de dependências. O MVP single-process é explicitamente in-process/thread-affine, não boundary de segurança: um compromise do renderer pode atingir o mesmo processo do core. Até prova de isolamento por OS, o projeto não promete sandbox, site isolation ou “secure renderer” equivalentes a browsers maduros; sem essa prova o produto não pode ser Stable.

Tauri oferece capabilities/permissions e um modelo de core/WebView que deve ser aplicado com least privilege.[4][5][8] O Servo/engine implementa a web platform; o core aplica políticas de browser. CSP, Secure Contexts, Permissions e Fetch/CORS devem seguir specs e testes do engine, não heurísticas inventadas.[23][24][25][26]

## 2. Trust boundaries

```text
TB-1 usuário ↔ browser UI
TB-2 browser UI (privileged Tauri WebView) ↔ typed IPC ↔ browser core
TB-3 browser core ↔ engine host/adapter
TB-4 engine/renderer ↔ web/network/untrusted document
TB-5 core ↔ profile/filesystem/keychain
TB-6 core ↔ update/release infrastructure
TB-7 contributor/PR/fork ↔ GitHub Actions/quality gate
TB-8 browser ↔ downloaded file/external protocol/OS
```

Regra: qualquer dado cruzando boundary é não confiável até schema, tamanho, identity, capability e policy serem validados.

## 3. Assets

- histórico, bookmarks, sessions, tabs e preferências;
- cookies, storage, credentials/tokens e profile keys;
- downloads e arquivos do usuário;
- render surface, engine memory e browser process;
- update metadata, public keys e release signing keys;
- GitHub tokens, workflow definitions, artifacts e provenance;
- trust store, certificate decisions e permission grants;
- diagnostics, crash dumps, URLs e telemetry;
- reputação e capacidade de distribuir software assinado.

## 4. Atores e ameaças

- página web maliciosa e script comprometido;
- renderer/engine comprometido por bug de parser/layout/JS;
- servidor MITM, DNS/protocol abuse ou certificado inválido;
- download malicioso e arquivo local preparado;
- extensão maliciosa ou com privilege escalation;
- processo local sem privilégio tentando IPC/filesystem;
- dependency/action comprometida;
- PR/fork com código e prompt/log/artifact hostil;
- conta/release infrastructure/update endpoint comprometido;
- usuário local com acesso ao profile; risco de exfiltração/roubo de profile.

## 5. Controles por boundary

| Boundary | Controles obrigatórios | Residual risk / gate |
|---|---|---|
| UI ↔ core | commands allowlisted, typed schema, size limits, capability/window/tab/frame/generation scope, correlation IDs, no generic eval; PR-045 fixes local origin/window/frame/generation checks and an empty capability baseline | bridge bug; fuzz + PR-024/045 |
| core ↔ engine | engine-api versioned, bounded channel, capability negotiation, stale generation checks, no Servo type leakage, runtime lifecycle contract | adapter drift/crash; PR-015/016/023/028 |
| engine ↔ web | engine SOP/CORS/CSP/secure context/permissions; browser policy for scheme/download/popup | Servo gaps; WPT + compatibility gate |
| core ↔ filesystem | brokered paths, profile root allowlist, canonicalization, atomic temp+rename, no page path; PR-040 enforces filename/path/quota/quarantine/cancel policy | local privilege/profile theft; PR-044/046 |
| core ↔ keychain | OS keychain abstraction, no plaintext credential fallback without ADR | OS-specific failure; PR-037 |
| CI ↔ PR | no secrets for fork, SHA-pinned actions, least token permissions, artifact identity validation, no `pull_request_target` code execution | GitHub policy drift; PR-006/008/010 |
| release ↔ client | signed artifacts/update metadata, key separation, provenance/SBOM, downgrade/channel checks, rollback | key compromise; PR-059/060 |
| browser ↔ external protocol | explicit scheme allowlist, confirmation, no shell interpolation, target validation | handler abuse; PR-044 |

## 6. Navigation e web policy

- Normalize and parse URLs with a real URL parser; não concatenar shell/paths.
- Allowlist `https`/`http` conforme policy; `file:`, `data:`, `javascript:`, custom schemes e external protocols têm decisões próprias e default deny onde não há contrato.
- `file://` não dá acesso ao profile nem é carregado na UI privilegiada. Acesso local futuro passa por broker e origin/permission model.
- Top-level/navigation policy inclui opener, current profile, private mode, user gesture e download intent.
- CORS, SOP, CSP, mixed content e secure contexts pertencem à semântica web do engine; o core não deve “corrigir” com regex.
- A UI Tauri só carrega assets empacotados/servidos pelo próprio app; nunca usa URL de página como origem privilegiada.
- Redirects reavaliam scheme, origin/context e download policy; não herdam cegamente uma decisão permissiva.

## 7. Tauri hardening

- capabilities por janela/webview, com allowlist mínima; PR-045 materializa `main-window` com `permissions: []`;
- commands explícitos, sem `invoke` genérico para filesystem/process/shell;
- CSP forte para frontend local, sem `unsafe-inline`/`unsafe-eval`/origins amplas salvo ADR comprovado; o fixture usa assets externos e `connect-src 'none'`;
- não expor APIs/plugins de Tauri ao conteúdo web;
- separar browser UI de page surface, mesmo que inicialmente compartilhando processo; declarar esse compartilhamento como risco, nunca como isolamento;
- validar payload, caller context, window/tab/frame/generation ID e lifecycle state;
- capability/CSP configuration lint e fixture hostil de negative IPC são requisitos do primeiro vertical slice, não controles adiados para Stable;
- rejeitar comandos de origem, janela, iframe, perfil ou geração incompatíveis antes de qualquer efeito privilegiado;
- updater configurado somente com chave pública e metadata assinada; a documentação do plugin updater requer atenção a assinatura e configuração do cliente.[7]
- plugins Tauri são opt-in, versionados e allowlisted por capability; cada plugin declara APIs, permissões, OS support, threat boundary, testes negativos e rollback. Nenhum plugin recebe acesso por herança ao conteúdo web ou ao filesystem inteiro.

## 8. Permissions

Modelo mínimo:

```text
PermissionRequest {
  permission_type,
  requesting_origin,
  top_level_site,
  opener_origin?,
  profile_id,
  tab_id,
  user_gesture,
  expiration,
}
```

Default deny para capacidades sensíveis. PR-042 mantém grants por `requesting_origin/top_level_site/opener_origin/profile_id/tab_id/permission`, exige user gesture para criar grants e suporta one-shot/session/persistent com expiração, revoke e clear por escopo. A UI mostra o origin efetivo; não confia em title/texto de página. Reset/clear data revoga grants relacionados. Permission policy do engine é observada e resolvida pelo core, nunca por evento frontend não autenticado.

## 9. Downloads

- request começa como pending; sem path controlado pela página;
- PR-040 usa um broker preso ao profile/root canonicalizado; metadata do engine não escolhe diretórios;
- filename sanitizado e limitado; separadores, ADS, device names e traversal bloqueados por OS;
- escreve em temp directory não executável quando possível;
- finaliza com rename atômico, quota e collision policy;
- content type/disposition é dado não confiável;
- checksum/quarantine/OS marking conforme plataforma;
- download interrompido não vira arquivo final;
- cancelamento remove/retém temp conforme policy e mostra estado;
- diagnósticos não vazam conteúdo ou tokens.

## 10. Updates e supply chain

- signing keys separadas por canal/ambiente, fora do repositório;
- metadata assinada inclui versão, canal, plataforma, hash, tamanho, min supported version e rollback/revocation info;
- cliente verifica assinatura, integridade, channel, downgrade e expiry antes de instalar;
- update falho mantém last-known-good; não apaga profile;
- compromise response pode revogar key/channel, pausar updater, publicar known-good e orientar instalação manual;
- cargo-deny para licenses/bans/sources/advisories;
- cargo-audit em PR/nightly;
- Dependabot/Renovate em PRs isoladas, nunca auto-merge de engine/FFI/trust boundary sem policy;
- SBOM SPDX/CycloneDX por artifact;
- CodeQL/code scanning, secret scanning, dependency review;
- actions pinadas por SHA e workflow permissions mínimas;
- provenance/attestation ligada a repo, workflow, commit e digest.[16][17][18][19][20]

## 11. Política `unsafe`

- crates de domínio, core, security e storage: `unsafe_code = "forbid"`;
- platform/render/Servo FFI: exceção apenas em módulos isolados;
- cada bloco contém `// SAFETY:` com preconditions/postconditions e ownership/thread invariants;
- teste positivo e negativo para a boundary;
- revisão por CODEOWNER de segurança/platform/engine;
- nenhum `unsafe` para silenciar Clippy, converter lifetime ou evitar refactor;
- novas exceções exigem ADR ou atualização explícita de policy.

## 12. Observabilidade segura

`tracing` estruturado com campos de categoria, request/tab/profile IDs derivados e timestamps monotônicos. Redaction antes do sink: URLs podem conter credentials/query secrets; page text, cookies, headers, paths e tokens nunca entram por padrão. Crash report é opt-in, mínimo e local primeiro. Telemetry de produto é opt-in até decisão formal; nenhuma coleta silenciosa é assumida.

PR-047 materializa o modelo: `browser-core::diagnostics` com `RedactionConfig` (allowlist de campos; campos fora da allowlist são `[redacted]`), redação golden por valor (userinfo/query/fragment de URLs, tokens/secrets, paths de usuário, conteúdo de página), `TelemetryGate` (opt-in; nada é coletado sem ele) e `CrashBundle` local (JSON redigido, sem campos de upload/cloud). Sempre-redigido: PII, page content, cookies, tokens.

Diagnostics bundle deve permitir investigação sem profile completo: versões, OS/arch, engine revision, feature flags, queue states, error codes, counts e hashes de artifacts. PII e conteúdo de página ficam fora.

## 13. Security gates

- M0: dependency/action/secret policy, unsafe baseline e capability/CSP policy schema;
- M1: Tauri capability/CSP review + engine boundary;
- M2: malformed IPC, navigation policy e crash tests;
- M3: profile/download/session threat cases;
- M4: threat model review and negative suite obrigatória; `docs/security/threat-regression-manifest.json` must enumerate TM-001…TM-018, and the Alpha/Stable evaluator is fail-closed;
- M5: WPT/security compatibility e fuzz/soak;
- M6: signed artifact, SBOM, provenance, updater compromise/recovery drill;
- Stable: engine host separado, launch restrictions e evidence adversarial por OS são obrigatórios; sem isso permanece Beta/experimental e nenhum claim de isolation é permitido.

## 14. Sources

[4] [Tauri Security](https://v2.tauri.app/security/) · [5] [Tauri Capabilities](https://v2.tauri.app/security/capabilities/) · [7] [Tauri Updater](https://v2.tauri.app/plugin/updater/) · [8] [Tauri Process Model](https://v2.tauri.app/concept/process-model/) · [16] [Artifact Attestations](https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations) · [17] [Security hardening](https://docs.github.com/en/actions/security-for-github-actions/security-hardening-your-deployments) · [18] [Secret scanning](https://docs.github.com/en/code-security/secret-scanning/introduction/about-secret-scanning) · [19] [Code scanning](https://docs.github.com/en/code-security/code-scanning/introduction-to-code-scanning/about-code-scanning) · [20] [Dependabot](https://docs.github.com/en/code-security/dependabot/dependabot-security-updates/about-dependabot-security-updates) · [23] [CSP](https://w3c.github.io/webappsec-csp/) · [24] [Secure Contexts](https://w3c.github.io/webappsec-secure-contexts/) · [25] [Permissions](https://w3c.github.io/permissions/) · [26] [Fetch](https://fetch.spec.whatwg.org/).

## Sources

[4] https://v2.tauri.app/security
[5] https://v2.tauri.app/security/capabilities
[7] https://v2.tauri.app/plugin/updater
[8] https://v2.tauri.app/concept/process-model
[16] https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations
[17] https://docs.github.com/en/actions/security-for-github-actions/security-hardening-your-deployments
[18] https://docs.github.com/en/code-security/secret-scanning/introduction/about-secret-scanning
[19] https://docs.github.com/en/code-security/code-scanning/introduction-to-code-scanning/about-code-scanning
[20] https://docs.github.com/en/code-security/dependabot/dependabot-security-updates/about-dependabot-security-updates
[23] https://w3c.github.io/webappsec-csp
[24] https://w3c.github.io/webappsec-secure-contexts
[25] https://w3c.github.io/permissions
[26] https://fetch.spec.whatwg.org
