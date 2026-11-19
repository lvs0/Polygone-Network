# ADR-0000 — Base canonique du produit Polygone

- **Statut** : ACCEPTÉE
- **Date** : 2026-11-09
- **Remplace la décision ouverte `D1`** de `POLYGONE_UNIFICATION_PLAN_2026-11-09.md`
- **Autorité** : mission fondatrice du 2026-11-09, qui désigne `Polygone Real`
  comme base de code de référence après comparaison technique.

## Contexte

Deux lignées Git portent le même produit et **n'ont aucun ancêtre commun**
(`git merge-base` vide entre `main` de `Polygone-real` et `realign/v2`) :

| Lignée | Tête | Date | Fichiers | État mesuré |
|---|---|---|---|---|
| `Polygone-real/main` | `f3d09c0` | 2026-10-29 | 269 | 6 commits locaux, 17 commits de retard sur `origin/main`, 78 modifications non commitées |
| `realign/v2` | `967afdb` | 2026-11-07 | 321 | arbre propre, 294 tests verts mesurés le 2026-11-09 |
| `p0/first-secure-network` | `d6e7229` | 2026-11-09 | +22 fichiers | correctif de sécurité du relais, non intégré |

Aucun des deux ensembles n'est un sur-ensemble de l'autre : 113 fichiers
n'existent que dans `realign/v2`, 61 que dans `main`, dont `archive/2026-07-src/`.

## Décision

**Option D1-b.** `main` de `Polygone-real` est la base canonique ; le contenu de
`realign/v2` est rejoué dessus par une fusion à histoires non liées
(`--allow-unrelated-histories`), puis la branche `p0/first-secure-network` est
intégrée par-dessus.

Motifs :

1. C'est la lignée que la mission du fondateur désigne comme référence.
2. C'est la seule qui contienne l'archive historique (`archive/2026-07-src/`,
   44 fichiers) et les 6 commits locaux (ledger démon, refactor `UserActivity`,
   déblocage workspace, retrait des modules morts `drive`/`brain`).
3. Le contenu plus récent vient malgré tout de `realign/v2`, donc le résultat
   combine l'ancienneté de l'histoire et la fraîcheur du code.

Les deux histoires sont conservées intactes : rien n'est réécrit, aucun
`force-push`, `main` et la branche d'archive restent consultables.

## Conséquences

