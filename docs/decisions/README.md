# Architecture Decision Records

ADRs são decisões normativas, não atas de reunião. Cada ADR deve registrar contexto, decisão, alternativas descartadas, consequências, evidência e condições de revisão.

## Estado inicial

Ainda não há ADR ratificado no repositório do produto. Os itens abaixo são a fila de decisões que precisam de evidência antes da PR indicada:

| ID | Tema | Decisão proposta | Gate de ratificação |
|---|---|---|---|
| ADR-001 | Rust | Rust é a linguagem principal do core, adapters e tooling de runtime. | PR-004/005: workspace e toolchain reproduzíveis. |
| ADR-002 | Tauri | Tauri é shell privilegiado, lifecycle/window manager e bridge; não é o renderer de páginas web. | PR-011/012/045. |
| ADR-003 | Servo | Servo é engine inicial por adapter isolado e revisão fixada; APIs de `main` não viram contrato sem spike. | PR-013/016. |
| ADR-004 | Engine contract | PR-015 define o contrato provisório e o fake engine; ADR-004 registra a decisão e só pode ser aceita/ratificada após a evidência do contrato, antes de PR-020/021/025/026. | PR-015 (provisório), PR-020/021/025/026 (autoridade aceita). |
| ADR-005 | Concorrência | Core usa atores/channels bounded; engine respeita thread affinity e `spin_event_loop`; Tokio não dirige Servo diretamente; quotas, lanes e fencing ficam no runtime lifecycle manifest. | PR-020/023/025. |
| ADR-006 | Storage | Dados de perfil pertencem ao core/storage, com migrações compatíveis, lock por perfil e transações atômicas. | PR-035/037. |
| ADR-007 | IPC | UI usa comandos/eventos tipados e capability-scoped; nenhuma chamada genérica ou comando sem validação. | PR-021/024/045. |
| ADR-008 | Process model | MVP começa single-process com engine thread isolada, que não é boundary de segurança; engine host separado, launch restrictions e evidência adversarial por OS são obrigatórios antes de Stable. | PR-064/065/066/069/070. |
| ADR-009 | Release | Artefatos assinados, SBOM, provenance e updater por canal; rollback é stop/last-known-good, não revert cego. | PR-059/060. |

## Regras

- Uma decisão permanece `proposed` até o spike/teste/canário indicado produzir evidência.
- Mudança em ADR ratificado exige ADR novo ou seção de supersession; editar história sem registrar motivo é proibido.
- APIs instáveis do Servo devem ser citadas pela revisão exata usada no spike.
- Decisões de segurança e supply chain exigem cenário negativo, não apenas descrição positiva.
- Use `ADR-000-template.md` para decisões arquiteturais e `../specs/SPEC-000-template.md` para contratos e acceptance criteria.
- Uma especificação pode detalhar uma Issue, mas não pode ratificar uma decisão aberta nem substituir o ADR requerido.
- Toda decisão deve declarar impacto de segurança, testing/evidence, rollback e dependências.
