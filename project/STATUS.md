# STATUS — état de mission, écrit pour être repris sans réexploration

> Point d'entrée d'une reprise. Il ne remplace pas [`../STATE.md`](../STATE.md)
> (journal des vérifications) : il dit **où en est la mission, quelle est la
> prochaine action, et comment la prouver.**
> Règle : aucune ligne `FAIT` sans commande et sortie associées.
> Dernière mise à jour : 2026-11-14 (ajout du prototype `fabric` ; session 9 + addendum).

## 1. Où travailler

| Élément | Valeur |
|---|---|
| Dépôt canonique | `~/Polygone-canonical` |
| Branche | `reconcile/2026-11` |
| Base | `main` (`f3d09c0`) + `realign/v2` (`967afdb`) + `p0/first-secure-network` (`d6e7229`) |
| Histoire d'archive | `realign/realign/v2` (jamais réécrite) |
| Anciens emplacements | `~/Projects/Polygone-real`, `~/Polygone-Atlas/03-REALIGNED` — **lecture seule** |
| Décision de base | `docs/decisions/ADR-0001-base-canonique-et-reconciliation.md` |

## 2. État vérifié

| Élément | État | Preuve |
|---|---|---|
| Fusion des deux lignées | FAIT | `comm -23` vide, 391 fichiers, ADR-0001 |
| Correctif P0/S1 intégré | FAIT | commit `9f5413c` |
| Quatre tests de sécurité du relais | FAIT, 1/1 chacun | `docs/proofs/security-2026-11-09.txt` |
| Stabilité du test de perte de bail | FAIT, 5 passes + suite lib | `docs/proofs/port-fix-2026-11-09.txt` |
| Scan de secrets (historique + arbre) | FAIT — aucune fuite | `docs/proofs/gitleaks-2026-11-09.txt` |
| Migration `peers.json` v1 → v2 | FAIT — 5 tests + fichier réel migré | `docs/proofs/peers-migration-2026-11-09.txt` |
| Compatibilité inter-versions | FAIT — strict refuse, transition accepte | `docs/proofs/cross-version-2026-11-09.txt` |
| Revue des 78 modifications de `Polygone-real` | FAIT — 4 fichiers porteurs réintégrés | `project/DIRTY-REVIEW.md` + `docs/proofs/dirty-2026-11-09.tsv` |
| Mesure de stockage restaurée | FAIT — `StorageInfo` + `mount_for_path` + `Platform::storage_info` | `daemon/src/resources/mod.rs`, portée par `runtime.rs` |
| Réservation de stockage dans le ledger | FAIT — réservations, libérations, expiration et persistance présentes | `daemon/src/socket.rs` : `Ledger::reserve/release/renew`, compteurs persistés et tests dédiés ; `docs/proofs/storage-ledger-2026-11-09.txt` |
| Plafond CPU par palier | FAIT — pool réservable = `floor(cœurs physiques × max_cpu_percent / 100)` ; `cpu_cores` reste la mesure physique | `allowed_cpu_cores()` ; `test_cpu_pool_is_capped_by_the_tier`, `test_cpu_above_the_tier_pool_is_denied` |
| Politique de confiance stricte | FAIT — `POLYGONE_TRUST_POLICY=strict` refuse un pair non ancré avant livraison, sans l'apprendre ; TOFU reste le défaut ; valeur inconnue = erreur | `crates/client/src/net/mod.rs` ; 3 tests réseau ; code `AUTH.UNKNOWN_PEER` |
| Négociation de protocole | FAIT — fenêtre `v1..=v2`, `hello` v1 avec `max_protocol_version: 2` et capacités, réponse dans la version de la requête ; le client retient `min(2, max daemon)` | `daemon/src/socket.rs`, `daemon/src/client.rs` ; 4 tests ; `docs/architecture/API-CONTRACTS.md` §1.6 |
| Persistance du ledger atomique et privée | FAIT — temporaire + `sync_all` + `rename`, `0600`, temporaire nettoyé | `test_ledger_persist_is_atomic_and_private` ; `docs/testing/VALIDATION-RESULTS.md` |
| Ledger illisible conservé en quarantaine | FAIT — `<nom>.corrupt-<horodatage>-<pid>`, journal d'erreur, redémarrage sur ledger vierge | `test_corrupt_ledger_is_quarantined_not_silently_erased` |
| Test de heartbeat déterministe | FAIT — comparé à la valeur renvoyée par la gateway, plus au timestamp client | `crates/gateway/tests/serve.rs` ; 502 tests workspace |
| Documentation de fondation (13 livrables) | FAIT — audit, roadmap, statut, architecture, menace, tests, produit | `project/FOUNDATION-AUDIT.md`, `project/FOUNDATION-ROADMAP.md`, `docs/architecture/`, `docs/security/THREAT-MODEL.md`, `docs/testing/`, `product/` |
| Modèle de tâche | FAIT — états explicites, transitions vérifiées, résultat BLAKE3, producteur imposé, bornes | `crates/core/src/task.rs` ; 15 tests ; `task::tests::lifecycle_reaches_success_in_order` |
| File et API de tâches | FAIT — créer, lire, annuler, expirer ; persistance atomique `0600` et quarantaine | `crates/gateway/src/tasks.rs`, `routes.rs` ; 10 tests ; `docs/architecture/API-CONTRACTS.md` §2.1 |
| Exécution d'une tâche | FAIT (local) — `echo`/`blake3`, transitions complètes, résultat BLAKE3 vérifié, échec nommé | `TaskKind::execute`, `TaskStore::with_executor` ; 5 tests |
| Reçu de résultat signé | FAIT (local) — signature ML-DSA-65 sur `task_id + producer + output_hash + produced_at` ; `NodeId` dérivé de la clé ; résultat non signé, altéré, étranger ou d'une autre tâche refusé | `TaskResult::verify`, `TaskRecord::complete` ; 20 tests `task` |
| Clé d'exécuteur persistante | FAIT — `gateway-signing.json` à côté de `tasks.json`, atomique `0600`, rechargée au démarrage | `load_or_create_executor_keypair` ; test de rechargement et de permissions |
| Reprise après interruption | FAIT (local) — une tâche `assigned`/`accepted`/`running` est reprise au démarrage ; une identité différente refuse de signer | `TaskStore::recover` ; 2 tests de reprise sur fichier réel |
| Publication / push | **INTERDIT** — voir §5 | — |

