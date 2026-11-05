# EXECUTION_STATUS — état de reprise

> Mis à jour après chaque lot. **Ne jamais y écrire un chiffre non exécuté.**
> Le registre des exigences est [`FOUNDER_VISION.md`](FOUNDER_VISION.md) ;
> l'avancement est calculé par `node scripts/vision-status.mjs`.

## Reprise immédiate

| Champ | Valeur |
|---|---|
| Date | 2026-11-04 |
| Dépôt | `~/Polygone-Atlas/03-REALIGNED` |
| Branche | `realign/v2` — locale, non poussée |
| HEAD | voir `git log -1 --format='%h %s'` |
| Arbre | propre après le lot (vérifié par `git status --short`) |
| Registre | **67 exigences**, **24 validées**, **9 bloquées** → avancement réel **24/58 = 41,4 %** (calculé, pas estimé) |

## Dernier travail effectué

0. **`GW.1` — heartbeat signé persisté** (fermé) : `POST /api/v1/nodes/heartbeat`
   vérifie ML-DSA-65 sur `polygone-heartbeat-v1\n{node_id}\n{timestamp}` avec la
   clé épinglée à l'appairage ; fenêtre anti-rejeu ±300 s ; écriture atomique
   de `nodes.json` ; rechargement au démarrage. Preuves :
   `heartbeat_requires_valid_signature` (403, état et disque inchangés),
   `heartbeat_survives_process_restart`, `heartbeat_outside_replay_window_is_refused`,
   `heartbeat_round_trip_over_real_http_is_persisted` (vrai socket TCP).
1. **`CLI.3` — profils d'usage** (fermé, commit `7a49150`) :
   `polygone profil <sobre|equilibre|genereux>` écrit `tier = "Custom"` + quatre
   seuils dans `~/.config/polygone/daemon.toml`, écriture atomique,
   `BehaviorConfig`/`SafetyMargins` conservés. Sans argument : profil courant,
   impact, chemin, et la phrase « aucune limite OS n'est posée sans isolation ».
   Preuves : `three_profiles_write_distinct_limits`,
   `profil_without_arg_prints_current`, session réelle où `polygoned status`
   relit `CPU 25% | RAM 30% | BW 40%` puis `CPU 75% | RAM 70% | BW 80%`,
   `profil turbo` → exit 1.
2. **Régression de latence corrigée** : OPS.2 cumulait deux sondes d'1 s
   (`refusals` puis `status`) ; `test_status_probe_does_not_hang_on_silent_daemon`
   a réellement échoué à 2,29 s. Budget partagé (`STATUS_PROBE_TIMEOUT / 2`),
   test repassé en 0,60 s.
3. **Registre de la vision** : `docs/FOUNDER_VISION.md`, construit depuis
   `_work/vision-long.md`, `00-SOURCES/VISION-EXTRAITE.md`, `VISION.md`,
   `ECOSYSTEM.md`, `PLAN-MAITRE.md`, `PLAN-FINITION.md` et `DECISION-POLY.md`.
4. **Outil d'avancement** : `scripts/vision-status.mjs` (compte les états 1–7,
   refuse une ligne mal formée, calcule validées / réalisables).

## Fichiers modifiés par ce lot

- `crates/client/src/profile.rs` (nouveau, 4 tests)
- `crates/client/src/lib.rs`, `crates/client/src/main.rs`, `crates/client/Cargo.toml`
- `daemon/src/cli.rs` (budget de sonde partagé)
- `docs/FOUNDER_VISION.md` (nouveau), `docs/EXECUTION_STATUS.md` (ce fichier)
- `docs/INDEX.md`, `scripts/vision-status.mjs` (nouveaux)
- `ARCHITECTURE.md`, `STATE.md`, `docs/atlas/agent/backlog.json`

## Tests exécutés (dernière exécution)

| Commande | Résultat |
|---|---|
| `cargo test --workspace -j 2` | **412 tests, 0 échec** |
| `cargo clippy --workspace --all-targets -- -D warnings` | **exit 0** |
| `cargo fmt --all -- --check` | **exit 0** |
| `node scripts/check-docs.mjs --tests 412` | **documents cohérents**, exit 0 |
| `node --test docs/atlas/agent/tests/backlog.test.mjs` | **6/6** |
| `node scripts/check-atlas.mjs` | **PASS** — 11 ouvertes, 29 terminées |
| `node scripts/vision-status.mjs` | **67 exigences, 24 validées, 9 bloquées** (41,4 % des réalisables) |

## Problèmes rencontrés

1. **Deux machines physiques absentes** : `BLOCKS.9`, `MSG.6` et `FV-INF-04`
   restent bloqués. Aucun test local ne doit être présenté comme une preuve
   inter-hôtes.
2. **Décisions du fondateur** : `D2` (isolation par défaut), `D8` (crédits POLY),
   licence (D1), Drive (D3), gateway (D7). Voir `NEEDS-LEVY.md` ; l'agent ne
   tranche pas.
3. **`gitleaks` absent** : SEC.8 prouve le refus des chemins suivis, pas
   l'audit d'historique.
4. **CI distante non exécutée** : aucun push autorisé ; CI.1 est vérifiée
   structurellement, pas sur runner.

## Prochaine tâche exacte

**`GW.1` — recevoir un heartbeat de nœud et le persister** (priorité P1 du
registre, `FV-OPS-05`). Dépendance : aucune.

Étapes :

1. `POST /api/nodes/heartbeat` authentifié ML-DSA-65 (signature du corps par la
   clé du nœud), mise à jour de `last_seen`.
2. Écriture atomique de `nodes.json` (fichier temporaire + `rename`), rechargement
   au démarrage de la gateway.
3. Refus explicite d'une signature invalide, sans écrire le fichier.
4. Preuves nommées : `heartbeat_survives_process_restart` et
   `heartbeat_requires_valid_signature`.
5. La gateway ne contacte jamais le relais : c'est le nœud qui pousse.

Après `GW.1` : `GW.2` (latence mesurée, jamais `0`), puis `SEC.1` (modèle de
menace par service), puis `CLI.6` (chaque code de refus → action).

## Commandes de reprise

```bash
cd ~/Polygone-Atlas/03-REALIGNED
git log --oneline -5
git status --short
node scripts/vision-status.mjs
cargo test --workspace -j 2
cargo clippy --workspace --all-targets -- -D warnings
node scripts/check-docs.mjs --tests 412
node --test docs/atlas/agent/tests/backlog.test.mjs
node scripts/check-atlas.mjs
```

## Règle de mise à jour

Ce fichier ne remplace ni `STATE.md` (journal des vérifications) ni
`FOUNDER_VISION.md` (registre). Après chaque lot : mettre à jour les trois,
exécuter les commandes ci-dessus, et ne passer une exigence à l'état 6 que si sa
preuve a été exécutée dans la session et citée dans le registre.
