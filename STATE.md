# STATE — état courant

> Mis à jour par l'orchestrateur après chaque bloc de travail.
> Ne jamais écrire ici une valeur non vérifiée.

## Dernière session

| Champ | Valeur |
|---|---|
| Date | 2026-11-02 |
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

## Bloquants connus

1. **Divergence Git non réconciliée** : 6 commits locaux, 17 commits distants
   sur `Polygone-Network` (`main`). Aucun merge effectué. Voir `SALVAGE.md` §7.
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

`P0.1` — archive restaurable du dépôt canonique. Voir `ROADMAP.md` et
`../04-AGENT/backlog.json`.