## 3. Prochaine action, dans l'ordre

1. Traiter `F1-API-01` : stabiliser les contrats des domaines encore absents
   (identity, resources, settings, events).
2. Traiter `F1-OBS-01` : rapport de diagnostic agrégé, sans secret.
3. Exécuter la CI GitHub Actions sur ce HEAD (pas encore vérifiée).
4. Organiser un test entre deux machines physiques distinctes ; les tests locaux
   actuels utilisent plusieurs processus sur une seule machine.
5. Rejouer Gitleaks sur l'arbre final immédiatement avant toute publication.
6. Terminer la revue des fichiers `a_examiner` de Polygone-real et corriger le
   décompte de `project/DIRTY-REVIEW.md` (`F1-DOC-02` / `F2-DOC-02`).
7. Ne rien pousser sans autorisation explicite.

La file de tâches (`F1-TASK-01`), l'exécuteur local (`F1-TASK-02`) et le reçu
signé vérifié de bout en bout (`F1-TASK-03`) sont livrés. Le placement
multi-nœuds et les événements restent absents.

Les vérifications locales de stockage, migration, formatage, Clippy, tests workspace,
documentation et smoke multi-processus ont été rejouées le 2026-11-09 ; voir
STATE.md et docs/proofs/.


## 4. Commandes utiles

```bash
cd ~/Polygone-canonical
./scripts/mission-resume.sh          # état + prochaine action, sans réexploration
make check                           # fmt + clippy + tests + docs
make smoke                           # parcours réel 3 processus
make migrate-proof                   # migration peers.json v1 → v2
./scripts/peers-migration-smoke.sh   # migration v1 → v2 sur un fichier réel
./scripts/cross-version-smoke.sh     # client d'octobre vs relais courant
~/bin/gitleaks git . --no-banner --redact
```

## 5. Ce qui interdit une publication

1. **Deux machines physiques** jamais testées.
2. **CI GitHub Actions** jamais exécutée sur cet arbre.
3. **Aucune autorisation explicite de publication** n'a été donnée.
4. Le scan de secrets doit être **rejoué** sur l'arbre final, immédiatement avant
   le push.

## 6. Limites de l'environnement

- `freebuff` en ligne de commande n'expose **aucun mode non interactif** : une
  session ne peut pas se relancer elle-même. La continuité repose sur ce fichier,
  `STATE.md`, `docs/atlas/agent/backlog.json` et les unités systemd déjà en place
  (`polygone-orchestrator.service`, `polygon-loop.service`).
