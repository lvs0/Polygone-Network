# STATUS — état de mission, écrit pour être repris sans réexploration

> Point d'entrée d'une reprise. Il ne remplace pas [`../STATE.md`](../STATE.md)
> (journal des vérifications) : il dit **où en est la mission, quelle est la
> prochaine action, et comment la prouver.**
> Règle : aucune ligne `FAIT` sans commande et sortie associées.
> Dernière mise à jour : 2026-11-13 (plafond CPU et politique de confiance, session 6).

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
| Persistance du ledger atomique et privée | FAIT — temporaire + `sync_all` + `rename`, `0600`, temporaire nettoyé | `test_ledger_persist_is_atomic_and_private` ; `docs/testing/VALIDATION-RESULTS.md` |
| Ledger illisible conservé en quarantaine | FAIT — `<nom>.corrupt-<horodatage>-<pid>`, journal d'erreur, redémarrage sur ledger vierge | `test_corrupt_ledger_is_quarantined_not_silently_erased` |
| Test de heartbeat déterministe | FAIT — comparé à la valeur renvoyée par la gateway, plus au timestamp client | `crates/gateway/tests/serve.rs` ; 499 tests workspace |
| Documentation de fondation (13 livrables) | FAIT — audit, roadmap, statut, architecture, menace, tests, produit | `project/FOUNDATION-AUDIT.md`, `project/FOUNDATION-ROADMAP.md`, `docs/architecture/`, `docs/security/THREAT-MODEL.md`, `docs/testing/`, `product/` |
| Modèle de tâche | FAIT — états explicites, transitions vérifiées, résultat BLAKE3, producteur imposé, bornes | `crates/core/src/task.rs` ; 15 tests ; `task::tests::lifecycle_reaches_success_in_order` |
| File et API de tâches | FAIT — créer, lire, annuler, expirer ; persistance atomique `0600` et quarantaine | `crates/gateway/src/tasks.rs`, `routes.rs` ; 10 tests ; `docs/architecture/API-CONTRACTS.md` §2.1 |
| Exécution d'une tâche | **NON FAIT** — aucun exécuteur ; `executor_attached: false` | — |
| Publication / push | **INTERDIT** — voir §5 | — |

## 3. Prochaine action, dans l'ordre

1. Traiter `F1-PROTO-01` : versionner et négocier les capacités du socket, en
   gardant `v: 1` lisible. `F1-RES-03` est tranché : le pool est plafonné par
   `max_cpu_percent` et le contrat est écrit dans `docs/architecture/API-CONTRACTS.md`.
2. Exécuter la CI GitHub Actions sur ce HEAD (pas encore vérifiée).
3. Organiser un test entre deux machines physiques distinctes ; les tests locaux
   actuels utilisent plusieurs processus sur une seule machine.
4. Rejouer Gitleaks sur l'arbre final immédiatement avant toute publication.
5. Terminer la revue des fichiers `a_examiner` de Polygone-real et corriger le
   décompte de `project/DIRTY-REVIEW.md` (`F1-DOC-02` / `F2-DOC-02`).
6. Ne rien pousser sans autorisation explicite.

La file de tâches (`F1-TASK-01`) est livrée ; la suite est le branchement d'un
exécuteur (`F1-TASK-02`), puis la vérification de bout en bout (`F1-TASK-03`).

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
