# Orchestrateur Polygone — mode d'emploi

> NOTE DE PORTAGE — 4 novembre 2026 : l’orchestrateur est séquentiel et agentCommand reste désactivé par défaut. maxParallel est une limite de configuration, pas une preuve de workers parallèles.

## Ce que c'est

Une boucle unique qui remplace les deux harnais abandonnés
(`polygone-company`, `polygone-team`) et les anciennes sessions cron :

```
découvrir → choisir une tâche → dispatcher un worker → vérifier → commiter → écrire l'état
```

Elle est volontairement **ennuyeuse** : pas de rapport automatique, pas de
« RAS », pas de tâche cochée sans commande.

## Fichiers

| Fichier | Rôle |
|---|---|
| `ORDRE.md` | La constitution : mission, interdits, boucle, format de preuve |
| `orchestrateur.mjs` | La boucle exécutable (Node ≥ 22, zéro dépendance) |
| `dispatch.json` | Repo cible, commande d'agent, checks, lanes |
| `backlog.json` | Les tâches, dérivées de `ROADMAP.md` |
| `FREEBUFF-FORK.md` | Comment brancher un agent basé sur Freebuff |

## Utilisation

```bash
cd <racine-du-dépôt>

# Voir l'état courant
node docs/atlas/agent/orchestrateur.mjs --status

# Trouver le travail réel (TODO/FIXME, tests ignorés)
node docs/atlas/agent/orchestrateur.mjs --discover

# Préparer la prochaine tâche sans rien exécuter
node docs/atlas/agent/orchestrateur.mjs --once

# Exécuter la tâche, vérifier, ne pas commiter
node docs/atlas/agent/orchestrateur.mjs --once --execute

# Exécuter, vérifier, commiter
node docs/atlas/agent/orchestrateur.mjs --once --execute --commit

# Session longue : 4 h, arrêt propre, commit par tâche stable
node docs/atlas/agent/orchestrateur.mjs --loop --hours 4 --execute --commit

# Une tâche précise
node docs/atlas/agent/orchestrateur.mjs --once --execute --commit --task P1.2
```

## Sécurités

- **Sans `--execute`**, aucun agent n'est lancé. Le brief est écrit dans
  `.orchestrator/briefs/` et c'est tout.
- **Sans `--commit`**, aucun commit. L'arbre reste tel quel.
- **`STOP`** à la racine du dépôt : la boucle s'arrête proprement.
- **Échec d'un check** : pas de commit, la tâche va dans `blocked`, la boucle
  s'arrête. On ne contourne pas.
- **Timeout** par tâche (`taskTimeoutMs`, 30 min par défaut), `SIGTERM` puis
  `SIGKILL`.
- **Pas de boucle vide** : trois passages sans tâche → arrêt.

## État produit

```
.orchestrator/state.json      tâche courante, faites, bloquées, compteurs
.orchestrator/discovery.json  TODO/FIXME trouvés
.orchestrator/briefs/*.md     briefs des workers
logs/orchestrateur-*.log      journal de la boucle
logs/*-P1.2.log               rapport de session par tâche
```

## Checks par défaut

`dispatch.json` exécute, dans l'ordre :

1. `cargo fmt --all -- --check`
2. `node scripts/check-docs.mjs --tests 371`
3. `node scripts/check-atlas.mjs`
4. `node --test docs/atlas/agent/tests/backlog.test.mjs`
5. `cargo test --workspace -j 2`
6. `cargo clippy --workspace --all-targets -- -D warnings`
7. `git diff --check`

Un seul échec suffit à bloquer le commit. C'est le but.

## Brancher un agent

Voir `FREEBUFF-FORK.md`. En résumé : `agentCommand` doit recevoir un chemin de
brief via `{prompt}`, et l'agent doit parler à `hope-gateway`
(`http://127.0.0.1:8787/v1`), pas directement aux fournisseurs.

## Ce que l'orchestrateur ne fait pas

- Il ne décide pas de la vision. `VISION.md` fait autorité.
- Il n'invente pas de tâches. Il lit `backlog.json`.
- Il ne pousse pas sans `--push`.
- Il ne réécrit pas un test pour le faire passer.
- Il ne remplace pas la lecture humaine d'un diff.

## Lancement sûr

Depuis la racine :

    node docs/atlas/agent/orchestrateur.mjs --status
    node docs/atlas/agent/orchestrateur.mjs --once

--once sans --execute écrit un brief. Avec --execute, aucun agent n’est lancé
tant que agentCommand est null ; la tâche ne sera pas cochée. Configure un
runner après audit. Les commits sont limités aux chemins task.files ; le push
est désactivé par défaut. Aucun parallélisme multi-worker n’est actif.