- `gitleaks` non fourni par la distribution : binaire officiel 8.30.1 installé dans
  `~/bin/gitleaks`.
- Disque à ~92 % : les builds supplémentaires coûtent cher, `make clean` est à
  éviter sans sauvegarde.

## 7. Addendum 2026-11-14 — prototype `polygone-fabric`

Tranche verticale expérimentale de calcul bidirectionnel, **non intégrée au
produit**. Détail, limites et mesures :
[`docs/prototypes/FABRIC-2026-11-14.md`](../docs/prototypes/FABRIC-2026-11-14.md).
Matrice concept → source → état :
[`docs/prototypes/TRACEABILITY-2026-11-14.md`](../docs/prototypes/TRACEABILITY-2026-11-14.md).

- `cargo test -p polygone-fabric` : **32 réussis, 0 échec**.
- `cargo test --workspace --no-fail-fast` : **544 réussis, 0 échec** (512
  existants + 32 nouveaux).
- `cargo clippy -p polygone-fabric --all-targets -- -D warnings` : exit 0.
- `cargo fmt --all -- --check` : exit 0.
- Smoke TCP réel entre deux processus : `verified: true`, 27 ms observés,
  37 ms mur, serveur 3 364 Kio RSS. **Loopback uniquement.**
- Babel : **non implémenté**. `ADR-014` / `DEC-028` restent ouvertes ; le module
  `store` est un CAS expérimental, pas Babel.
- Décision produit à prendre : intégrer cette piste à `polygone compute` ou la
  retirer. Ne pas exposer un second chemin de calcul dans le produit.

**Limites non résolues** : pas de TLS, pas d'authentification de la demande,
pas d'anti-rejeu, pas de sandbox, pas de découverte mesh, pas de daemon, pas de
UI, aucun test entre deux machines physiques.

## 8. Consolidation Compute — 2026-11-14

- **Décision : C** — `polygone-fabric` reste un laboratoire opt-in, invisible
  dans le produit. Cible **B** : porter uniquement le planificateur de coût dans
  Compute, après mesure réelle de RTT/débit. Détail :
  [`docs/prototypes/FABRIC-DECISION-2026-11-14.md`](../docs/prototypes/FABRIC-DECISION-2026-11-14.md).
- Garde-fou appliqué : `publish = false`, binaire derrière `--features lab`,
  `cargo build --release` ne construit plus `polygone-fabric`.
- Sécurité Compute rejouée : grants signés/routés, cache anti-rejeu et rejeu
  réel via relais **verts**. À cette date, les 3 tests sandbox étaient
  **skippés** à cause d'un probe systemd défectueux ; le §9 corrige le probe et
  apporte les preuves réelles.
- **Validation physique : bloquée.** Un seul hôte Linux avec shell ; l'iPhone
  actif sur Tailscale ne peut pas exécuter le binaire. `docs/P0-DEUX-MACHINES.md`
  reste non validé.
- **Babel** : recherche documentaire versée dans
  [`docs/prototypes/BABEL-RESEARCH-2026-11-14.md`](../docs/prototypes/BABEL-RESEARCH-2026-11-14.md).
  `DEC-028` reste ouverte ; aucun code Babel ajouté.

## 9. P0 sécurité Compute — 2026-11-14

- **E2E implémenté** : `kem_hello` signé → `kem_offer` signé → `req` scellé
  ML-KEM-1024 + AES-256-GCM → `grant` scellé avec la même clé. Le relais ne lit
  plus la tâche ni la sortie ; un grant en clair est refusé. Preuve :
  [`docs/P0-COMPUTE-SECURITY-2026-11-14.md`](../docs/P0-COMPUTE-SECURITY-2026-11-14.md).
- Tests : `net::tests::` **46 réussis** ; `exec::tests::` **12 réussis** ; workspace
  **554 réussis, 0 échec**.
- **Sandbox réelle** : les 12 tests `exec::tests::` s'exécutent (le probe
  systemd utilise désormais une unité unique). Timeout 1 s → arrêt en 1,296 s ;
  `$HOME/.polygone` bloqué ; réseau privé `lo` seulement ; unités échouées
  nettoyées.
