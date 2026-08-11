# SECURITY.md

## Escopo

Este projeto trata todo conteúdo web, payload de IPC, metadata de engine, artifact e log como potencialmente hostil. O MVP é experimental e não deve alegar sandbox, site isolation ou renderer seguro sem evidência executável.

## Reporte privado

Não publique detalhes de vulnerabilidade, exploit, credenciais, dados de usuário ou payloads em Issues, Discussions, commits ou PRs públicos. Use o fluxo privado de **GitHub Security Advisories** assim que ele estiver habilitado no repositório.

Enquanto o canal privado não estiver habilitado, não faça divulgação pública: registre apenas um blocker administrativo sem detalhes exploráveis e habilite o canal antes de aceitar reports sensíveis.

## Conteúdo mínimo do reporte privado

- versão/tag/commit e plataforma;
- pré-condições e configuração;
- passos mínimos de reprodução, sem dados reais;
- impacto observado e boundary afetada;
- logs redigidos, sem tokens/URLs privadas/PII;
- sugestão de mitigação, se houver.

## Política de resposta

1. confirmar recebimento pelo canal privado;
2. reproduzir em ambiente isolado e fixar SHA do caso;
3. classificar impacto no threat model;
4. criar correção/teste/ADR em fluxo privado quando necessário;
5. publicar advisory e release somente com artifact, provenance, assinatura e rollback verificáveis;
6. atualizar `THREAT_MODEL.md`, `SECURITY_MODEL.md` e `RELEASE_STRATEGY.md`.

Não serão aceitos bypasses de CI, permissões amplas, segredos em workflows ou claims de segurança baseados apenas em documentação.

## Escopo de segurança por fase

- M0: trust path, Actions, secrets, CODEOWNERS e policy.
- M1/M2: boundary UI privilegiada/page surface, IPC, capability e lifecycle.
- M3/M4: storage, permissions, navigation, privacy e recovery.
- M5/M6: OS, supply chain, artifacts, signing e updater.
- M7/M8: adversarial evidence, engine host separado, rollout e Stable gate.

## Fontes normativas

`SECURITY_MODEL.md`, `THREAT_MODEL.md`, `CI_CD_STRATEGY.md`, `RELEASE_STRATEGY.md`, `docs/gates/release-gates.yaml` e `AI_AGENT_GOVERNANCE.md` são as fontes locais. Em conflito, siga a autoridade documental registrada em `docs/document-authority.yaml` e abra ADR quando necessário.
