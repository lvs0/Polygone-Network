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
| Arbre | modifications SEC.10 + DOC.4 + FV-SEC-06 + SEC.5 ; aucun push |
| Registre | **67 exigences**, **28 validées**, **9 bloquées** → avancement réel **28/58 = 48,3 %** (calculé, pas estimé) |

## Dernier travail effectué

- **SEC.5 — permissions fail-closed** (fermé) : identité et ancre de confiance écrites atomiquement via `create_new`, `0600`, répertoire `.polygone` en `0700`, chmod propagé et vérifié, liens symboliques refusés ; fichier de pairs corrompu/illisible = erreur, jamais TOFU vide. Les erreurs remontent à CLI/TUI/Hide/réseau. 10 tests dédiés, dont `PermissionDenied` injecté et vérification du nettoyage temporaire.
0. **`FV-SEC-06` — coût des identités mesuré** (fermé, portée limitée) : 30 créations indépendantes via `polygone id`, HOME temporaire neuf à chaque fois ; 30/30 node_id distincts ; médiane/moyenne 2,85 ms, P95 3,07 ms, identity.json 21 641 octets, mode 0600. Rapport `reports/identity-cost-audit-2026-11-05.md`. Cela prouve le coût local, pas une identité humaine ni une résistance Sybil globale.
1. **`DOC.4` — index documentaire fail-closed** (fermé) : `scripts/check-docs.mjs` appelle `validateDocumentIndex` ; les 66 Markdown de `docs/` doivent être indexés exactement une fois avec titre, date ISO ou « Non indiquée », statut autorisé, thème et justification. Les entrées manquantes, doublons, chemins orphelins, dates vides et statuts inconnus font échouer le contrôle. Tests dédiés : 6/6 ; tests négatifs sur l'index réel : date vide puis statut invalide, exit 1 avec cause ; restauration vérifiée par SHA-256.
2. **`SEC.10` — métadonnées réellement visibles par le relais** (fermé) :
   `scripts/relay-metadata-audit.mjs` lance un relais, un émetteur et un
   récepteur réels derrière un proxy TCP d'observation, exige au moins deux
   connexions, deux `HELLO` et un fragment, et refuse de conclure si le marqueur
   de contenu apparaît sur le fil ou dans un journal. Mesure : 16 fragments,
   2 `HELLO`, 1 session, 21 640–47 566 octets, contenu absent. Rapport daté
   `reports/relay-metadata-audit-2026-11-05.md`, synthèse `docs/threat-msg.md`.
   Limite : une seule machine (loopback), métadonnées de graphe non masquées.
3. **`FV-SEC-05` — rotation et révocation de clé** (fermé) : `polygone clef-tourner`
   génère une ML-DSA-65 neuve, signe la nouvelle clé avec l'ancienne sur
   `polygone-key-rotation-v1`, révoque l'ancienne et conserve l'identité KEM
   (donc le `node_id`). L'écriture de `identity.json` est atomique
   (temporaire + `rename`, chmod 600). Preuves :
   `rotation_keeps_identity_and_signs_the_new_key` (preuve vérifiable avec
   l'ancienne clé), `revoked_key_cannot_sign_anymore`, `rotation_survives_process_restart`.
   Limite : la révocation est locale, pas encore propagée aux pairs.
4. **`FV-SEC-10` — limite par identité** (fermé, commit `4bd1474`) :
   `IdentityLimiter` dans `crates/relay/src/relay.rs` : 100 enveloppes/s par
   identité, compteur partagé entre reconnexions, purge des fenêtres inactives.
   Un dépassement est journalisé et l'enveloppe ignorée, **sans fermer la
   connexion ni toucher les autres pairs**. Preuves :
   `per_identity_limit_denies_only_the_abusive_peer`,
   `per_identity_limit_resets_after_window`.
5. **`CLI.6` — chaque refus traduit en action** (fermé) : `crates/client/src/refusal.rs`
   associe les 8 constantes `REASON_*` du daemon à une cause et une action ;
   `NodeError::Denied` affiche le code technique **et** l'action. Un motif
   inconnu reste visible avec la mention « rapport de bug ». Preuves :
   `every_reason_code_has_an_action` (compare la liste aux constantes du
   daemon, exige cause + action non vides), `unknown_denial_keeps_the_technical_code`.
6. **`SEC.1` — modèle de menace par service** (fermé) :
   `docs/THREAT-MODEL-SERVICES.md` couvre client, relais, gateway, daemon,
   serverless et petals ; chaque contrôle renvoie à un test existant, chaque
   service liste ses risques résiduels et ce qu'il ne protège pas. Limite
   assumée : un fichier de synthèse au lieu d'un fichier par service ;
   `FV-SEC-08` reste à l'état 5 faute d'audit externe.
7. **`GW.2` — latence réellement mesurée** (fermé, commit `e98bcb1`) :
   `POST /api/v1/nodes/ping` délivre un challenge signé, à usage unique et
   valable 30 s ; `POST /api/v1/nodes/pong` n'écrit `latency_ms` qu'après
   vérification du challenge et de la signature, avec RTT ≤ 60 s. Un pong sans
   ping ou rejoué est refusé 409 ; un nœud silencieux garde `None`. Preuves :
   `latency_populated_after_pong`, `pong_without_ping_is_refused`,
   `pong_replay_is_refused`, et la mesure `42` lue dans
   `/api/v1/network/status` sur un vrai socket TCP.
8. **`GW.1` — heartbeat signé persisté** (fermé, commit `6a84c9b`) : `POST /api/v1/nodes/heartbeat`
   vérifie ML-DSA-65 sur `polygone-heartbeat-v1\n{node_id}\n{timestamp}` avec la
   clé épinglée à l'appairage ; fenêtre anti-rejeu ±300 s ; écriture atomique
   de `nodes.json` ; rechargement au démarrage. Preuves :
   `heartbeat_requires_valid_signature` (403, état et disque inchangés),
   `heartbeat_survives_process_restart`, `heartbeat_outside_replay_window_is_refused`,
   `heartbeat_round_trip_over_real_http_is_persisted` (vrai socket TCP).
9. **`CLI.3` — profils d'usage** (fermé, commit `7a49150`) :
   `polygone profil <sobre|equilibre|genereux>` écrit `tier = "Custom"` + quatre
   seuils dans `~/.config/polygone/daemon.toml`, écriture atomique,
   `BehaviorConfig`/`SafetyMargins` conservés. Sans argument : profil courant,
   impact, chemin, et la phrase « aucune limite OS n'est posée sans isolation ».
   Preuves : `three_profiles_write_distinct_limits`,
   `profil_without_arg_prints_current`, session réelle où `polygoned status`
   relit `CPU 25% | RAM 30% | BW 40%` puis `CPU 75% | RAM 70% | BW 80%`,
   `profil turbo` → exit 1.
10. **Régression de latence corrigée** : OPS.2 cumulait deux sondes d'1 s
   (`refusals` puis `status`) ; `test_status_probe_does_not_hang_on_silent_daemon`
   a réellement échoué à 2,29 s. Budget partagé (`STATUS_PROBE_TIMEOUT / 2`),
   test repassé en 0,60 s.
11. **Registre de la vision** : `docs/FOUNDER_VISION.md`, construit depuis
   `_work/vision-long.md`, `00-SOURCES/VISION-EXTRAITE.md`, `VISION.md`,
   `ECOSYSTEM.md`, `PLAN-MAITRE.md`, `PLAN-FINITION.md` et `DECISION-POLY.md`.
12. **Outil d'avancement** : `scripts/vision-status.mjs` (compte les états 1–7,
   refuse une ligne mal formée, calcule validées / réalisables).

