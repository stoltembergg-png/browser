# Wave 001 — bootstrap da esteira

## Regra

Wave 001 materializa governança, contratos e rastreabilidade. Nenhum item desta wave implementa Tauri, Servo, browser-core, Cargo workspace ou comportamento de navegador.

## Itens

| Ordem | ID | Issue | Draft PR | Dependências | Estado |
|---:|---|---:|---:|---|---|
| 1 | `PR-001` Repository governance | #1 | #71 | — | ready |
| 2 | `PR-002` ADR/spec templates | #2 | #72 | #1 | blocked until PR-001 |
| 3 | `PR-003` PR/CODEOWNERS/policy contracts | #3 | #73 | #1, #2 | blocked until PR-001/002 |
| 4 | `PR-004` Rust workspace contract | #4 | #74 | #1 | blocked until PR-001 |
| 5 | `PR-005` Toolchain and dependency policy | #5 | #75 | #4 | blocked until PR-004 |

## Critério de entrada

Cada item precisa de Definition of Ready, Issue estruturada, ownership exclusivo, ADR gates resolvidos e testing plan. Draft aberto não significa que a dependência foi mergeada.

## Critério de saída

- os contratos dos cinco itens estão publicados como Draft PRs;
- a cadeia `PR-ID → Issue → Draft PR → SHA` está registrada;
- nenhum Draft contém funcionalidade do navegador;
- labels/status refletem dependências reais;
- nenhum required check é configurado antes de existir workflow funcional;
- `CURRENT_STATE.md` e `EXECUTION_MAP.md` foram atualizados com evidência GitHub.

## Evidence mapping

| Stable ID | Issue | Draft PR | Branch | Head SHA |
|---|---:|---:|---|---|
| `PR-001` | #1 | #71 | `docs/pr-001-repository-governance` | `f7d2ad0ce41a2a2a9ac5a20041ca26ea63ea7181` |
| `PR-002` | #2 | #72 | `docs/pr-002-adr-spec-templates` | `fda370ee88d0f432c246cc53c297a66b85fa31be` |
| `PR-003` | #3 | #73 | `docs/pr-003-policy-contracts` | `6c3b8ce4770d3203bdde02a1faa9afd4099ef004` |
| `PR-004` | #4 | #74 | `docs/pr-004-workspace-contract` | `ae6c619a88e51fb856bd58d2199666d49d945b9c` |
| `PR-005` | #5 | #75 | `docs/pr-005-dependency-policy` | `20f03733935744d467147bdc59de52112444fd70` |

## Paralelismo

`PR-002` e `PR-004` podem ser preparados em paralelo depois de `PR-001`; `PR-003` aguarda `PR-002`; `PR-005` aguarda `PR-004`. A abertura de Drafts não autoriza merge fora dessa ordem.