- **Correction mémoire (2026-11-15)** : l'affirmation « `MemoryMax` non
  appliqué » était fausse. La sonde manquait `MemorySwapMax=0` et le swap
  absorbait l'allocation. Mesure discriminante : 16 Mo survivent sous 32 Mo,
  96 Mo touchés sont tués, 96 Mo sans plafond survivent. `MemorySwapMax=0` est
  désormais posé ; `RLIMIT_AS` reste en seconde barrière.
- **Quota CPU mesuré (2026-11-15)** : 48 Mio SHA-256, 3,11 s à 100 % → 13,04 s à
  10 % (**4,2x** ; seconde mesure 2,63 s → 13,06 s, **5,0x**).
- **Vérification** : tout grant porte `verification: producer-signed` et la
  limite « signature ≠ justesse ». **L1 est implémenté en opt-in**
  (`--verifier-local` avec `--executer`) : le demandeur recalcule la tâche
  localement, le grant est annoté `recompute.*` avec les deux BLAKE3, une
  divergence conserve L0 et fait échouer la commande. Quorum (L2) et TEE (L3)
  restent absents.
- **Rotation et révocation (2026-11-15)** : le `node_id` dérive de la clé de
  signature, donc une rotation **change l'adresse** ; un grant signé par la
  nouvelle clé ne peut pas revendiquer l'ancien `node_id` (test
  `net::tests::key_rotation_changes_the_node_id_and_old_grants_stay_with_the_old_key`).
  Limite mesurée : la révocation est **locale**, l'ancienne clé reste valide
  pour l'ancien `node_id` ; aucune liste de révocation n'est distribuée.
  L'interruption en cours de session reste non testée (exige le déploiement
  deux hôtes).
- **Reprise** : `project/AUTONOMOUS_EXECUTION.md` porte l'état complet.
- **Bloqué** : toujours un seul hôte Linux avec shell ; aucun test physique
  Compute. Planificateur non porté faute de RTT/débit inter-hôtes.
- **Non commencé** : endpoint JSON/PWA de supervision, Device Reuse. Aucun
  compteur simulé n'a été ajouté.
- **Release : non.** Les critères P0 ne sont pas satisfaits.

## 10. Supervision agrégée et Device Reuse — 2026-11-15 (session 10)

- **`GET /api/v1/diagnostics`** : rapport agrégé en lecture seule
  (`crates/gateway/src/diagnostics.rs`). Réseau mesuré (actifs sur 300 s,
  latences mesurées vs non mesurées, médiane `null` sans mesure), comptage des
  tâches avec **les 10 états présents**, `executor_attached`, persistance
  déclarée. Les sections `daemon` et `refusals` sont `available: false` avec un
  motif : la gateway n'interroge pas le socket du daemon, elle n'invente donc
  ni réserve ni compteur de refus. Aucune clé de signature n'est recopiée.
- **Device Reuse** : `polygone_core::reuse::suggest_roles` propose `relais`,
  `supervision`, `stockage`, `calcul léger` à partir des mesures du daemon ;
  `polygone doctor` affiche la section « Rôles proposés ». Une mesure absente
  (batterie, température, activité, stockage) est écrite « contrainte non
  évaluée » et ne devient jamais un feu vert. **Aucun rôle n'est activé.**
- Preuves : `cargo test -p polygone-core reuse::` **8 réussis** ;
  `cargo test -p polygone-gateway diagnostics` **2 réussis** ;
  `cargo test -p polygone-client --bin polygone doctor` **8 réussis** ;
  `cargo test --workspace --no-fail-fast` **565 réussis, 0 échec**.
- Mesure : **100 fichiers Rust, 43 350 lignes**, 565 tests (voir
  `ARCHITECTURE.md`).
- **Console PWA** (`web/node.html`, `web/dashboard.js`) : panneau « Diagnostic
  agrégé » alimenté par la même route. Une gateway antérieure (404) ou
  injoignable laisse toutes les valeurs à « — » ; aucun zéro n'est affiché.
  Vérifié par `node --check web/dashboard.js` et par un appel HTTP réel
  (`curl /api/v1/diagnostics`) sur un socket TCP.
- **Ce qui reste absent** : flux d'événements (bus), historique de métriques,
  réglages par API, et toute activation réelle d'un rôle.
- **Bloqué, inchangé** : deux hôtes physiques, quorum L2, TEE L3, intervention
  de session deux hôtes, `DEC-028` (Babel). Planificateur toujours non porté :
  aucun RTT/débit inter-hôtes mesuré.
