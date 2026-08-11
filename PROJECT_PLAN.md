# PROJECT_PLAN.md — Navegador Rust/Tauri/Servo

> **Status:** planejamento mestre aprovado; nenhum código de navegador foi implementado. A materialização de repositório, Issues e Draft PRs é uma fase operacional separada, cujo estado atual vive em `CURRENT_STATE.md`; enforcement externo só pode ser afirmado após consulta autenticada.
>
> **Objetivo:** definir uma fundação de produção para um navegador open source, extensível e multiplataforma, com Rust no core, Tauri 2 como shell privilegiado e Servo como engine web inicial.

## 1. Decisão executiva

A recomendação é **não** construir o navegador como uma aplicação Tauri cujo WebView também renderiza páginas web. O Tauri deve hospedar a UI privilegiada do navegador — omnibox, abas, configurações, downloads, histórico e diagnostics — enquanto o conteúdo web pertence a um `BrowserEngine` hospedado por um adapter Servo. A fronteira entre os dois será uma API de comandos/eventos tipada e versionada; tipos Servo não atravessam `browser-core`.

O primeiro milestone técnico é um **spike de integração Tauri + surface nativa do Servo**, antes de congelar o layout de uma janela. O Servo atual documenta `EventLoopWaker`, `ServoBuilder`, `RenderingContext`, `WebViewBuilder`, `WebViewDelegate`, `WebView::paint`, `RenderingContext::present`, `WebView::notify_input_event` e `Servo::spin_event_loop`, mas o próprio Servo Book classifica o embedding como esparso e em andamento.[9][10] A arquitetura, portanto, absorve mudanças do Servo por `servo-engine` e não por acoplamento espalhado.

O MVP começa com um processo de aplicação e uma thread/actor de engine isolada, sem alegar sandbox ou site isolation fortes. Multiprocessamento, sandbox do renderer e separação adicional de rede/GPU são gates posteriores ao MVP; engine host separado e evidência adversarial por OS são obrigatórios antes de Stable. Isso é mais honesto e reversível do que declarar isolamento que a primeira integração ainda não prova.

## 2. O que está dentro e fora

### Dentro do plano

- arquitetura modular e ownership de estado;
- contrato da engine e integração Servo;
- shell Tauri e bridge frontend/core;
- workspace Cargo e regras de dependência;
- concorrência, event loops, cancelamento e backpressure;
- segurança, threat model e política de `unsafe`;
- testes unitários, integração, E2E, WPT, visual, performance, stress e security;
- GitHub Actions, quality gate, Rulesets, merge queue/auto-merge e supply chain;
- releases assinadas, SBOM, provenance e updater;
- governança para agentes, ADRs e PRs pequenas;
- roadmap e DAG de PRs até Stable 1.0 e evolução multiprocess.

### Fora do escopo de implementação do navegador

- implementação funcional em Rust, TypeScript, Tauri ou Servo; a materialização operacional de repositório, Issues e Draft PRs pertence à esteira de desenvolvimento;
- escolha final de versão/revisão do Servo sem spike;
- promessa de compatibilidade web ou segurança equivalente a navegadores maduros;
- números de performance inventados;
- sistema de extensões ou DevTools implementado.

## 3. Premissas explícitas

| ID | Premissa | Estado | Consequência |
|---|---|---|---|
| ASM-001 | O workspace de produto começou sem código e sem histórico Git local. | Observado no baseline da fase de planejamento: `/c/Users/Gabriel/Desktop/browser` não tinha `.git` nem arquivos de produto. | A materialização cria somente governança/contratos; a primeira implementação funcional continua sendo PR-004/011 conforme o DAG. |
| ASM-002 | Servo será usado como engine inicial, mas seu embedding é uma superfície móvel. | Documentado e observado no `main` do Servo.[9][10] | Fixar uma revisão em cada spike; não expor tipos Servo fora do adapter. |
| ASM-003 | Tauri é adequado para shell e UI local, não automaticamente para compor uma surface Servo na mesma janela. | Não comprovado. | PR-013/014 são bloqueadores de arquitetura. |
| ASM-004 | Tokio é útil para orquestração do browser core, mas não deve dirigir diretamente o event loop do Servo. | Decisão proposta; confirmar no spike. | Engine actor com thread affinity e waker explícito. |
| ASM-005 | A UI de browser será frontend local e privilegiado. | Decisão proposta. | Nunca carregar páginas arbitrárias no WebView privilegiado; capabilities/CSP mínimas.[4][5][6] |
| ASM-006 | GitHub Rulesets, Merge Queue e artifact attestations estarão disponíveis no repositório futuro. | Não verificado. | A ativação exige snapshot autenticado e canário; até lá, apenas policy local.[12][14][15][16] |
| ASM-007 | Métricas de performance só serão convertidas em budgets depois de baseline reproduzível. | Decisão. | Nenhum número arbitrário vira gate de release. |
| ASM-008 | Extensões são uma superfície de privilégio e não fazem parte do MVP. | Decisão proposta. | Devem ter threat model e capability boundary próprios antes de protótipo. |

