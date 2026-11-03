# STATE — état courant

> Mis à jour par l'orchestrateur après chaque bloc de travail.
> Ne jamais écrire ici une valeur non vérifiée.

## Dernière session

| Champ | Valeur |
|---|---|
| Date | 2026-11-03 |
| Branche | `realign/v1` |
| HEAD | voir `git log -1 --format='%h %s'` (jamais recopié à la main) |
| Arbre | propre |
| Origine | `lvs0/Polygone-Network`, branche `realign/v1` |

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

**`MSG.1` — rendre la durée de rétention vraie.** Trois documents publiés
(`LEGAL.md:33`, `THREAT_MODEL.md:19`, `docs/threat-commodity.md:38`) affirment un
TTL de 30 s sur le relais. Le relais ne conserve rien du tout, et la session
incomplète côté récepteur vit **300 s**. C'est une affirmation fausse dans un
document juridique : elle passe donc avant `DOC.2` (inventaire des documents),
qui est la tête de file du backlog mais ne corrige aucun mensonge.

Ensuite, dans l'ordre du backlog : `DAEMON.13` (crash, veille, SIGKILL) puis
`BLOCKS.9`, qui reste la preuve la plus ancienne qui manque au projet.

**L'autorité d'ordre est `../04-AGENT/backlog.json`** : 23 tâches restantes,
3 bloquées (D2, D3, D8 + `BLOCKS.9`), 7 fermées avec leur preuve dans
`completed`. Les vagues W0…W5 de `../02-PLAN/PLAN-ECOSYSTEME.md` §8 placent le
gateway, l'agent et le corpus sur le même calendrier.

> **Correction du 2026-11-03** : la note précédente demandait de committer les
> 65 fichiers non suivis du dépôt canonique. **C'était inutile.** Vérifié par
> `cmp` sur 60 fichiers et `diff -rq` sur 3 dossiers : ils sont **déjà
> byte-identiques** ici, donc déjà commités sur `realign/v1` et poussés. Le
> dépôt canonique reste volontairement fragile : c'est le poste de travail,
> pas la copie de référence.

Voir `ROADMAP.md`, `docs/P0-RAPPORT.md` et `../04-AGENT/backlog.json`.
