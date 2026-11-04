# STATE — état courant

> Mis à jour par l'orchestrateur après chaque bloc de travail.
> Ne jamais écrire ici une valeur non vérifiée.

## Dernière session

| Champ | Valeur |
|---|---|
| Date | 2026-11-04 |
| Branche | `realign/v2` — locale, non poussée ; `realign/v1` est la branche publiée |
| HEAD | voir `git log -1 --format='%h %s'` (jamais recopié à la main) |
| Arbre | propre |
| Origine | `lvs0/Polygone-Network` ; `origin/realign/v1` existe, `origin/realign/v2` non |

## Vérifications exécutées

| Commande | Résultat | Date |
|---|---|---|
| `cargo test --workspace` | **348 tests, 0 échec** | 2026-11-02 |
| `cargo fmt --all -- --check` | OK (hérité du dépôt canonique) | 2026-11-02 |
| `cargo clippy --workspace --all-targets -- -D warnings` | OK (hérité du dépôt canonique) | 2026-11-02 |
| Archive P0.1 : listing + extraction + `cmp` ×3 + HEAD | **PASS** — 6 204 entrées, `tar exit=0`, stderr vide, HEAD `f3d09c0` et 65 fichiers modifiés identiques | 2026-11-03 |
| Inventaire P0.2 : 6 commits locaux + 17 distants | **Fait** — décisions par commit dans `docs/RECONCILIATION.md` | 2026-11-03 |
| Les 65 fichiers non commités du canonique sont-ils perdables ? | **Non** — 60 fichiers byte-identiques ici, 3 dossiers identiques (`diff -rq`), les 2 restants sont `.freebuff-tasks/` et `.specify/` (outillage). Déjà commités sur `realign/v1`. | 2026-11-03 |
| P0.3 : documentation factuellement fausse | **Corrigé** — `ARCHITECTURE.md` passe de « 4 crates, ~9 600 LOC, 109 tests » à **7 membres, 28 456 LOC, 4 binaires, 348 tests** (mesuré). `threat-commodity.md` ne recopie plus de compte. `CORE-COMPLETE.md` reçoit un bandeau d'instantané daté. | 2026-11-03 |
| `cargo test --workspace` sur `realign/v1` | **348 tests, 17 suites, 0 échec** | 2026-11-03 |
| P1.1 : capacités séparées (`Capacity`, `enforced`, refus distingué) | **353 tests, 0 échec** · `cargo clippy --workspace --all-targets -- -D warnings` **exit 0** · `cargo fmt --all -- --check` **OK** | 2026-11-03 |
| Plan Atlas importé | 132 identifiants dans docs/atlas/plan/epiques/ ; backlog opérationnel : 18 ouvertes + 20 terminées dans docs/atlas/agent/backlog.json ; 103 spécifications épiques non promues | 2026-11-04 |
| Cohérence backlog ↔ épiques | Contrôle automatisé : 132 identifiants uniques ; 38 entrées opérationnelles (18 ouvertes, 20 terminées), 29 correspondances épiques et 9 tâches transversales ; 103 tâches épiques non promues | 2026-11-04 |
| DAEMON.1/DAEMON.3 : expiration et renouvellement des baux | **fait** — 5 tests ajoutés (échéance, adoption d'un bail hérité, relecture du format historique, renouvellement, refus d'un bail inconnu/échu) · `315f0eb` | 2026-11-03 |
| DOC.1 : contrôle de cohérence documentaire | **fait** — `node scripts/check-docs.mjs --tests 361` → « documents cohérents », **exit 0** ; chiffre cassé exprès → **exit 1** et l'écart nommé · `288d67e` | 2026-11-03 |
| UI.1/UI.2/UI.5 : plus de chiffres inventés, plus de fausse promesse | **fait** — `Math.random` → 0 dans `web/dashboard.js`, `node --check` OK, libp2p/Five tabs/Sparklines retirés des deux pages · `f2e0a59` | 2026-11-03 |
| `cargo test --workspace` après ce lot | **361 tests, 18 suites, 0 échec** (exit 0) | 2026-11-03 |
| `cargo clippy --workspace --all-targets -- -D warnings` | **exit 0** | 2026-11-03 |
| `cargo fmt --all -- --check` | **OK** | 2026-11-03 |
| Orchestrateur : tâches bloquées (AGT.1/AGT.2) | **fait** — `node --test docs/atlas/agent/tests/backlog.test.mjs` → **5/5**, dont « une tâche bloquée est refusée en nommant son bloqueur » (exit 3) | 2026-11-03 |
| hope-gateway : suite et démarrage | **44 tests, 0 échec** ; `server.mjs` + `GET /healthz` → **200** au premier essai ; CI ajoutée (`83ed22a`) | 2026-11-03 |
| MSG.1 : TTL de session contre les documents | **fait** — constante 300 s → **30 s**, reliée à `PUBLISHED_SESSION_TTL_SECS` ; trois documents alignés et lus à la compilation par `test_session_ttl_matches_the_published_promise` · `ac71668` | 2026-11-03 |
| DAEMON.13 : crash réel (SIGKILL du consommateur, puis du daemon) | **fait** — `node scripts/crash-recovery.mjs` → **9/9 PASS**, exit 0 : bail durable après SIGKILL du daemon, marge rendue à l'échéance, redémarrage qui répond | 2026-11-03 |
| MESH.1 : aucune annonce sur une interface non autorisée | **fait** — `parse_ip_addr_output` + `select_interfaces` remplacent le broadcast depuis `0.0.0.0` ; loopback, link-local, VPN et conteneurs exclus, `POLYGONE_MESH_IFACE` ne peut que restreindre. 4 tests, dont « une allowlist ne ressuscite pas `tailscale0` » et le témoin positif d'un datagramme bien reçu | 2026-11-04 |
| SEC.4 : usurpation de `from` figée en régression nommée | **fait** — `refuse_from_spoofing_through_relay` : trame forgée refusée **et** trame légitime acceptée dans la même session ; `cargo test -p polygone-relay` → **17/17** | 2026-11-04 |
| `cargo test --workspace -j 2` après ce lot | **370 tests, 0 échec** (exit 0) | 2026-11-04 |
| `cargo fmt --all -- --check` | **OK** (exit 0) | 2026-11-04 |
| `scripts/check-docs.mjs` : branche de `STATE.md` comparée au checkout | **ajouté** — `git rev-parse --abbrev-ref HEAD` contre la ligne `| Branche |` ; une divergence nomme les deux branches et sort en 1 | 2026-11-04 |
| `scripts/check-docs.mjs` : somme des LOC du tableau vérifiée comme celle des tests | **ajouté** — c'est ce contrôle qui manquait quand `client` affichait 11 365 pour 11 677 mesurées | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 370` | **documents cohérents**, exit 0 ; deux négatifs vérifiés : branche forcée à `realign/v1` → exit 1 (`[state.branche]`), LOC volontairement faussées → exit 1 (`[architecture.somme.LOC]`) | 2026-11-04 |
| `node --test docs/atlas/agent/tests/backlog.test.mjs` | **5/5**, exit 0 — 19 tâches ouvertes, 15 fermées, JSON valide | 2026-11-04 |
| SEC.3 : rejeu capturé sur le fil, refusé par le **vrai** relais | **fait** — `replay_through_relay_is_refused_with_an_explicit_code` : session déchiffrée (témoin positif), rejeu immédiat → `REPLAY.SESSION_ALREADY_COMPLETED`, rejeu à +10 min contre un récepteur frais → `REPLAY.STALE_TIMESTAMP`. Le refus n'est plus un `None` muet : `Refusal::code()` le nomme | 2026-11-04 |
| `cargo test --workspace -j 2` après SEC.3 | **371 tests, 0 échec** (exit 0) | 2026-11-04 |
| `cargo clippy --workspace --all-targets -- -D warnings` | **exit 0** | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 371` | **documents cohérents**, exit 0 | 2026-11-04 |
| DOC.2 : index complet de `docs/` | **fait** — `docs/INDEX.md` recense les 62 Markdown récursifs (date, statut, thème et justification) ; `cargo test -p polygone-client --test doc_index` **1/1** ; test négatif : retrait de `docs/BUDGET.md` → échec nommé, index restauré | 2026-11-04 |
| `cargo test --workspace -j 2` après DOC.2 | **372 tests, 0 échec** (exit 0) | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 372` | **documents cohérents avec le code**, exit 0 | 2026-11-04 |
| Validation DOC.2 | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `node scripts/check-atlas.mjs`, tests agent **6/6**, `git diff --check` : tous verts | 2026-11-04 |
| CLI.1 : `polygone doctor` lit l'isolation OS au lieu de la déclarer | **fait** — section « Isolation OS (ce processus) » alimentée par `/proc/self/cgroup`, `memory.max` et `cpu.max` : `max` → « illimitée », fichier illisible → « Non mesurée », jamais un `0.00 Gio` inventé. Sortie réelle contre le daemon du projet : `Cgroup : /user.slice/user-1000.slice/session-42.scope (scope Polygone : non)`, `MemoryMax : illimitée`, exit 0. 7 tests `doctor`, 7 tests `node_isolation` | 2026-11-04 |
| `cargo test --workspace -j 2` après CLI.1 | **384 tests, 0 échec** (exit 0) | 2026-11-04 |
| `cargo clippy --workspace --all-targets -- -D warnings` après CLI.1 | **exit 0** | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 384` | **documents cohérents**, exit 0 | 2026-11-04 |
| CLI.2 : pause bornée, bail rendu, reprise refusée si expirée | **fait** — session réelle contre le daemon du projet : `pause --duree 10m` → exit 0, `pause.json` écrit (`resumes_at = paused_at + 600`) ; `reprendre` → `bail obtenu … 0.25 Gio · 1 cœur(s) — rendu aussitôt : oui`, exit 0, intention consommée ; sans pause → exit 1 `aucune pause en cours` ; `--duree 9m` → exit 1 `durée hors bornes` ; intention antidatée → exit 1 `pause expirée depuis 60 s`, aucun bail demandé. 7 tests `pause` | 2026-11-04 |
| `cargo test --workspace -j 2` après CLI.2 | **391 tests, 0 échec** (exit 0) | 2026-11-04 |
| `cargo clippy --workspace --all-targets -- -D warnings` après CLI.2 | **exit 0** | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 391` | **documents cohérents**, exit 0 | 2026-11-04 |
| CLI.5 : désinstallation énumérée, confirmée, sans reste | **fait** — session dans un `HOME` temporaire : sans `--confirmer`, exit 1 `confirmation requise` et **arborescence identique** (5 fichiers intacts) ; avec `--confirmer`, exit 0, 6 chemins supprimés, il ne reste que `Documents/keep.txt` (jamais touché) ; un chemin hors racines est refusé par `remove`. 4 tests `uninstall` | 2026-11-04 |
| `cargo test --workspace -j 2` après CLI.5 | **395 tests, 0 échec** (exit 0) | 2026-11-04 |
| `cargo clippy --workspace --all-targets -- -D warnings` après CLI.5 | **exit 0** | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 395` | **documents cohérents**, exit 0 | 2026-11-04 |
| DOC.3 : bandeaux `Instantané daté` sur les documents faux | **fait** — les **29** documents classés `daté` dans `docs/INDEX.md` portent le bandeau et un lien vers `ETAT-REEL.md`/`STATE.md` ; `grep -L "Instantané daté"` sur ces 29 → **aucun fichier** ; `check-docs` gagne le contrôle `docs.instantané` : bandeau retiré de `RECONCILIATION.md` → **exit 1** nommant le fichier, restauré → **exit 0**. Aucun contenu historique réécrit | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 395` après DOC.3 | **documents cohérents**, exit 0 — « documents datés : 29 vérifiés, tous bandeautés » | 2026-11-04 |
| CI.1 : matrice honnête sur Windows | **fait** — `windows-latest` retiré du job bloquant `test` (il ne peut pas être vert : `daemon/src/resources/mod.rs` porte un `compile_error!`) ; job `windows-canary` en `continue-on-error: true` qui imprime la raison. Contrôle local `node scripts/check-ci-honesty.mjs` → exit 0 ; négatif : `windows-latest` réinjecté dans la matrice bloquante → **exit 1** nommant le job, fichier restauré → exit 0. **Limite : le run GitHub Actions n'a pas été exécuté ici (aucun push autorisé) — la structure est vérifiée, pas le runner.** | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 395` après CI.1 | **documents cohérents**, exit 0 | 2026-11-04 |
| OPS.2 : compter les refus d'allocation par motif | **fait** — `RefusalCounters` vit dans le serveur (jamais `DaemonState`), incrémenté au seul point où une réponse `AllocDenied` passe ; nouvelle requête `refusals_request` (pas un champ de `Status`, qui est déjà au plafond de taille porté par `NodeError`). Test sur un VRAI serveur : les **8** motifs provoqués (`invalid_request`, `exceeds_measured`, `insufficient_headroom`, `insufficient_cpu`, `insufficient_vram`, `insufficient_storage`, `unknown_allocation`, `policy_shrinking`), chacun compté une fois. Session réelle : `alloc_request` de 100 000 Gio sur le socket → `alloc_denied` ; `polygoned status` → `Refus : exceeds_measured: 1` ; `polygoned doctor` → `exceeds_measured: 1` ; daemon arrêté → `non mesurés (daemon absent)`, jamais `0` | 2026-11-04 |
| `daemon/src/cli.rs` n'était compilé par aucune cible | **corrigé** — le module (et ses 5 tests, jamais exécutés) n'était référencé ni par `lib.rs` ni par `main.rs` ; `pub mod cli;` ajouté, `cargo test -p polygoned --lib` passe de 111 à **117 tests** | 2026-11-04 |
| `cargo test --workspace -j 2` après OPS.2 | **402 tests, 0 échec** (exit 0) | 2026-11-04 |
| `cargo clippy --workspace --all-targets -- -D warnings` après OPS.2 | **exit 0** — le champ a été retiré de `Status` précisément parce que 8 octets suffisaient à déclencher `result_large_err` | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 402` | **documents cohérents**, exit 0 | 2026-11-04 |
| SEC.8 : empêcher un secret d'entrer dans l'arbre | **fait** — `crates/client/tests/no_tracked_secrets.rs` lit ce que git **suit** (`git ls-files -z`, repli par parcours si git absent) et refuse `identity.json`, `*.key`, `.env`/`.env.*` (gabarits `.env.example|sample|template` permis). Positif **2/2** ; porte : `.env` réellement ajouté avec `git add -f` → **échec nommant `.env`**, dé-suivi puis retiré → vert. Hook `.githooks/pre-commit` (activation documentée : `git config core.hooksPath .githooks`) + étape CI | 2026-11-04 |
| `cargo test --workspace -j 2` après SEC.8 | **404 tests, 0 échec** (exit 0) | 2026-11-04 |
| `cargo clippy --workspace --all-targets -- -D warnings` après SEC.8 | **exit 0** | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 404` | **documents cohérents**, exit 0 | 2026-11-04 |

