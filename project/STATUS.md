# STATUS — état de mission, écrit pour être repris sans réexploration

> Point d'entrée d'une reprise. Il ne remplace pas [`../STATE.md`](../STATE.md)
> (journal des vérifications) : il dit **où en est la mission, quelle est la
> prochaine action, et comment la prouver.**
> Règle : aucune ligne `FAIT` sans commande et sortie associées.
> Dernière mise à jour : 2026-11-09 (session 2).

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
| Réservation de stockage dans le ledger | À FAIRE | `daemon/src/socket.rs` (version réelle : `grants` à 3 champs) |
| Publication / push | **INTERDIT** — voir §5 | — |

## 3. Prochaine action, dans l'ordre

1. Porter la réservation de stockage du ledger : `grants: HashMap<String,(u32,u64,u64)>`
   dans `daemon/src/socket.rs`, en partant du fichier de `~/Projects/Polygone-real`
   (655 lignes uniques côté canonique : fusion manuelle, pas une copie).
2. Ajouter le test de migration `peers.json` au `Makefile` (`make migrate-proof`).
3. Porter les tests de mesure de stockage de `daemon/src/runtime.rs` (38 lignes uniques côté réel).
4. Relancer `make check` et `make smoke`, puis consigner dans `STATE.md`.
5. Seulement ensuite : envisager la CI GitHub Actions et la procédure deux machines.

## 4. Commandes utiles

```bash
cd ~/Polygone-canonical
./scripts/mission-resume.sh          # état + prochaine action, sans réexploration
make check                           # fmt + clippy + tests + docs
make smoke                           # parcours réel 3 processus
./scripts/peers-migration-smoke.sh   # migration v1 → v2 sur un fichier réel
./scripts/cross-version-smoke.sh     # client d'octobre vs relais courant
~/bin/gitleaks git . --no-banner --redact
```

## 5. Ce qui interdit une publication

1. **Réservation de stockage** non portée dans le ledger : la capacité mesurée est
   publiée, mais rien ne réserve le disque alloué.
2. **Deux machines physiques** jamais testées.
3. **CI GitHub Actions** jamais exécutée sur cet arbre.
4. **Aucune autorisation explicite de publication** n'a été donnée.
5. Le scan de secrets doit être **rejoué** sur l'arbre final, immédiatement avant
   le push (celui du jour couvre les commits `9ff9360` et antérieurs).

## 6. Limites de l'environnement

- `freebuff` en ligne de commande n'expose **aucun mode non interactif** : une
  session ne peut pas se relancer elle-même. La continuité repose sur ce fichier,
  `STATE.md`, `docs/atlas/agent/backlog.json` et les unités systemd déjà en place
  (`polygone-orchestrator.service`, `polygon-loop.service`).
- `gitleaks` non fourni par la distribution : binaire officiel 8.30.1 installé dans
  `~/bin/gitleaks`.
- Disque à ~92 % : les builds supplémentaires coûtent cher, `make clean` est à
  éviter sans sauvegarde.
