# DEPENDENCIES.md — dependency policy

## Estado

O workspace M0 contém `Cargo.toml` e `Cargo.lock`, mas ainda não possui dependências externas de produção. Esta policy governa a introdução futura de dependências e a resolução reproduzível atual.

## Regras

- Preferir stdlib e dependências maduras, pequenas e necessárias.
- Toda dependência deve declarar função, owner, licença, source, versão/pin, manutenção, advisories conhecidos, alternativa e custo de remoção.
- `Cargo.lock` é versionado; builds e CI usam resolução locked (`--locked`).
- Não usar tags mutáveis como prova de supply chain; revisões, checksums e source devem ser verificáveis.
- Dependências de produção não podem importar `test-support`, fixtures, ferramentas de desenvolvimento ou secrets.
- Servo/Tauri e patches locais exigirão revisão específica, source/revision, licença, build evidence e rollback.
- `unsafe` não é permitido por padrão em domain/core/security/storage; exceção exige justificativa, policy, testes e reviewer humano.

## Advisories, licenses e sources

O gate futuro deve verificar advisories, licenças permitidas/proibidas, sources registradas, transitive closure e secrets. Uma exceção exige owner, justificativa, escopo, mitigação, data de expiração e ADR; exceção expirada ou sem evidência é `NO_GO`.

## Toolchain e MSRV

O bootstrap fixa Rust `1.97.1`, edition `2021`, resolver `2`, perfil minimal e os componentes `rustfmt`/`clippy` em `rust-toolchain.toml`. A matriz Windows/Linux/macOS e o MSRV formal serão ratificados após evidência de runners; não declarar suporte de OS apenas por intenção documental.

## Atualização e rollback

Atualizações devem ser pequenas, reproduzíveis e acompanhadas de changelog/advisory/license diff, testes e artifact identity. Para vulnerabilidade urgente, atualizar/forward-fix ou pin conhecido; não desabilitar o gate. Remoção de dependência deve preservar o lockfile histórico e verificar a closure resultante.

## Escopo excluído

Esta policy não autoriza adicionar dependências de produto, configurar CI ou escolher uma licença final. O bootstrap do workspace já foi entregue pela PR-004.