## Décisions tranchées

| Date | Sujet | Décision | Document |
|---|---|---|---|
| 2026-11-03 | Monnaie `POLY` | **Pas de cryptomonnaie.** Unité de réciprocité non transférable, dans le `Ledger` du daemon ; reçu signé dans `crates/core` ; rien dans `hope-gateway`, rien sur une chaîne. | `docs/atlas/plan/DECISION-POLY.md` |

## Bloquants connus

1. **Divergence Git non réconciliée** : 6 commits locaux, 17 commits distants
   sur `Polygone-Network` (`main`). Inventoriée commit par commit et tranchée
   dans `docs/RECONCILIATION.md` ; **aucun merge effectué**, et c'est volontaire.
   Le distant est une ligne antérieure, pas un complément : 2 éléments à
   reprendre, 15 refusés avec motif. L'installateur, d'abord classé « à
   sauver », s'est révélé déjà fail-closed localement avec cinq tests
   d'échec — correction du 2026-11-03, vérifiée dans `install.sh:157-177`.
2. **Aucun test entre deux machines physiques** : les tests prouvent des
   processus locaux, pas un réseau réel.
3. **18 capacités non câblées** : listées dans `ETAT-REEL.md` §3.
4. **macOS/Windows** : compilation possible, isolation OS non prouvée.

