# PLAN_REPRISE — état extrait et plan, 2026-10-03

> Reprise demandée : « extraire pleins de datas pour comprendre et former un plan ».
> Ce document ne fait que consigner ce qui a été **lu et exécuté** ce jour.
> Aucun code modifié. Preuves : commandes et sorties brutes en marge.

---

## 0 · Où est le vrai projet (trois dossiers portent le nom)

| Dossier | Nature | Verdict |
|---|---|---|
| `~/Projects/Polygone-real` | **125 commits**, 6 crates, AGPL-3.0, v2.0.0, suite de tests vivante | ✅ **canonique** |
| `~/Projects/Polygone-Network` | 6 crates, **0 commit**, README aux chiffres invérifiables (débit 2 400 msg/s, « 45 tests ») | 🔴 dépôt public, code divergent |
| `~/Projects/Polygone-v2` | 6 crates, **0 commit**, README MIT, « polygone.toml » jamais implémenté | ⚪ coquille quasi vide |

`Polygone-real` pousse (`origin`) vers `github.com/lvs0/Polygone-Network` — le nom du dépôt distant
et le dossier local ne correspondent donc pas. Toute reprise se fait dans **`Polygone-real`**.

## 1 · État réel du dépôt canonique (exécuté)

| Métrique | Réalité mesurée | Commande |
|---|---|---|
| Cargo check | **RC=0**, 10 warnings (tous dans `petals`, variables inutilisées) | `cargo check --workspace` |
| Tests | **215 verts, 0 échec** (client 98, core 53, relay 29, core-int 27, + petals/daemon/gateway) | `cargo test --workspace` |
| LOC cœur | core 1 373 · relay 623 · client 9 757 · daemon 8 662 | `wc -l` |
| Travail non commité | **218 fichiers, +7 801 / −666**, du **29/09 au 02/10** | `git status`, `git diff --numstat` |
| Synchronisation | `git rev-list origin/main...main` → **0 0** ; les 218 fichiers sont **purement locaux** | `git rev-list` |
| Licence | AGPL-3.0-or-later, auteur Lévy Verpoort Scherpereel | `Cargo.toml` |

**Risque immédiat :** tout le travail du 29/09→02/10 existe **uniquement dans ce dossier**. Un `git clean`,
un `git checkout` ou une perte de disque l'efface. C'est le premier point à traiter (§4, P0).

## 2 · L'axe de travail en cours (29/09 → 02/10)

Fil directeur unique, tiré des `docs/PLAN_*.md` et du dernier `ARCHITECTURE.md` :

> Le daemon `polygoned` mesurait, décidait et publiait des allocations de ressources
> — **et personne ne consommait.** Tout le travail récent consiste à lui donner un
> consommateur réel, avec une preuve à chaque étape.

| Incrément | Livré (non commité) | Fichier |
|---|---|---|
| 2b-1→8 | Protocole socket v1 complet, JSON par ligne, versionné (`{"v":1,…}`) | `daemon/src/socket.rs`, `daemon/src/cli.rs` |
| 2b-9→10 | Client de référence + CLI | `daemon/src/client.rs` |
| 2b-12b | **Magasin de blocs adressé par hash** — 4 étapes falsifiables prouvées (stocke → sert → refus si serveur tué → refus si bloc altéré) | `crates/client/src/blocks.rs` (648 l.) |
| 2b-12c | Test **deux machines réelles** — **JAMAIS EXÉCUTÉ** (pas de second nœud) | — |
| 2b-13 | `polygone node servir` : **premier consommateur de production** — demande un *bail*, s'y limite, s'arrête à la fin du bail | `crates/client/src/node.rs` (1 293 l.) |
| Chantier II | **Cache de calcul** — clé `BLAKE3(version ‖ entrée)` → bloc content-addressed | `crates/client/src/calc.rs` (702 l.) |

**Nouvelle primitive unificatrice** (thèse de `docs/ARCHITECTURE_RESEAU.md`) : *un bloc adressé par son
hash, placé chez le pair le plus proche, compté par le ledger, attesté.* C'est à la fois le cache de
calcul, le stockage, le relais et le ledger — cinq idées, une seule brique.

## 3 · Ce qui bloque (nommé, pas contourné)

1. **Aucun second nœud physique** → `2b-12c` (preuve LAN) n'a jamais tourné. Le projet **admet** que
   « deux processus » ≠ « deux machines » et refuse de présenter la variante locale comme la preuve.
2. **Ledger volatil** — `socket.rs:458` : `grants` est un `HashMap` en mémoire ; dès qu'on y met du
   stockage durable, un redémarrage re-accorde des octets déjà occupés.
3. **Index de cache non vérifiable** — le hash prouve l'**intégrité**, pas la **justesse** : un pair peut
   servir un résultat faux à hash valide. Assumé et écrit noir sur blanc.
4. **Aucun audit externe** — posture « honesty-first », rien n'est certifié.
5. **Docs contradictoires** — `VISION-COMPLETE.md` promet `docs/site/` (inexistant), un service worker
   (iOS n'a pas de background sync), `/api/v1/network/status` (la passerelle renvoie `latency_ms_median: 42`
   **en dur**, `crates/gateway/src/lib.rs:107`).

## 4 · Plan proposé

**P0 — protéger le travail, puis figer l'axe (aujourd'hui)**
1. Commit des 218 fichiers non commités, en un ou deux commits thématiques (« 2b-x: daemon consumer +
   block store + calc cache »). Rien n'est poussé sans confirmation.
2. Nettoyer les 10 warnings `petals` (variables inutilisées) — `cargo fix --lib -p polygone-petals`.

**P1 — lever le seul blocage nommé (cette semaine)**
3. Exécuter `2b-12c` sur **deux machines réelles** du LAN (le ThinkPad X250 + un second hôte, ou deux
   VM réseau distinctes — mais alors le dire comme « VM », pas « machines »). Critère de sortie :
   les 4 étapes falsifiables de `ARCHITECTURE_RESEAU.md` §7, cette fois sur le réseau.

**P2 — une fois un bloc réellement servi entre deux nœuds**
4. Étendre le ledger au **stockage persistant** (blocage §3.2) — c'est la seule extension que la
   primitive exige.
5. Brancher `receipt.rs` (`hash_payload` existe, **zéro appelant**) pour attester un bloc servi.

**P3 — hygiène produit**
6. Réconcilier `README.md` avec la réalité mesurée (le README de `Polygone-real` annonce « 113 tests »
   et une installation depuis `Polygone-Network`) ; décider du sort de `Polygone-Network` et `Polygone-v2`
   (fusion, archive, ou suppression).

**Ne pas commencer maintenant** (raison du dépôt, reprise telle quelle) : streaming de jeu, VRAM
partagée, agents IA distribués, marketplace — « pas avant qu'un bloc soit réellement servi par un
autre nœud ».

## 5 · Prochaine action unique

> **Décider du sort des 218 fichiers non commités.**

C'est le seul point où une erreur détruit du travail. Commande de contrôle, sans effet :

```bash
cd ~/Projects/Polygone-real && git status --porcelain | head && git diff --numstat | awk '{a+=$1;d+=$2} END{print "lignes +"a" -"d}'
```