## 4. Princípios não negociáveis

1. **Core primeiro:** Tauri é shell, não autoridade de domínio.
2. **Engine atrás de contrato:** Servo é substituível e nunca aparece no domínio.
3. **No proof, no merge:** falta, skip, cancelamento, timeout, stale ou evidência incompatível bloqueia merge.
4. **Segurança não é um hardening tardio:** UI privilegiada, IPC, navegação, downloads, permissões e supply chain têm gates desde M0.
5. **Sem paridade fictícia:** capability ausente da engine vira degradação explícita ou bloqueio de release.
6. **PR pequena e verificável:** uma mudança lógica, um ciclo de teste, rollback específico e diff real.
7. **Evidência vinculada ao SHA:** plano, comentário de agente ou snapshot antigo não prova estado atual.
8. **Automação é autoridade técnica; IA explica:** um reporter de IA nunca aprova, cria status obrigatório, faz merge ou bypass.
9. **`unsafe` é exceção isolada:** FFI/platform/engine inevitáveis exigem justificativa, `SAFETY`, testes e revisão especializada.
10. **Sem panics acidentais:** o caminho de produto retorna erros tipados; `unwrap`/`expect`/`panic!` são proibidos por padrão em crates de domínio/core/security/storage e exceções locais exigem invariável documentada, mensagem, teste e allowlist revisável. Testes/xtask podem usar asserts deliberados. O adapter/engine não promete que uma dependência externa nunca panicará; essa limitação é tratada por host/restart e process boundary futuro.
11. **Sem números inventados:** performance, cobertura e compatibilidade entram como métricas medidas e ADRs, não como metas decorativas.

## 5. Resultado arquitetural proposto

```text
Tauri shell / local browser UI
        │ typed commands/events; least privilege
        ▼
Browser Core (Rust state machines and policy authority)
        │ engine-api; no Servo types
        ▼
Engine Host actor ── servo-engine adapter ── Servo revision
        │ render/input/event bridge
        ▼
Native render surface / compositor integration
```

O core é responsável por tabs, navigation intent, profiles, sessions, history, bookmarks, downloads, permissions, privacy, browser security policy e user-visible state. O engine é responsável por DOM, JavaScript, document lifecycle, web networking interno, layout, rendering e web-platform behavior que o Servo suporta. O core decide as consequências do browser; o engine emite requests e eventos para essas decisões.

## 6. Fases de produto

### MVP experimental (surface real)

O MVP é experimental e tem escopo de plataforma explicitamente limitado. Ele só estará pronto quando, em um perfil limpo e em fixtures controladas:

- o aplicativo abrir e fechar de forma determinística;
- uma janela Tauri apresentar uma UI local privilegiada;
- o usuário digitar uma URL HTTPS na omnibox;
- o core validar a navegação e criar um único tab;
- Servo carregar uma página local/HTTPS de teste na surface aprovada pelo spike;
- a página renderizar e aceitar input de mouse/teclado e resize;
- back, forward, reload, stop e estados loading/failed funcionarem;
- o caminho de eventos UI → core → engine → render → UI for testado;
- o engine crashar sem derrubar silenciosamente o core e deixar o tab em estado explícito;
- uma plataforma de referência previamente ratificada executar a superfície real;
- Windows, Linux e macOS tiverem somente o nível de build/feasibility declarado no manifest, sem claim de suporte completo;
- nenhum conteúdo arbitrário for carregado no WebView privilegiado do Tauri;
- nenhum documento chame o MVP de navegador de produção, sandboxed ou site-isolated.

