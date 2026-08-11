# RELEASE_STRATEGY.md — canais, artefatos e updates

## 1. Objetivo

Release é um produto de supply chain. O workflow só publica o que foi construído, testado, assinado e atestado para o mesmo repository/commit/tree, com artifact digest verificável fora do runner.

## 2. Canais

| Canal | Propósito | Entrada | Garantias |
|---|---|---|---|
| `nightly` | integração Servo/OS, WPT, stress | schedule/protected main | pode quebrar; não é recomendada a usuários |
| `alpha` | MVP/feature feedback | tag/branch protegida | signed, known risks, migration warning |
| `beta` | estabilidade/compatibilidade | release candidate | gates M5/M6, recovery e support docs; não pode alegar production browser, sandbox, site isolation ou secure renderer |
| `stable` | conjunto 1.0 suportado | tag protegida | Stable criteria, signed/provenance, incident runbook |

Tags e versionamento devem ser definidos por ADR antes do primeiro artifact. Recomendação: SemVer para releases públicas, identificador de revisão/release metadata separado e canal explícito; nenhum canal reutiliza tag.

## 3. Build matrix

A matriz alvo é Windows, Linux e macOS, mas o OS floor e os formatos exatos só são “supported” após PR-052/053/054. Candidatos a validar:

- Windows installer/package produzido pelo bundler Tauri e assinado com Authenticode;
- macOS app/bundle/dmg conforme notarization e signing disponíveis;
- Linux AppImage e/ou packages da distribuição escolhida, com checksums e assinatura do repositório quando aplicável.

Não anunciar uma combinação de formato/arquitetura só porque o build local completou; cada uma precisa installer smoke, launch, update/uninstall e profile preservation.

## 4. Pipeline de release

```text
tag/protected approval
  → verify tag/commit/tree/policy
  → locked build per OS
  → unit/integration/E2E/WPT/release tests
  → package
  → SBOM
  → provenance/attestation
  → sign artifacts + update metadata
  → verify signatures/checksums in clean job
  → canary/private release
  → publish GitHub Release/channel metadata
  → post-publish download/verify smoke
```

Release workflow usa protected environment, secrets apenas no job mínimo, runner confiável/isolado e artifacts imutáveis. Pull requests nunca possuem signing secrets.

## 5. Artefatos obrigatórios

Para cada `(channel, version, OS, arch, artifact)`:

- arquivo instalável e digest SHA-256;
- signature do artefato e instrução de verificação;
- SBOM SPDX ou CycloneDX;
- provenance/attestation com repo, workflow, ref, SHA e digest;
- manifest de versão, channel, engine revision, minimum supported version;
- release notes e known issues;
- test summary e compatibility/WPT expectation snapshot;
- rollback/incident reference.

GitHub artifact attestations participam da cadeia de provenance; não substituem signing do produto nem verificação no updater.[16]

## 6. Signing e chaves

- chaves de build, update metadata e macOS/Windows são separadas;
- chaves não aparecem em source, logs ou artifacts intermediários;
- rotação, backup, revogação e emergency contact documentados;
- release job não imprime inputs/paths sensíveis;
- assinatura verificada em job limpo e por ferramenta independente;
- um artifact sem signature válida nunca é “repaired” manualmente após publicação.

## 7. Update system posterior

O updater deve:

1. buscar metadata do channel configurado via transporte seguro;
2. validar assinatura, schema, channel, OS/arch, version ordering, expiry e hash;
3. baixar para staging não executável;
4. verificar digest e espaço/compatibility;
5. instalar de modo atômico com last-known-good;
6. confirmar startup/health após update;
7. fazer stop/rollback para last-known-good se health falhar;
8. preservar profile e migrations com forward-compatibility/backup;
9. obedecer revocation/kill switch e pausar channel comprometido.

A configuração de assinatura e chave pública do Tauri updater precisa ser testada contra metadata inválida, não apenas ligada no config.[7]

## 8. Rollback específico por risco

| Risco | Rollback/response |
|---|---|
| binário não inicia | stop e last-known-good; preservar profile |
| migration falha | recovery/forward-fix com snapshot; nunca apagar histórico/audit append-only |
| update key comprometida | revogar channel/key, pausar updater, publicar known-good por canal alternativo, rotate keys |
| artifact adulterado | rejeitar digest/provenance; remover release; reconstruir de SHA conhecido |
| regressão web | pausar canal, fix-forward por PR, manter expectativa/release anterior |
| vulnerability crítica | emergency release assinada, advisories, update floor, disable risky capability |

`git revert` é útil para código, mas não é rollback suficiente para migrations compartilhadas, profile data, signatures ou update metadata.

## 9. Versionamento e changelog

- mudanças públicas têm changelog com risco/migration/update notes;
- mudanças de schema e security policy são destacadas;
- engine revision e WPT revision aparecem em diagnostics/release manifest;
- breaking changes de profile têm migration test e rollback/recovery plan;
- release notes não prometem suporte de OS/feature não validado na matriz;
- claim scanner compara release metadata/notes com `forbidden_claims` do gate; claim proibido ou sem matriz engine/OS/teste resulta em `NO_GO`.

## 10. Release gates

### Nightly

Build + smoke + basic WPT/stress; failures abrem diagnóstico e não fazem auto-publish de stable.

### Alpha

MVP/Alpha criteria, security negative suite, profile migration, signed artifacts, SBOM/provenance, known risks.

### Beta

Platform matrix, WPT expectation policy, performance/stress, updater verification/recovery, threat model review, support docs.

### Stable

Stable criteria do `PROJECT_PLAN.md` e `docs/gates/release-gates.yaml`, incluindo engine host separado e evidência adversarial por OS, artifact verification fora do runner, signed/provenanced artifacts, no critical unexplained blockers, rollback drill e release incident playbook.

## 11. Sources

[7] [Tauri Updater](https://v2.tauri.app/plugin/updater/) · [16] [Artifact Attestations](https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations) · [17] [Security hardening](https://docs.github.com/en/actions/security-for-github-actions/security-hardening-your-deployments) · [20] [Dependabot](https://docs.github.com/en/code-security/dependabot/dependabot-security-updates/about-dependabot-security-updates).

## Sources

[7] https://v2.tauri.app/plugin/updater
[16] https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations
[17] https://docs.github.com/en/actions/security-for-github-actions/security-hardening-your-deployments
[20] https://docs.github.com/en/code-security/dependabot/dependabot-security-updates/about-dependabot-security-updates
