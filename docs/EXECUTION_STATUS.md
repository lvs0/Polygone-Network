# EXECUTION_STATUS — état de reprise

> Mis à jour après chaque lot. **Ne jamais y écrire un chiffre non exécuté.**
> Le registre des exigences est [`FOUNDER_VISION.md`](FOUNDER_VISION.md) ;
> l'avancement est calculé par `node scripts/vision-status.mjs`.

## Reprise immédiate

| Champ | Valeur |
|---|---|
| Date | 2026-11-05 |
| Dépôt | `~/Polygone-Atlas/03-REALIGNED` |
| Branche | `realign/v2` — locale, non poussée |
| HEAD | voir `git log -1 --format='%h %s'` |
| Arbre | propre après le lot (vérifié par `git status --short`) |
| Registre | **67 exigences**, **27 validées**, **9 bloquées** → avancement réel **27/58 = 46,6 %** (calculé, pas estimé) |

## Dernier travail effectué

0. **`FV-SEC-05` — rotation et révocation de clé** (fermé) : `polygone clef-tourner`
   génère une ML-DSA-65 neuve, signe la nouvelle clé avec l'ancienne sur
   `polygone-key-rotation-v1`, révoque l'ancienne et conserve l'identité KEM
   (donc le `node_id`). L'écriture de `identity.json` est atomique
   (temporaire + `rename`, chmod 600). Preuves :
   `rotation_keeps_identity_and_signs_the_new_key` (preuve vérifiable avec
   l'ancienne clé), `revoked_key_cannot_sign_anymore`, `rotation_survives_process_restart`.
   Limite : la révocation est locale, pas encore propagée aux pairs.
1. **`FV-SEC-10` — limite par identité** (fermé, commit `4bd1474`) :
   `IdentityLimiter` dans `crates/relay/src/relay.rs` : 100 enveloppes/s par
   identité, compteur partagé entre reconnexions, purge des fenêtres inactives.
   Un dépassement est journalisé et l'enveloppe ignorée, **sans fermer la
   connexion ni toucher les autres pairs**. Preuves :
   `per_identity_limit_denies_only_the_abusive_peer`,
   `per_identity_limit_resets_after_window`.
2. **`CLI.6` — chaque refus traduit en action** (fermé) : `crates/client/src/refusal.rs`
   associe les 8 constantes `REASON_*` du daemon à une cause et une action ;
   `NodeError::Denied` affiche le code technique **et** l'action. Un motif
   inconnu reste visible avec la mention « rapport de bug ». Preuves :
   `every_reason_code_has_an_action` (compare la liste aux constantes du
   daemon, exige cause + action non vides), `unknown_denial_keeps_the_technical_code`.
3. **`SEC.1` — modèle de menace par service** (fermé) :
   `docs/THREAT-MODEL-SERVICES.md` couvre client, relais, gateway, daemon,
   serverless et petals ; chaque contrôle renvoie à un test existant, chaque
   service liste ses risques résiduels et ce qu'il ne protège pas. Limite
   assumée : un fichier de synthèse au lieu d'un fichier par service ;
   `FV-SEC-08` reste à l'état 5 faute d'audit externe.
4. **`GW.2` — latence réellement mesurée** (fermé, commit `e98bcb1`) :
   `POST /api/v1/nodes/ping` délivre un challenge signé, à usage unique et
   valable 30 s ; `POST /api/v1/nodes/pong` n'écrit `latency_ms` qu'après
   vérification du challenge et de la signature, avec RTT ≤ 60 s. Un pong sans
   ping ou rejoué est refusé 409 ; un nœud silencieux garde `None`. Preuves :
   `latency_populated_after_pong`, `pong_without_ping_is_refused`,
   `pong_replay_is_refused`, et la mesure `42` lue dans
   `/api/v1/network/status` sur un vrai socket TCP.
5. **`GW.1` — heartbeat signé persisté** (fermé, commit `6a84c9b`) : `POST /api/v1/nodes/heartbeat`
   vérifie ML-DSA-65 sur `polygone-heartbeat-v1\n{node_id}\n{timestamp}` avec la
   clé épinglée à l'appairage ; fenêtre anti-rejeu ±300 s ; écriture atomique
   de `nodes.json` ; rechargement au démarrage. Preuves :
   `heartbeat_requires_valid_signature` (403, état et disque inchangés),
   `heartbeat_survives_process_restart`, `heartbeat_outside_replay_window_is_refused`,
   `heartbeat_round_trip_over_real_http_is_persisted` (vrai socket TCP).
6. **`CLI.3` — profils d'usage** (fermé, commit `7a49150`) :
   `polygone profil <sobre|equilibre|genereux>` écrit `tier = "Custom"` + quatre
   seuils dans `~/.config/polygone/daemon.toml`, écriture atomique,
   `BehaviorConfig`/`SafetyMargins` conservés. Sans argument : profil courant,
   impact, chemin, et la phrase « aucune limite OS n'est posée sans isolation ».
   Preuves : `three_profiles_write_distinct_limits`,
   `profil_without_arg_prints_current`, session réelle où `polygoned status`
   relit `CPU 25% | RAM 30% | BW 40%` puis `CPU 75% | RAM 70% | BW 80%`,
   `profil turbo` → exit 1.
7. **Régression de latence corrigée** : OPS.2 cumulait deux sondes d'1 s
   (`refusals` puis `status`) ; `test_status_probe_does_not_hang_on_silent_daemon`
   a réellement échoué à 2,29 s. Budget partagé (`STATUS_PROBE_TIMEOUT / 2`),
   test repassé en 0,60 s.
8. **Registre de la vision** : `docs/FOUNDER_VISION.md`, construit depuis
   `_work/vision-long.md`, `00-SOURCES/VISION-EXTRAITE.md`, `VISION.md`,
   `ECOSYSTEM.md`, `PLAN-MAITRE.md`, `PLAN-FINITION.md` et `DECISION-POLY.md`.
9. **Outil d'avancement** : `scripts/vision-status.mjs` (compte les états 1–7,
   refuse une ligne mal formée, calcule validées / réalisables).

