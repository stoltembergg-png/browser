# DEPENDENCIES.md — dependency policy

## Estado

Este repositório ainda não contém `Cargo.toml`, runtime code ou product dependencies. Esta policy prepara o bootstrap; não é um inventário nem prova de resolução de dependências.

## Regras

- Preferir stdlib e dependências maduras, pequenas e necessárias.
- Toda dependência deve declarar função, owner, licença, source, versão/pin, manutenção, advisories conhecidos, alternativa e custo de remoção.
- `Cargo.lock` será versionado quando houver workspace executável; builds e CI usarão resolução locked.
- Não usar tags mutáveis como prova de supply chain; revisões, checksums e source devem ser verificáveis.
- Dependências de produção não podem importar `test-support`, fixtures, ferramentas de desenvolvimento ou secrets.
- Servo/Tauri e patches locais exigirão revisão específica, source/revision, licença, build evidence e rollback.
- `unsafe` não é permitido por padrão em domain/core/security/storage; exceção exige justificativa, policy, testes e reviewer humano.

## Advisories, licenses e sources

O gate futuro deve verificar advisories, licenças permitidas/proibidas, sources registradas, transitive closure e secrets. Uma exceção exige owner, justificativa, escopo, mitigação, data de expiração e ADR; exceção expirada ou sem evidência é `NO_GO`.

## Toolchain e MSRV

A revisão do toolchain, edition, resolver, MSRV e matriz Windows/Linux/macOS será ratificada em ADR-001 após o workspace mínimo e a evidência de runners. Não declarar suporte de OS apenas por intenção documental.

## Atualização e rollback

Atualizações devem ser pequenas, reproduzíveis e acompanhadas de changelog/advisory/license diff, testes e artifact identity. Para vulnerabilidade urgente, atualizar/forward-fix ou pin conhecido; não desabilitar o gate. Remoção de dependência deve preservar o lockfile histórico e verificar a closure resultante.

## Escopo excluído

Esta policy não autoriza adicionar dependências, implementar o workspace, configurar CI ou escolher uma licença final.