- Le dépôt canonique de travail est `~/Polygone-canonical`, branche `reconcile/2026-11`.
- `realign/v2` reste accessible comme branche d'archive sous `realign/realign/v2`.
- Les 78 modifications non commitées de `Polygone-real` ne sont **pas** dans la
  fusion (elles n'ont jamais été commitées) ; elles sont préservées dans
  `~/Ops/backups/polygone-unification-2026-11-09/dirty/` et doivent être
  réévaluées une par une contre l'arbre fusionné avant toute réintégration.
- Toute publication (`origin/main`) reste conditionnée aux critères de l'étape 8
  du plan : tests verts comptés, gitleaks, contrôle de non-perte.

## Réversibilité

```bash
cd ~/Polygone-canonical && git switch main   # état d'avant fusion, intact
```

## Annexe — arbitrage des 65 conflits `add/add`

Règle appliquée : version `realign/v2` retenue pour chacun des fichiers
ci-dessous, car postérieure de 9 jours et couverte par les 294 tests verts
mesurés le 2026-11-09. Aucun `-X ours`/`-X theirs` global.

| Fichier | Version retenue | Motif |
|---|---|---|
| `.github/workflows/ci.yml` | realign/v2 | plus récente, testée |
| `.github/workflows/release.yml` | realign/v2 | plus récente, testée |
| `.gitignore` | realign/v2 | plus récente, testée |
| `ARCHITECTURE.md` | realign/v2 | plus récente, testée |
| `CHANGELOG.md` | realign/v2 | plus récente, testée |
| `Cargo.lock` | realign/v2 | plus récente, testée |
| `Cargo.toml` | realign/v2 | plus récente, testée |
| `LEGAL.md` | realign/v2 | plus récente, testée |
| `README.md` | realign/v2 | plus récente, testée |
| `STAGING.md` | realign/v2 | plus récente, testée |
| `THREAT_MODEL.md` | realign/v2 | plus récente, testée |
| `crates/client/Cargo.toml` | realign/v2 | plus récente, testée |
| `crates/client/src/exec.rs` | realign/v2 | plus récente, testée |
| `crates/client/src/hide.rs` | realign/v2 | plus récente, testée |
| `crates/client/src/identity.rs` | realign/v2 | plus récente, testée |
| `crates/client/src/lib.rs` | realign/v2 | plus récente, testée |
| `crates/client/src/main.rs` | realign/v2 | plus récente, testée |
| `crates/client/src/mesh.rs` | realign/v2 | plus récente, testée |
| `crates/client/src/net/mod.rs` | realign/v2 | plus récente, testée |
| `crates/client/src/net/tests.rs` | realign/v2 | plus récente, testée |
| `crates/client/src/node.rs` | realign/v2 | plus récente, testée |
| `crates/client/src/product.rs` | realign/v2 | plus récente, testée |
| `crates/client/src/reputation.rs` | realign/v2 | plus récente, testée |
| `crates/client/src/serverless.rs` | realign/v2 | plus récente, testée |
| `crates/client/src/tui.rs` | realign/v2 | plus récente, testée |
| `crates/gateway/src/lib.rs` | realign/v2 | plus récente, testée |
| `crates/gateway/src/main.rs` | realign/v2 | plus récente, testée |
| `crates/petals/src/lib.rs` | realign/v2 | plus récente, testée |
| `crates/petals/tests/protocol_contract.rs` | realign/v2 | plus récente, testée |
| `crates/relay/src/main.rs` | realign/v2 | plus récente, testée |
| `crates/relay/src/relay.rs` | realign/v2 | plus récente, testée |
| `daemon/src/cli.rs` | realign/v2 | plus récente, testée |
| `daemon/src/client.rs` | realign/v2 | plus récente, testée |
| `daemon/src/lib.rs` | realign/v2 | plus récente, testée |
| `daemon/src/main.rs` | realign/v2 | plus récente, testée |
| `daemon/src/policy/glow_up.rs` | realign/v2 | plus récente, testée |
| `daemon/src/runtime.rs` | realign/v2 | plus récente, testée |
| `daemon/src/socket.rs` | realign/v2 | plus récente, testée |
| `docs/ARCHITECTURE_RESEAU.md` | realign/v2 | plus récente, testée |
| `docs/CORE-COMPLETE.md` | realign/v2 | plus récente, testée |
| `docs/DAEMON_STATE.md` | realign/v2 | plus récente, testée |
| `docs/PLAN_CONSOMMATEUR.md` | realign/v2 | plus récente, testée |
| `docs/PLAN_CONSOMMATEUR_MAJ.md` | realign/v2 | plus récente, testée |
| `docs/PLAN_PRIORITES_2B5.md` | realign/v2 | plus récente, testée |
| `docs/PLAN_REPRISE-2026-10-03.md` | realign/v2 | plus récente, testée |
| `docs/RAPPORT-FINAL-COMPLET-2026-08-12.md` | realign/v2 | plus récente, testée |
| `docs/RAPPORT-IDEES-MINEES-2026-08-12.md` | realign/v2 | plus récente, testée |
| `docs/RAPPORT-MISSION-2026-08-12.md` | realign/v2 | plus récente, testée |
| `docs/SESSION-REPORT-2026-08-06.md` | realign/v2 | plus récente, testée |
| `docs/SESSION-REPORT-2026-08-08.md` | realign/v2 | plus récente, testée |
| `docs/STRATEGIE.md` | realign/v2 | plus récente, testée |
| `docs/VISION-COMPLETE.md` | realign/v2 | plus récente, testée |
| `docs/VISION_CANONIQUE.md` | realign/v2 | plus récente, testée |
| `docs/cli.md` | realign/v2 | plus récente, testée |
| `docs/config.md` | realign/v2 | plus récente, testée |
| `docs/kill-switch.md` | realign/v2 | plus récente, testée |
| `docs/threat-commodity.md` | realign/v2 | plus récente, testée |
| `install.sh` | realign/v2 | plus récente, testée |
| `scripts/hide-smoke.sh` | realign/v2 | plus récente, testée |
| `scripts/install.sh` | realign/v2 | plus récente, testée |
| `web/dashboard.css` | realign/v2 | plus récente, testée |
| `web/dashboard.js` | realign/v2 | plus récente, testée |
| `web/index.html` | realign/v2 | plus récente, testée |
| `web/landing.html` | realign/v2 | plus récente, testée |
| `web/node.html` | realign/v2 | plus récente, testée |