## Ce qui n'est pas dans ce dépôt

- `target/`, `node_modules/` : artefacts.
- `archive/2026-07-src/` : monolithe libp2p abandonné.
- Sous-produits d'outillage (`graphify-out/`, `.claude-flow/`, `.agents/`).
- Secrets, identités, bases de données, conversations.

## Prochaine action

**Priorité du backlog : `BLOCKS.9`** — prouver un transfert de blocs entre deux machines physiques. Cette preuve ne peut pas être simulée par deux processus locaux ; elle nécessite un second hôte réel.

**`CLI.1`, `CLI.2` et `CLI.5` fermés le 2026-11-04.** `doctor` lit l'isolation OS au lieu de la déclarer ; `pause --duree` rend le bail et écrit une intention bornée, `reprendre` la consomme et redemande un bail ; `desinstaller` énumère, exige `--confirmer`, ne touche que ses racines et ne laisse aucun reste.

**Prochaine tâche réalisable sur ce poste : `CLI.3`** — des profils d'usage nommés (`--profil eco|equilibre|performance`) qui écrivent une configuration réellement relue, plutôt qu'un palier recopié dans trois fichiers. Ensuite : `DOC.4` (refuser un document sans date ni statut), `SEC.1` (modèle de menace par service), `GW.1` (heartbeat persisté) — puis `CI.3` (audit de dépendances, dépend de la licence) et `BLOCKS.9`/`MSG.6`, qui **exigent deux machines physiques** : non réalisables ici, scripts à préparer. **Limite SEC.8** : `gitleaks` n'est pas installé sur ce poste ; l'audit d'historique annoncé par la spec n'a pas été exécuté, seul le refus des chemins suivis est prouvé.

