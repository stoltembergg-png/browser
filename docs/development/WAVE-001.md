# Wave 001 — bootstrap da esteira

## Regra

Wave 001 materializa governança, contratos e rastreabilidade. Nenhum item desta wave implementa Tauri, Servo, browser-core, Cargo workspace ou comportamento de navegador.

## Itens

| Ordem | ID | Issue | Draft PR | Dependências | Estado |
|---:|---|---:|---:|---|---|
| 1 | `PR-001` Repository governance | #1 | a criar | — | ready |
| 2 | `PR-002` ADR/spec templates | #2 | a criar | #1 | blocked until PR-001 |
| 3 | `PR-003` PR/CODEOWNERS/policy contracts | #3 | a criar | #1, #2 | blocked until PR-001/002 |
| 4 | `PR-004` Rust workspace skeleton | #4 | a criar | #1 | blocked until PR-001 |
| 5 | `PR-005` Toolchain and dependency policy | #5 | a criar | #4 | blocked until PR-004 |

## Critério de entrada

Cada item precisa de Definition of Ready, Issue estruturada, ownership exclusivo, ADR gates resolvidos e testing plan. Draft aberto não significa que a dependência foi mergeada.

## Critério de saída

- os contratos dos cinco itens estão publicados como Draft PRs;
- a cadeia `PR-ID → Issue → Draft PR → SHA` está registrada;
- nenhum Draft contém funcionalidade do navegador;
- labels/status refletem dependências reais;
- nenhum required check é configurado antes de existir workflow funcional;
- `CURRENT_STATE.md` e `EXECUTION_MAP.md` foram atualizados com evidência GitHub.

## Paralelismo

`PR-002` e `PR-004` podem ser preparados em paralelo depois de `PR-001`; `PR-003` aguarda `PR-002`; `PR-005` aguarda `PR-004`. A abertura de Drafts não autoriza merge fora dessa ordem.
