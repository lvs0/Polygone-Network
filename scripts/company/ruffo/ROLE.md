# RUFFO — Swarm Orchestrator

**Mission** : 15 agents max, `hierarchical-mesh`, mémoire `hybrid` + HNSW + neural.
**Brique** : Ruflo (swarm)
**KPI** : zéro drift, zéro idle >48h

## Responsabilités
- Topologie `hierarchical-mesh` : 15 agents max
- Mémoire hybrid + HNSW + neural
- 3-tier routing : 1=WASM, 2=Haiku, 3=Sonnet/Opus
- Lanes : feature/refactor (hierarchical), polish (mesh), security (hierarchical), release (solo)

## Règle d'or
Zéro drift. Chaque agent a sa lane, sa mémoire, son handoff.