## Fichiers modifiés par ce lot

- `crates/client/src/identity.rs`, `crates/client/src/net/mod.rs`, `crates/client/src/net/tests.rs` (SEC.5) ; propagation des erreurs dans `hide.rs`, `main.rs`, `product.rs`, `tui.rs`.
- `scripts/identity-cost-audit.py`, `reports/identity-cost-audit-2026-11-05.md` (FV-SEC-06).
- `scripts/check-docs.mjs`, `scripts/document-index.mjs`, `scripts/document-index.test.mjs` (DOC.4).
- `scripts/hide-smoke.sh` (ports libres et binaires surchargeables).
- `scripts/relay-metadata-audit.mjs` (nouveau, SEC.10), `reports/relay-metadata-audit-2026-11-05.md`
- `docs/threat-msg.md` (nouveau, SEC.10), `docs/THREAT-MODEL-SERVICES.md`
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
| `cargo test --workspace -j 2` | **432 tests, 0 échec** |
| `cargo clippy --workspace --all-targets -- -D warnings` | **exit 0** |
| `cargo fmt --all -- --check` | **exit 0** |
| `node scripts/check-docs.mjs --tests 432` | **documents cohérents**, exit 0 — 66 Markdown, dates/statuts vérifiés |
| `node --test scripts/document-index.test.mjs` | **6/6**, dont date absente, statut inconnu, ligne incomplète, doublon et document oublié refusés |
| `node --test docs/atlas/agent/tests/backlog.test.mjs` | **6/6** |
| `node scripts/check-atlas.mjs` | **PASS** — 9 ouvertes, 39 terminées, 96 non promues |
| `node scripts/vision-status.mjs` | **67 exigences, 28 validées, 9 bloquées** (48,3 % des réalisables) |

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

**`DOC.4` est fermé** : 66/66 Markdown indexés ; date ou « Non indiquée » et statut autorisé obligatoires ; tests positifs et négatifs exécutés.

**SEC.5 est fermé** : identité et ancres écrites atomiquement en `0600`, répertoire `0700`, erreurs de chmod/lecture propagées, symlinks refusés.

**Prochaine tâche : G0 — réconcilier les 132 identifiants des six épiques avec les preuves et le backlog opérationnel (96 tâches encore non promues selon check-atlas).** Ne pas cocher une tâche sur la foi d'un ancien document ; marquer explicitement « à vérifier », « terminé prouvé », « bloqué » ou « sans objet ». Les preuves inter-hôtes `BLOCKS.9`, `MSG.6` et `FV-INF-04` restent bloquées tant qu'un second ordinateur physique n'est pas disponible.

## Commandes de reprise

```bash
cd ~/Polygone-Atlas/03-REALIGNED
git log --oneline -5
git status --short
node scripts/vision-status.mjs
cargo test --workspace -j 2
cargo clippy --workspace --all-targets -- -D warnings
node scripts/check-docs.mjs --tests 432
node --test scripts/document-index.test.mjs
python3 scripts/identity-cost-audit.py --samples 30
node --test docs/atlas/agent/tests/backlog.test.mjs
node scripts/check-atlas.mjs
```

## Règle de mise à jour

Ce fichier ne remplace ni `STATE.md` (journal des vérifications) ni
`FOUNDER_VISION.md` (registre). Après chaque lot : mettre à jour les trois,
exécuter les commandes ci-dessus, et ne passer une exigence à l'état 6 que si sa
preuve a été exécutée dans la session et citée dans le registre.
