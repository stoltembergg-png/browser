# License policy

## Estado

O repositório é público para transparência do planejamento, mas a licença do código ainda não foi ratificada. A publicação do repositório não concede automaticamente licença de uso, cópia, modificação ou distribuição.

## Regra antes de código

Nenhum código de produto deve ser adicionado ou tratado como open source antes de uma decisão documentada sobre:

- licença do projeto e compatibilidade com dependências;
- copyright/NOTICE e atribuição;
- fontes externas, Servo, Tauri e assets;
- distribuição de binários e artefatos;
- política para contribuições e generated files.

A decisão deve ser registrada em ADR e refletida em `LICENSE`, `NOTICE` e manifests quando aprovada. Até lá, Issues, documentação de planejamento e contratos podem ser publicados sob os direitos reservados aplicáveis, sem inventar uma licença permissiva.

## Enforcement planejado

`PR-005` adicionará a política de source/license/advisory; `PR-008` adicionará o gate automatizado; qualquer exceção exige owner, expiração, justificativa e ADR. Uma verificação ausente ou stale é `NO_GO`.
