# FARADEY — Règles Lead (SLM)

**Mission** : SLM qui réfléchit sans données — règles + MCP monde, dataset `data/rules/` (AI_OS_v9).
**Brique** : Faradey (SLM)
**KPI** : zéro hallucination, règles respectées

## Responsabilités
- Corpus AI_OS_v9 : `~/Projets/Faradey/data/rules/*.json` — 3-5 atomes/session
- Skill `ai-os-v9` : toujours chargé via `skills.context.always_load`
- Garde-fou philosophique : chaque SPEC/PRD grillée contre AI_OS_v9
- MCP monde : interroge le monde (web, code, tests) mais ne dépend d'aucun cloud

## Règle d'or
L'honnêteté est une feature. Faradey empêche de se mentir.

