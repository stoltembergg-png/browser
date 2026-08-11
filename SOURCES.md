# Fontes primárias consultadas

Este arquivo registra a base externa usada no planejamento. As referências são evidência documental atual consultada diretamente por HTTPS; elas não substituem spikes executáveis nem prova de compatibilidade entre Tauri e Servo.

[1] https://doc.rust-lang.org/cargo/reference/workspaces.html — Cargo Workspaces.
[2] https://doc.rust-lang.org/cargo/reference/manifest.html#the-lints-section — Cargo manifest e seção de lints.
[3] https://doc.rust-lang.org/cargo/commands/cargo-test.html — comportamento e opções de `cargo test`.
[4] https://v2.tauri.app/security/ — modelo de segurança do Tauri.
[5] https://v2.tauri.app/security/capabilities/ — capabilities, permissions e escopo por plataforma/janela.
[6] https://v2.tauri.app/develop/calling-rust/ — comunicação frontend/core.
[7] https://v2.tauri.app/plugin/updater/ — updater e verificação por chave pública.
[8] https://v2.tauri.app/concept/process-model/ — core process, WebView process e least privilege.
[9] https://book.servo.org/embedding/overview.html — estado atual da documentação de embedding do Servo.
[10] https://github.com/servo/servo/blob/main/components/servo/lib.rs — API pública e guia inline de embedding observado no `main`.
[11] https://github.com/servo/servo/blob/main/components/servo/Cargo.toml — crate/features do Servo observado no `main`.
[12] https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets — GitHub Rulesets.
[13] https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets — regras disponíveis para Rulesets.
[14] https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue — Merge Queue.
[15] https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#merge_group — evento `merge_group`.
[16] https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations — artifact attestations.
[17] https://docs.github.com/en/actions/security-for-github-actions/security-hardening-your-deployments — hardening de workflows/deployments.
[18] https://docs.github.com/en/code-security/secret-scanning/introduction/about-secret-scanning — secret scanning.
[19] https://docs.github.com/en/code-security/code-scanning/introduction-to-code-scanning/about-code-scanning — code scanning/CodeQL.
[20] https://docs.github.com/en/code-security/dependabot/dependabot-security-updates/about-dependabot-security-updates — Dependabot security updates.
[21] https://github.com/web-platform-tests/wpt/blob/master/README.md — escopo e ferramentas do WPT.
[22] https://web-platform-tests.org/running-tests/ — execução do WPT.
[23] https://w3c.github.io/webappsec-csp/ — Content Security Policy Level 3.
[24] https://w3c.github.io/webappsec-secure-contexts/ — Secure Contexts.
[25] https://w3c.github.io/permissions/ — Permissions specification.
[26] https://fetch.spec.whatwg.org/ — Fetch, CORS e políticas de rede da plataforma web.
[27] https://api.github.com/repos/servo/servo/commits/main — snapshot da revisão `main` observado na pesquisa.

## Snapshot de pesquisa

- [27] No momento da consulta, a API pública do GitHub reportou Servo `main` em `859bd5edd60c0fb162a1f73c083a23e55474faf7`, com a mensagem “layout: Implement text-transform: full-size-kana (#47160)”, de 2026-08-11T16:18:20Z. Esse SHA é evidência de pesquisa, não uma dependência já aprovada.

## Limites de evidência

- O Servo `main` expõe tipos e métodos úteis para um adapter, mas a documentação de embedding se declara esparsa e em andamento; nenhuma API de integração é tratada como contrato estável sem spike contra uma revisão fixada.
- A documentação do Tauri descreve o shell e seus boundaries; ela não prova que uma surface renderizada pelo Servo possa ser composta dentro de uma janela Tauri em todas as três plataformas.
- GitHub Docs documenta capacidades e regras; a configuração efetiva de um repositório futuro só será provada por snapshot autenticado e canário negativo após o repositório existir.
- WPT fornece a suíte e o runner; a matriz de expectativas e os resultados do Servo precisam ser gerados pelo projeto, nunca inventados no planejamento.

## Sources

[1] https://doc.rust-lang.org/cargo/reference/workspaces.html
[2] https://doc.rust-lang.org/cargo/reference/manifest.html#the-lints-section
[3] https://doc.rust-lang.org/cargo/commands/cargo-test.html
[4] https://v2.tauri.app/security/
[5] https://v2.tauri.app/security/capabilities/
[6] https://v2.tauri.app/develop/calling-rust/
[7] https://v2.tauri.app/plugin/updater/
[8] https://v2.tauri.app/concept/process-model/
[9] https://book.servo.org/embedding/overview.html
[10] https://github.com/servo/servo/blob/main/components/servo/lib.rs
[11] https://github.com/servo/servo/blob/main/components/servo/Cargo.toml
[12] https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets
[13] https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets
[14] https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue
[15] https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#merge_group
[16] https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations
[17] https://docs.github.com/en/actions/security-for-github-actions/security-hardening-your-deployments
[18] https://docs.github.com/en/code-security/secret-scanning/introduction/about-secret-scanning
[19] https://docs.github.com/en/code-security/code-scanning/introduction-to-code-scanning/about-code-scanning
[20] https://docs.github.com/en/code-security/dependabot/dependabot-security-updates/about-dependabot-security-updates
[21] https://github.com/web-platform-tests/wpt/blob/master/README.md
[22] https://web-platform-tests.org/running-tests/
[23] https://w3c.github.io/webappsec-csp/
[24] https://w3c.github.io/webappsec-secure-contexts/
[25] https://w3c.github.io/permissions/
[26] https://fetch.spec.whatwg.org/
[27] https://api.github.com/repos/servo/servo/commits/main