## Fichiers modifiés par ce lot

- `crates/client/src/identity.rs`, `src/main.rs` (FV-SEC-05, 3 tests)
- `crates/relay/src/relay.rs` (FV-SEC-10, 2 tests)
- `crates/client/src/refusal.rs` (nouveau, 2 tests), `src/lib.rs`, `src/node.rs` (CLI.6)
- `crates/gateway/src/lib.rs`, `src/routes.rs`, `tests/serve.rs` (GW.2)
- `docs/THREAT-MODEL-SERVICES.md` (nouveau, SEC.1)
- `crates/client/src/profile.rs` (nouveau, 4 tests)
- `crates/client/src/lib.rs`, `crates/client/src/main.rs`, `crates/client/Cargo.toml`
- `daemon/src/cli.rs` (budget de sonde partagé)
- `docs/FOUNDER_VISION.md` (nouveau), `docs/EXECUTION_STATUS.md` (ce fichier)
- `docs/INDEX.md`, `scripts/vision-status.mjs` (nouveaux)
- `ARCHITECTURE.md`, `STATE.md`, `docs/atlas/agent/backlog.json`

## Tests exécutés (dernière exécution)

| Commande | Résultat |
|---|---|
| `cargo test --workspace -j 2` | **422 tests, 0 échec** |
| `cargo clippy --workspace --all-targets -- -D warnings` | **exit 0** |
| `cargo fmt --all -- --check` | **exit 0** |
| `node scripts/check-docs.mjs --tests 422` | **documents cohérents**, exit 0 |
| `node --test docs/atlas/agent/tests/backlog.test.mjs` | **6/6** |
| `node scripts/check-atlas.mjs` | **PASS** — 9 ouvertes, 35 terminées |
| `node scripts/vision-status.mjs` | **67 exigences, 27 validées, 9 bloquées** (46,6 % des réalisables) |

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
5. **Audit de sécurité externe absent** : `FV-SEC-08` reste à 5 malgré SEC.1 ;
   le modèle par service est une auto-évaluation, pas une certification.
6. **Gateway sans TLS** : l'intégrité est signée, la confidentialité vis-à-vis du
   LAN n'est pas assurée ; c'est écrit dans le modèle de menace.

## Prochaine tâche exacte

**`SEC.10` — inventorier ce que le relais voit réellement** (priorité P2,
backlog Atlas ; soutient `FV-SEC-02`). Dépendance : SEC.6 (fait).

Étapes :

1. Écrire `scripts/relay-metadata-audit.mjs` : lancer un relais local et un
   envoi réel, capturer les trames, produire le tableau de ce qui est lisible
   (source, destination, taille, cadence, horodatage) et de ce qui ne l'est pas.
2. Refuser de conclure si le flux de test n'a pas traversé un vrai socket.
3. Preuve : le rapport nomme les champs observés et le test de contrôle
   `hide-smoke.sh` reste vert.

Après `SEC.10` : `DOC.4` (document sans date ni statut refusé), puis
`FV-SEC-06` (coût Sybil mesuré).

## Commandes de reprise

```bash
cd ~/Polygone-Atlas/03-REALIGNED
git log --oneline -5
git status --short
node scripts/vision-status.mjs
cargo test --workspace -j 2
cargo clippy --workspace --all-targets -- -D warnings
node scripts/check-docs.mjs --tests 422
node --test docs/atlas/agent/tests/backlog.test.mjs
node scripts/check-atlas.mjs
```

## Règle de mise à jour

Ce fichier ne remplace ni `STATE.md` (journal des vérifications) ni
`FOUNDER_VISION.md` (registre). Après chaque lot : mettre à jour les trois,
exécuter les commandes ci-dessus, et ne passer une exigence à l'état 6 que si sa
preuve a été exécutée dans la session et citée dans le registre.