**L'autorité d'ordre est `docs/atlas/agent/backlog.json`** : 11 tâches restantes (8 ouvertes et 3 bloquées : DAEMON.10 par D2, DRIVE.1 par D3, RECIP.1 par BLOCKS.9 et D8) ; 27 tâches fermées avec leur preuve dans `completed`. DOC.2, DOC.3, CLI.1, CLI.2, CLI.5, CI.1, OPS.2 et SEC.8 sont fermés depuis le 4 novembre 2026. `DOC.3` n'était pas promue dans les tâches opérationnelles : sa preuve est ajoutée directement à `completed`. Les vagues W0…W5 de `docs/atlas/plan/PLAN-ECOSYSTEME.md` §8 placent le gateway, l'agent et le corpus sur le même calendrier.

> **Correction du 2026-11-03** : la note précédente demandait de committer les
> 65 fichiers non suivis du dépôt canonique. **C'était inutile.** Vérifié par
> `cmp` sur 60 fichiers et `diff -rq` sur 3 dossiers : ils sont **déjà
> byte-identiques** ici, donc déjà commités sur `realign/v1` et poussés. Le
> dépôt canonique reste volontairement fragile : c'est le poste de travail,
> pas la copie de référence.

Voir `ROADMAP.md`, `docs/P0-RAPPORT.md` et `docs/atlas/agent/backlog.json`.


## Synchronisation documentaire Atlas

| Contrôle | Résultat à vérifier | Date |
|---|---|---|
| node scripts/check-atlas.mjs | 6 épiques, 132 identifiants, backlog cohérent, aucun chemin privé | 2026-11-04 |
| node --test docs/atlas/agent/tests/backlog.test.mjs | tests du véritable orchestrateur, worker absent et tâches déjà terminées | 2026-11-04 |
