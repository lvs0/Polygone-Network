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
| Plan de finition : épiques et backlog | **Fait** — 132 tâches en 15 épiques (`../02-PLAN/epiques/`), 29 tâches écosystème (`../02-PLAN/PLAN-ECOSYSTEME.md`), 30 tâches dans `../04-AGENT/backlog.json` | 2026-11-03 |
| Cohérence backlog ↔ épiques (script Node sur les 2 fichiers) | **PASS** — JSON valide, 30 tâches, 132 épiques, 0 doublon, 0 identifiant absent (sauf `RECIP.1`, qui vit dans `DECISION-POLY.md`), 3 bloquées déclarées | 2026-11-03 |
| DAEMON.1/DAEMON.3 : expiration et renouvellement des baux | **fait** — 5 tests ajoutés (échéance, adoption d'un bail hérité, relecture du format historique, renouvellement, refus d'un bail inconnu/échu) · `315f0eb` | 2026-11-03 |
| DOC.1 : contrôle de cohérence documentaire | **fait** — `node scripts/check-docs.mjs --tests 361` → « documents cohérents », **exit 0** ; chiffre cassé exprès → **exit 1** et l'écart nommé · `288d67e` | 2026-11-03 |
| UI.1/UI.2/UI.5 : plus de chiffres inventés, plus de fausse promesse | **fait** — `Math.random` → 0 dans `web/dashboard.js`, `node --check` OK, libp2p/Five tabs/Sparklines retirés des deux pages · `f2e0a59` | 2026-11-03 |
| `cargo test --workspace` après ce lot | **361 tests, 18 suites, 0 échec** (exit 0) | 2026-11-03 |
| `cargo clippy --workspace --all-targets -- -D warnings` | **exit 0** | 2026-11-03 |
| `cargo fmt --all -- --check` | **OK** | 2026-11-03 |
| Orchestrateur : tâches bloquées (AGT.1/AGT.2) | **fait** — `node --test 04-AGENT/tests/backlog.test.mjs` → **5/5**, dont « une tâche bloquée est refusée en nommant son bloqueur » (exit 3) | 2026-11-03 |
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
| `node --test ../04-AGENT/tests/backlog.test.mjs` | **5/5**, exit 0 — 19 tâches ouvertes, 15 fermées, JSON valide | 2026-11-04 |
| SEC.3 : rejeu capturé sur le fil, refusé par le **vrai** relais | **fait** — `replay_through_relay_is_refused_with_an_explicit_code` : session déchiffrée (témoin positif), rejeu immédiat → `REPLAY.SESSION_ALREADY_COMPLETED`, rejeu à +10 min contre un récepteur frais → `REPLAY.STALE_TIMESTAMP`. Le refus n'est plus un `None` muet : `Refusal::code()` le nomme | 2026-11-04 |
| `cargo test --workspace -j 2` après SEC.3 | **371 tests, 0 échec** (exit 0) | 2026-11-04 |
| `cargo clippy --workspace --all-targets -- -D warnings` | **exit 0** | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 371` | **documents cohérents**, exit 0 | 2026-11-04 |

## Décisions tranchées

| Date | Sujet | Décision | Document |
|---|---|---|---|
| 2026-11-03 | Monnaie `POLY` | **Pas de cryptomonnaie.** Unité de réciprocité non transférable, dans le `Ledger` du daemon ; reçu signé dans `crates/core` ; rien dans `hope-gateway`, rien sur une chaîne. | `../02-PLAN/DECISION-POLY.md` |

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

**`CLI.1` — `polygone doctor`.** Une commande qui répond aux trois questions
réelles : ce que la machine **mesure**, ce qu'elle peut **réserver**, ce qu'elle
**subit** (politique, isolation OS, bail du daemon) — et qui dit « non vérifié »
plutôt que d'inventer un chiffre quand la mesure échoue.

Ensuite : `CLI.2`, `CLI.5`, `DOC.2` (inventaire daté des documents), `CI.1`
(matrice Windows honnête), `SEC.8` (secrets), `OPS.2` (compteurs de refus) — puis
`BLOCKS.9`, la preuve la plus ancienne qui manque au projet et qui **exige deux
machines physiques** : non réalisable ici, scripts à préparer.

**L'autorité d'ordre est `../04-AGENT/backlog.json`** : 18 tâches restantes,
4 bloquées (D2, D3, D8, `BLOCKS.9` + `RECIP.1`), 16 fermées avec leur preuve
dans `completed`. Les vagues W0…W5 de `../02-PLAN/PLAN-ECOSYSTEME.md` §8 placent le
gateway, l'agent et le corpus sur le même calendrier.

> **Correction du 2026-11-03** : la note précédente demandait de committer les
> 65 fichiers non suivis du dépôt canonique. **C'était inutile.** Vérifié par
> `cmp` sur 60 fichiers et `diff -rq` sur 3 dossiers : ils sont **déjà
> byte-identiques** ici, donc déjà commités sur `realign/v1` et poussés. Le
> dépôt canonique reste volontairement fragile : c'est le poste de travail,
> pas la copie de référence.

Voir `ROADMAP.md`, `docs/P0-RAPPORT.md` et `../04-AGENT/backlog.json`.