### Alpha

Além do MVP: múltiplas abas, tab lifecycle, popups com decisão, session save/restore, profiles/locking, history, bookmarks, downloads seguros, permissions com default deny, clearing de dados, diagnostics redigidos, WPT subset com expectations triadas, builds assinados de canal alpha e crash recovery exercitado. O Alpha só pode ser publicado após os artefatos e evidências de M0–M6 exigidos pelo gate machine-readable.

### Beta

Além do Alpha: matriz multiplataforma estável, WPT e regressões relevantes sem falhas críticas não triadas, stress/soak, performance baseline e regressão controlada, updater verificado, SBOM/provenance, threat model revisado, documentação operacional, suporte a migrações de perfil e critérios de estabilidade por canal.

### Stable 1.0

Stable não significa “todas as features de um navegador maduro”. Significa que o conjunto declarado de navegação, tabs, sessions, history, bookmarks, downloads, permissions, privacy básica, packaging e updates possui:

- engine host fora do processo do browser core, com restrições de lançamento e evidência adversarial por OS;
- cenário de compromise cross-origin/renderer exercitado e residual risk explicitamente limitado;
- ausência de vulnerabilidade crítica conhecida sem decisão formal de bloqueio;
- release reproduzível o suficiente para o nível de risco aceito;
- artefatos assinados, SBOM e provenance verificável;
- rollback/stop de update exercitado;
- crash e data-loss scenarios tratados;
- WPT/compatibility gaps documentados, testados e sem regressão não triada;
- performance e memory budgets derivados de baseline aprovada;
- documentação de instalação, segurança, suporte e recuperação;
- nenhuma alegação de sandbox/site isolation além da prova produzida para cada plataforma.

Ausência de engine host separado, assinatura, provenance ou evidência de isolamento mantém o produto em Beta/experimental; não pode ser compensada por exceção editorial. Extensões e DevTools podem permanecer fora do Stable somente quando seu escopo e sua exclusão estiverem ratificados pelo gate.

## 7. Resultado da revisão crítica

A segunda passagem como arquiteto adversarial alterou o plano nestes pontos:

1. **Surface não presumida:** Tauri/WebView e surface Servo têm spike separado; se a janela única falhar, o core permanece intacto e a composição muda.
2. **Isolamento não presumido:** o MVP single-process é explicitamente in-process/thread-affine e não é boundary de segurança; Stable fica bloqueado até engine host separado e evidência por OS.
3. **CI não presumida:** YAML válido não prova required status; o repositório futuro exige snapshot autenticado, canários negativos e identity por SHA/tree/evento.
4. **Performance não presumida:** não há números arbitrários; PR-055 mede baseline e um ADR posterior ratifica budgets.
5. **Features de alto privilégio adiadas:** extensions, DevTools amplo, network/GPU process e updater unattended têm gates próprios e rollback específico.
6. **PR DAG reduzido a contratos verificáveis:** crates são poucos no bootstrap, domains viram módulos antes de virar packages, o grafo está em `docs/architecture-graph.yaml` e cada PR tem fora de escopo.
7. **Dados e updates têm rollback próprio:** migration/profile usa forward-fix/backup; update usa stop/last-known-good; chave comprometida exige revogação/rotação.
8. **Protocolo de agentes não é autoridade:** dispatch ack, comentário de IA ou review humano não substitui teste/gate executado no SHA atual; o control-plane GitHub passa por UNVERIFIED → OFF → SHADOW → ENFORCED.
9. **TM-009 é gate executável:** a separação entre página hostil e UI/IPC privilegiado exige fixture hostil, negative tests de capabilities/CSP/IPC e evidência; sua ausência bloqueia MVP/Alpha.
10. **Bloqueios não são fatos de implementação:** surface Tauri↔Servo, ADRs, Rulesets/CI efetivos, storage/migração e HTTP/TLS permanecem spikes, decisões ou gates até evidência real.

Esses ajustes são blockers de planejamento corrigidos antes da apresentação final, não tarefas de implementação nesta execução.

## 8. Perguntas abertas que bloqueiam decisões finais

