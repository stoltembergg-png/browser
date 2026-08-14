# ADR-008 — Extensions boundary: disabled for MVP/Alpha

- **Status:** accepted
- **Date:** 2026-08-14
- **Owners:** repository maintainers
- **Related PRs:** PR-058 (spike), PR-043, PR-045, PR-048
- **Related threats/tests:** TM-019, extensions boundary tests, privilege matrix

## Contexto

Extensões são uma superfície de privilégio (ASM-008). Antes de qualquer API, o
projeto precisa decidir manifest, isolated world, permissions, lifecycle e
process model. Este ADR registra a decisão de boundary do spike PR-058:
**extensões permanecem desabilitadas no MVP e no Alpha**.

Não existe loader, API empacotada, isolated world, modelo de permissões
completo nem processo de extensão. Enquanto o capability gate estiver
fechado, toda tentativa de ativação — incluindo manifestos hostis — é
rejeitada antes de qualquer conteúdo ser confiado.

## Decisão

- `ExtensionsCapability` default `enabled=false` no runtime do browser-domain.
- `try_activate` é fail-closed: capability desabilitada → `ExtensionsDisabled`;
  id/name/version inválidos → `InvalidManifest`; permissão fora da allowlist
  (`storage` apenas) → `PermissionNotAllowed`.
- `go/no-go` explícito: `NO_GO` para extensões no MVP/Alpha.
- Privilege matrix registra os scopes adiados: loader, isolated world,
  permissions, lifecycle e process model (extensão fora de processo).

## Limites explícitos

- Este ADR não cria loader nem API de extensão; não existe processo de
  extensão; sem store de extensões.
- Manifesto validado é spike-level (forma conservadora), não semântica
  completa de manifest.
- O gate só abre após ADR revisada cobrindo process model (fora de processo),
  isolated world, permissions allowlist e lifecycle.

## Consequências

### Positivas

- Superfície de ataque mínima e testável; tentativa hostil de ativação tem
  teste negativo explícito.
- Matriz de privilégios dá rastreabilidade do que falta e por quê.

### Negativas e riscos aceitos

- Sem extensões não há mercado de extensões; aceito para MVP/Alpha.
- Adiar isolation pode encarecer a retomada; mitigado pela matriz.
