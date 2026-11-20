# STATUS — état de mission, écrit pour être repris sans réexploration

> Point d'entrée d'une reprise. Il ne remplace pas [`../STATE.md`](../STATE.md)
> (journal des vérifications) : il dit **où en est la mission, quelle est la
> prochaine action, et comment la prouver.**
> Règle : aucune ligne `FAIT` sans commande et sortie associées.
> Dernière mise à jour : 2026-11-20 (fusions PR #7 calcul distant + PR #8 nodes,
> gates rejoués sur l'arbre fusionné : 711 tests, fmt/clippy/check-docs verts ;
> PR #3 obsolète fermée — voir §15)

## 1. Où travailler

| Élément | Valeur |
|---|---|
| Dépôt canonique | `~/Polygone-canonical` |
| Branche | `main` (`20cf055` — PR #7 et #8 fusionnées le 2026-11-20, §15) |
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
| Publication / push | FAIT — `reconcile/2026-11` poussée sur `lvs0/Polygone-Network`, PR #4 **fusionnée** le 2026-11-19 (merge `8307094`), réconciliation ADR-0004 | §11-12 |

## 3. Prochaine action, dans l'ordre

> **État au 2026-11-20 : voir §15** — les trois premiers points ci-dessous sont
> clos (PR #4, #6, #7 et #8 fusionnées ; release v2.1.0 publiée ; aucun PR ouvert).

1. Vérifier le verdict de la CI GitHub Actions (run 44486416115, branche
   `reconcile/2026-11`) ; corriger si rouge, ne pas contourner.
2. Release : `./scripts/release-dry-run.sh` avec les quatre binaires release,
   puis publication du tag si la CI est verte et le dry-run conforme.
3. Réconcilier `github/main` ↔ `reconcile/2026-11` (18 conflits, lignées
   divergentes) — mission distincte, arbitrages ADR-0001 à rejouer.
4. Mesurer RTT/débit inter-hôtes (Hope ↔ Cue) et porter le planificateur de
   coût dans Compute (décision C, cible B).
5. Traiter `F1-API-01` : stabiliser les contrats des domaines encore absents
   (identity, resources, settings, events).
6. Test LAN/NAT : un pair derrière une box résidentielle (le WAN Tailscale
   est prouvé, pas la traversée de NAT).
7. Quorum L2 (attribution de faute = décision humaine, ADR à porter) ; TEE L3.

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

## 5. Gates de publication — état au 2026-11-19

1. **Deux hôtes distincts** : LEVÉ — exécuté le 2026-11-19, Hope ↔ cue-sandbox
   (WAN Tailscale, deux noyaux, horloges 0 s). Preuve :
   `docs/proofs/P0-DEUX-HOTES-2026-11-19.md`. Limite : LAN/NAT non testé.
2. **CI GitHub Actions sur l'arbre** : LEVÉ — run 44501139127 **vert**
   (Test ubuntu ✓, Test macos ✓, Smoke ubuntu ✓ ; canari Windows : échec
   attendu et documenté). HEAD 26c52d9.
3. **Autorisation explicite** : DONNÉE — mandat opérateur 2026-11-19
   (« finir la production en toute autonomie » ; accès SSH Cue confirmé).
4. **Gitleaks rejoué avant push** : FAIT — 292 commits, 0 fuite (2026-11-19).

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
- **Bloqué, inchangé** : quorum L2, TEE L3, `DEC-028` (Babel). Planificateur
  toujours non porté : aucun RTT/débit inter-hôtes mesuré. **Le blocage deux
  hôtes est levé le 2026-11-19 (§11).**

## 11. Production — 2026-11-19 (mandat opérateur)

Mandat : « finir la production en toute autonomie » ; « tu peux accéder en SSH
à la sandbox Cue ». Le disque fait foi ; cette section prime sur les §8-10
pour tout ce qu'elle contredit.

| Élément | État | Preuve |
|---|---|---|
| WIP sessions 9-10 commité | FAIT — 5 commits structurés (`8c6704f`→`d8d08f3`), arbre propre | `git log`, sauvegarde `~/Ops/backups/polygone-unification-2026-11-09/canonical-wip-20261119-075311.tar.gz` (SHA256 `dd47b8d6…`) |
| D2 tranchée | FAIT — ADR-0002, `node_id` = BLAKE3(contexte ‖ clé ML-DSA-65)[..16], DEC-SEC-03 SUPERSEDED | `docs/decisions/ADR-0002-identite-noeud-cle-signature.md`, registre de consolidation |
| Preuve L1 persistée | FAIT — ADR-0003, `net::persist_l1_evidence`, 4 tests, émission réelle deux hôtes | commit `804d2ee`, `docs/proofs/P0-DEUX-HOTES-2026-11-19.md` §4 |
| Test deux hôtes | FAIT — Hope ↔ cue-sandbox, WAN Tailscale, deux noyaux, horloges 0 s ; protocole P0 complet + contrôles négatifs sur le fil | `docs/proofs/P0-DEUX-HOTES-2026-11-19.md` (commit `017a6f4`) ; limites §2 : LAN/NAT, pcap non rapatrié |
| Tests workspace | FAIT — **586 passés, 0 échec**, 24 suites ; clippy `--workspace --all-targets -D warnings` exit 0 ; fmt exit 0 | `TMPDIR=/tmp cargo test --workspace -j 2` (2026-11-19) |
| Gitleaks | FAIT — 292 commits, 0 fuite, rejoué avant push | `~/bin/gitleaks git . --no-banner --redact` |
| Push + PR | FAIT — `reconcile/2026-11` → `lvs0/Polygone-Network`, PR #4 | https://github.com/lvs0/Polygone-Network/pull/4 |
| CI sur l'arbre | **VERTE** — run 44501139127 (HEAD 26c52d9) : Test ubuntu ✓, Test macos ✓, Smoke ubuntu ✓ (installeur, p0-two-process, smoke-commands, gateway-smoke, CI honesty, secrets) ; canari Windows échec attendu. 9 runs : probe sandbox discriminant (exit 218 runners), Default MacOSPlatform, cfg cgroup linux/test, **socket accepté repassé bloquant (bug produit macOS/BSD : O_NONBLOCK hérité)**, tests sysfs/PrivateTmp gatés Linux | commits `22cc718`→`26c52d9` |
| Release dry-run | FAIT — archive `polygone-linux-x86_64.tar.gz` acceptée par les règles de l'installeur, altération détectée | `docs/proofs/release-dry-run-2026-11-19.txt`, `dist/SHA256SUMS` |
| Version | 2.1.0 (la release v2.0.0 du 2026-08-20 existe déjà ; ne pas l'écraser) | `Cargo.toml` |
| Release publiée | FAIT — https://github.com/lvs0/Polygone-Network/releases/tag/v2.1.0 : 4 cibles (linux x86_64/aarch64, macos x86_64/aarch64) + SHA256SUMS, run 44505345176 vert. Asset macos-x86_64 vérifié x86_64 (cross-compilation ; macos-latest est arm64, macos-14 retiré). Installation réelle testée : `POLYGONE_VERSION=v2.1.0 install.sh` en répertoire isolé → checksums vérifiés, `polygone id` fonctionnel | `gh release view v2.1.0`, test install 2026-11-19 |
| Fusion github/main | **FAITE** — merge `78e9447` (ADR-0004, arbitrage inspecté fichier par fichier), PR #4 fusionnée → `main` = `8307094`, CI verte run 44558244332 | §12 |

**Prochaine action** : mesure RTT/débit inter-hôtes (Cue sandbox, hors ligne
par intermittence — sonde active) → planificateur de coût ; F1-API-01
(contrats documentés, §12 — implémentation progressive par domaine) ;
test LAN/NAT (accès physique).

## 12. Réconciliation + contrats F1-API-01 — 2026-11-19 (soir, autonomie)

Mandat : « Contine en tt autonomie sur Polygone ». Le disque fait foi.

| Élément | État | Preuve |
|---|---|---|
| Réconciliation `github/main` | **FAITE** — lignée septembre (17 commits, ancêtre `060c11d` du 02/09) fusionnée dans la lignée canonique ; merge `78e9447` à deux parents (`c13cdf6` + `23d27ac`), aucun rewrite, aucun force-push | ADR-0004 (table d'arbitrage : 20 conflits + 11 auto-fusions inspectées, 24 ajouts purs conservés en couche historique, 2 fichiers mock exclus) |
| Arbre fusionné = arbre vérifié | **PROUVÉ** — `git diff c13cdf6 78e9447 -- '*.rs'` = 0 ; seuls changements : docs (ADR, INDEX, bandeaux, ARCHITECTURE re-mesuré 101 fichiers / 44 332 lignes) | commandes git, §6 ADR-0004 |
| Gates locaux | fmt OK ; clippy `--workspace --all-targets -D warnings` **RC=0** ; `TMPDIR=/tmp cargo test --workspace --no-fail-fast -j 2` **586 passés / 0 échec / 24 suites** ; check-docs OK (96 documents) ; check-ci-honesty OK ; gitleaks **313 commits, 0 fuite** | logs `~/.hermes/cache/scratch/reconcile-*.log` |
| CI sur l'arbre fusionné | **VERTE** — run 44558244332 : Test ubuntu ✓, Test macos ✓, Smoke ubuntu ✓ ; canari Windows échec attendu (non bloquant, documenté) | https://github.com/lvs0/Polygone-Network/actions/runs/44558244332 |
| PR #4 fusionnée | **FAITE** — `main` = `8307094` (2026-11-19T16:07:42Z), arbre identique à `78e9447` ; retour arrière = `git revert -m 1` (ADR-0004 §7) | `gh pr view 4` state=MERGED |
| Couche septembre | conservée historique : `GOVERNANCE.md`, `DOSSIER-POLYGONE-DREAM.md`, `docs/COMPANY.md`, `docs/RAPPORT-PRE-SHIP-2026-09-10.md` (indexés `daté` + bandeaux), `scripts/company*`, `web-pwa/` (stub SW pass-through, **ne satisfait pas** la PWA offline ; console produit = `web/`) | ADR-0004 §5 |
| F1-API-01 — contrats documentés | **FAIT** — `docs/architecture/API-CONTRACTS.md` §8 : plan de stabilisation progressive des 8 domaines (prochain pas + test de contrat + dépendance par domaine), sources revérifiées sur `8307094`. Implémentation : NON commencée (progressive, ne pas figer trop tôt) | §8, roadmap F1-API-01 |
| Mesures RTT/débit | **BLOQUÉ** — Cue sandbox (`root@100.99.15.112`) hors ligne par intermittence (suspension idle) ; sonde SSH active en arrière-plan ; ne JAMAIS ouvrir l'URL Tailscale SSH check | tentatives datées 19/11 soir |

**Prochaine action** : dès Cue réveillé → mesures RTT/débit inter-hôtes
(→ planificateur de coût) ; sinon F1-API-01 premier pas implémentable sans
dépendance : `GET /api/v1/identity` ou `GET /api/v1/resources` (+ tests de
contrat, §8). LAN/NAT et quorum L2 restent bloqués (physique / second opérateur).

## 13. Prototype complet — 2026-11-19 (nuit, autonomie)

Mandat : « Build le prototype entier et tu pourras t'arrêter ». Périmètre
déduit des docs produit (`PRODUCT-FOUNDATION`, `PRODUCT-READINESS`,
`UI-API-MAPPING`), pas inventé. Branche `prototype-complet` depuis `b63e3a8`.

| Élément | État | Preuve |
|---|---|---|
| Protocole daemon étendu | **FAIT** — `ledger_request`/`ledger_response` additifs, capacité `ledger` annoncée au `hello`, `Ledger::snapshot()` trié et normalisé (legacy → même forme, `expires_at` omis) ; ancien daemon → `unknown_type`, jamais un faux ledger vide | `daemon/src/socket.rs`, `client.rs` ; test `test_ledger_reflete_allocations_releases_et_capacite` (allocation → bail visible → release → vide) |
| `GET /api/v1/identity` | **FAIT** — identité publique locale, `node_id` dérivé ADR-0002 ; les champs `*_sk_hex` ne sont même pas désérialisés ; absente → 404, illisible → 500 | `local.rs`, test « aucun matériel secret publié » + smoke vivant (pseudo réel, node_id = CLI `polygone id`) |
| `GET /api/v1/daemon` + `/resources` | **FAIT** — relais du `hello` et du `status_response` **par les types partagés** (`polygoned`), parité champ par champ prouvée contre un vrai daemon (omissions comprises) ; absent → 503 `daemon_absent`, jamais des zéros | `relay.rs`, test `relais_daemon_parite_complete_sur_un_vrai_daemon` + smoke vivant |
| `GET /api/v1/settings` | **FAIT** — lecture seule, `source: file\|defaults`, tier au vocabulaire du fil ; config invalide → 500 explicite, jamais de defaults masquant une casse | `local.rs`, 6 tests + smoke vivant (fichier isolé `tier=performance` lu, daemon aligné) |
| `GET /api/v1/usage` | **FAIT** — ledger actif + refus relayés ; capacité vérifiée avant demande (ancien daemon → 501) ; pas d'historique inventé | test parité socket + smoke vivant (refus `policy_shrinking: 3` relayés, RAM hôte saturée) |
| `GET /api/v1/events` (F1-EVT-01) | **FAIT (flux borné)** — anneau 512, ids monotones, curseur `since`, `gap` signalé, `dropped_total`, plafond 500, filtre `kind`, 400 sur paramètre inconnu ; 13 kinds réels ; sonde daemon 5 s (`resources` sur changement de `seq`, `daemon_up`/`daemon_absent` sur transition) ; push SSE/WebSocket NON fait (polling à curseur assumé, documenté) | `events.rs` (5 tests), `local_api.rs` (ordre, gap, rejeu, filtre, 400) + smoke vivant |
| Événements de tâches | **FAIT** — `task_created`/`cancelled`/`resumed`/`expired` émis par le magasin aux transitions réelles ; jetons d'appairage et challenges JAMAIS dans le bus | `tasks.rs` (`with_events`, `sweep_and_emit`), test « création de tâche publie un événement réel » |
| Écrans | **FAIT** — `node.html` étendu (identité, daemon, ressources mesurée/réservable/appliquée, réglages lecture seule), `compute.html` (tâches echo/blake3 signées, annulation), `activity.html` (flux à curseur + bandeau gap, ledger actif, refus) ; nav étendue, coque PWA `polygone-v3` | `web/*.js` + smoke vivant : pages 200, tâches exécutées signées, événements affichables |
| Smoke test bout-en-bout | **FAIT** — daemon + gateway réels en HOME isolé : identité réelle sans secret, relais alignés, tâches `echo`/`blake3` `succeeded` signées ML-DSA-65, événements `gateway_up`/`daemon_up`/`resources`/`task_created`, daemon tué → 503 sur les 3 relais + 1 événement `daemon_absent`, identity/settings restent 200 | journal `~/.hermes/cache/scratch/proto-smoke/`, sortie du 19/11 |
| Gates | fmt OK ; clippy workspace `-D warnings` **RC=0** ; **612 tests / 25 suites / 0 échec** (workspace complet, `--no-fail-fast`) ; check-docs RC=0 (96 documents) ; check-ci-honesty RC=0 ; gitleaks avant push | logs `proto-clippy2.log`, `proto-test-full2.log`, `proto-docs.log`, `proto-honesty.log` |
| Contrats + docs | `API-CONTRACTS.md` §1 (ledger), §2 (6 routes), **§2.4** (contrats détaillés), §6/§8 à jour ; `UI-API-MAPPING` §9 (écrans livrés) ; `PRODUCT-READINESS` verdict levé pour l'activité ; roadmap F1-API-01 PARTIAL (locaux faits), F1-EVT-01 PARTIAL (push non fait), F2-PROD-01 DONE | diffs du commit |

**Reste ouvert (honnête)** : `nodes` révocation par API, `network`
routage/débit (mesures inter-hôtes, Cue hors ligne), `tasks` placement
multi-nœuds, écritures identité/réglages (F1-SEC-02), historique de
consommation (F1-USG à ouvrir — le ledger n'enregistre pas les baux clos),
livraison push d'événements. LAN/NAT et quorum L2 : physiques / second
opérateur, hors autonomie.

**Prochaine action** : PR #6 **FUSIONNÉE** le 2026-11-19 (`main` = `1956f36`,
CI verte run 44582207237 : ubuntu ✓ macos ✓ smoke ✓, canari Windows échec
attendu). Prototype livré et publié. Suite : F1-OBS-01 (diagnostic exportable
sans secret), puis F1-SEC-02 (écritures bornées) ; mesures inter-hôtes dès Cue
réveillé (RTT/débit → placement et planificateur de coût).

## 14. Domaine nodes local — 2026-11-20 (mandat « finis Polygone »)

WIP trouvé sur `mission/nodes-revocation` (routes + écran Réseau non commités) :
inspecté, complété, vérifié, publié. Le disque fait foi.

| Élément | État | Preuve |
|---|---|---|
| `GET /api/v1/nodes` | **FAIT** — ancres appairées, champs publics seulement (clé de vérification, jamais privée), ordre stable, `active` ≤ 300 s, `latency_ms: null` si non mesuré, `persisted` honnête | `routes.rs::list_nodes`, tests `nodes_list_empty_is_honest`, `nodes_list_shows_paired_node_public_only` (refuse tout champ hors liste publique) |
| `POST /api/v1/nodes/{id}/revoke` | **FAIT** — retire l'ancre, persiste hors verrou, événement `node_revoked` ; 404 sans écriture, 400 id invalide | `routes.rs::revoke_node`, 5 tests dont persistance rechargée et **heartbeat bloqué après révocation** (ajouté ce jour) |
| Contrats | **FAIT** — `API-CONTRACTS.md` §2 (table), **§2.5** (domaine nodes), §6 et §8 à jour ; kinds d'événements 13 → 14 | diffs du commit |
| Écran Réseau/Appareils | **FAIT** — `mesh.html`/`mesh.js` : données réelles (`/network/status`, `/nodes`, `/events`), bouton Révoquer avec état `revoking`, null ≠ 0, aucune donnée fictive ; nav « Mesh » → « Réseau » ; coque `polygone-v4` | `node --check` OK, `UI-API-MAPPING.md` §9 |
| Gates | fmt RC=0 ; clippy workspace `-D warnings` RC=0 ; **619 tests / 25 suites / 0 échec** ; ARCHITECTURE re-mesuré (105 fichiers, 46 087 lignes) | log `~/.hermes/cache/scratch/nodes-gates.log` |
| Limites assumées | révocation **locale** : pas de CRL distribué (décision protocole ouverte) ; `peers.json` CLI non modifié par la route ; un ré-appairage explicite recrée l'ancre (nouvelle décision de confiance) | §2.5 |
| Cue | hors ligne (SSH timeout 2026-11-20) — mesures RTT/débit toujours bloquées | tentative datée |

**Prochaine action** : fusion de la PR nodes (CI verte exigée), puis
F1-OBS-01 (rapport de diagnostic exportable sans secret), écritures bornées
identité/réglages, F1-NET-02/F1-SVC-01..03 (câbler ou requalifier honnêtement).
LAN/NAT, quorum L2, TEE, Babel : physiques / décision humaine, hors autonomie.

## 15. Fusion finale — 2026-11-20 (mandat « finis Polygone »)

| Élément | État | Preuve |
|---|---|---|
| PR #7 calcul distant | **FUSIONNÉE** — checks bloquants verts (ubuntu, macos, smoke) ; canari Windows échec attendu | `gh pr merge 7 --merge`, RC=0 ; run 44703014260 **vert** |
| PR #8 nodes + révocation | **FUSIONNÉE** — `mergeable=MERGEABLE`, ubuntu/macos/smoke verts | `gh pr merge 8 --merge`, RC=0 ; run 44703480350 **vert** |
| `main` | `f123965` (fusions) → `20cf055` (état documentaire) ; gates rejoués sur l'arbre fusionné | `git log --oneline` |
| Gates arbre fusionné | fmt RC=0 ; clippy `--workspace --all-targets -D warnings` RC=0 ; **711 tests / 0 échec** ; check-docs « documents cohérents avec le code » ; gitleaks 0 fuite | logs `~/.hermes/cache/scratch/main-*.log` |
| Mesure | **107 fichiers Rust, 46 642 lignes, 711 tests** ; gateway 15 fichiers / 6 260 lignes / 97 tests | `scripts/check-docs.mjs --tests 711` |
| PR #3 | **fermée (obsolète)** — basée sur `23d27ac` (septembre), CONFLICTING ; son objet fmt/clippy est déjà vert sur `main` | commentaire + `gh pr close 3` |

**Prochaine action** : plus aucun PR ouvert. Suite fonctionnelle non bloquante —
F1-OBS-01 (diagnostic exportable sans secret), écritures bornées identité/réglages
(F1-SEC-02), câblage F1-NET-02/F1-SVC-01..03. Bloqués par la physique ou une
décision humaine : mesures RTT/débit inter-hôtes (Cue hors ligne), LAN/NAT,
quorum L2, TEE L3, Babel.
