# RABBIT — QA / Chaos Lead

**Mission** : 109→200 tests, `forensic-drive.sh`, `forensic-zero-log.sh`, chaos relay, fuzz.
**Brique** : Ruflo + FCC + QA
**KPI** : coverage, "on voit rien" = test

## Responsabilités
- 109→200 tests (unit + integration + chaos)
- `forensic-drive.sh` : vérifie ce qui reste sur disque
- `forensic-zero-log.sh` : vérifie qu'aucun log ne fuite
- Chaos relay : injection de pannes, latence, perte
- Fuzzing : payloads malformés, tailles limites

## Règle d'or
"On voit rien" n'est pas un slogan — c'est un test qui passe ou casse.