- **Q-001 — Surface:** a composição será uma janela nativa única, uma surface filha, duas janelas coordenadas ou outra estratégia? Gate: PR-013/014 em três OS.
- **Q-002 — Servo pin:** qual revisão e quais features do Servo serão suportadas? Gate: PR-016 com lockfile, build e smoke de `EventLoopWaker`/`spin_event_loop`, surface, frame e input.
- **Q-003 — Frontend:** TypeScript sem framework, React, Svelte ou outro? Gate: PR-012 com custo de dependência, accessibility e testability.
- **Q-004 — OS floor:** versões mínimas de Windows, macOS e distribuições Linux. Gate: PR-017/052/053/054.
- **Q-005 — Multiprocess:** Stable exige engine host separado e evidência de isolamento; permanece aberta apenas a estratégia concreta por OS e o custo/performance do processo. Gate: PR-064/066/069/070.
- **Q-006 — Credential storage:** keychain/credential manager por OS e política de fallback. Gate: PR-037/046.
- **Q-007 — Telemetry:** opt-in, opt-out ou nenhuma telemetry no primeiro canal? Recomendação: opt-in, dados mínimos e diagnostics local primeiro.
- **Q-008 — Merge enforcement:** Merge Queue disponível e habilitado? Gate: snapshot autenticado e canário; sem isso usar branch up-to-date + native auto-merge comprovado.
- **Q-009 — Licença:** compatibilidade e obrigações de Rust/Tauri/Servo e dependências. Gate: cargo-deny/license review antes do primeiro release.
- **Q-010 — Extensions:** superfície e modelo de permissões. Recomendação: fora do MVP/Stable inicial.
- **Q-011 — Profile storage:** backend/formato de armazenamento, schema de sessão, journal/backup e protocolo de migração/upgrade. Gate: ADR-006 e PR-035/036/037.
- **Q-012 — HTTP/TLS:** política de certificados, trust store, HTTP sem TLS, mixed content, erro de rede e delegação engine/plataforma. Gate: ADR de network policy, TM-008 e PR-044/049.

## 9. Milestones e gates

| Milestone | Saída | Gate de saída |
|---|---|---|
| M0 Governance | regras, workspace, toolchain, docs, CI trust root | quality gate local e política versionada |
| M1 Feasibility | Tauri shell + Servo embedding spike + engine contract | evidência de surface, input, frame, lifecycle nos OS alvo |
| M2 Browser Core | state machines, typed bridge, fake-engine slice e integração real mínima | E2E local, lifecycle contract e smoke real na plataforma de referência |
| M3 Browser State | tabs, popup, sessions, profiles, history/bookmarks/downloads | persistência atômica, crash/restart e regressões |
| M4 Security | navigation, permissions, Tauri capabilities/CSP, privacy | threat scenarios e negative tests |
| M5 Compatibility | WPT, platform packaging, performance, stress | expectations triadas, artefatos por OS e baseline |
| M6 Release | signed artifacts, SBOM, provenance, updater, channels | release canary e recovery drill |
| M7 Product maturity | Alpha/Beta gates e DevTools/extensions somente quando aprovados | gates machine-readable e release decision preliminar |
| M8 Isolation/Stable | engine host separado, process manager, sandbox/site isolation evaluation e Stable gate pós-isolation | ADR por OS, evidência processual e nenhum claim sem prova |

## 10. Definition of Done de qualquer PR futura

- objetivo, fora de escopo, risco, dependências e rollback explícitos;
- diff contém uma mudança lógica real, não apenas status ou placeholder;
- testes escritos antes da implementação quando há comportamento novo;
- `cargo fmt --check`, Clippy, testes afetados, quality gate e security checks executados;
- critérios de aceite ligados a teste, fixture ou canário autenticado;
- nenhuma evidência stale de outro SHA é reutilizada;
- documentação/ADR atualizados quando contrato, threat boundary ou operação mudou;
- PR não altera `main` diretamente e usa merge nativo protegido.

## Sources

[1] https://doc.rust-lang.org/cargo/reference/workspaces.html
[4] https://v2.tauri.app/security
[5] https://v2.tauri.app/security/capabilities
[6] https://v2.tauri.app/develop/calling-rust
[8] https://v2.tauri.app/concept/process-model
[9] https://book.servo.org/embedding/overview.html
[10] https://github.com/servo/servo/blob/main/components/servo/lib.rs
[12] https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets
[14] https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue
[15] https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows
[16] https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations
[21] https://github.com/web-platform-tests/wpt/blob/master/README.md
