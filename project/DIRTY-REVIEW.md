# Revue des 78 modifications non commitées de `Polygone-real`

> Source : `~/Ops/backups/polygone-unification-2026-11-09/dirty/Projects_Polygone-real.status.txt`
> Classification mesurée : [`../docs/proofs/dirty-2026-11-09.tsv`](../docs/proofs/dirty-2026-11-09.tsv)
> Date : 2026-11-09. Ces modifications n'ont **jamais** été commitées : la fusion
> Git ne pouvait pas les voir.

## 1. Méthode

Pour chaque entrée, comparaison du fichier de travail de `Polygone-real` avec le
fichier correspondant de l'arbre canonique, puis comptage des lignes uniques de
chaque côté (`comm -23` / `comm -13` sur les lignes triées). Trois verdicts :

| Verdict | Signification | Nombre |
|---|---|---:|
| `identique` | même contenu des deux côtés | 23 |
| `remplace_par_le_canonique` | aucune ligne du fichier réel n'est absente du canonique → le canonique est un sur-ensemble | 14 |
| `a_examiner` | le fichier réel contient des lignes absentes du canonique | 31 |
| `dossier_non_versionne` | répertoire non suivi (`checklists/`, `.specify/`, …) | 5 |
| `absent_du_canonique` | fichier qui n'existe pas dans l'arbre canonique | 5 |

`a_examiner` ne veut pas dire « précieux » : une version ancienne contient aussi
des lignes que la version récente a **supprimées volontairement**. Le tri a donc
été fait sur le critère inverse, celui qui ne trompe pas — le canonique est-il un
sous-ensemble strict du fichier réel (aucune ligne canonique absente) ?

## 2. Ce qui était réellement perdu, et a été réintégré

Quatre fichiers étaient dans ce cas, donc la réintégration ne pouvait pas
écraser de contenu canonique :

| Fichier | Lignes uniques côté réel | Contenu perdu | Décision |
|---|---:|---|---|
| `daemon/src/resources/mod.rs` | 80 | `StorageInfo` (total/disponible), `mount_for_path` (montage par plus long préfixe), `Platform::storage_info` fail-closed | **réintégré** (copie intégrale) |
| `daemon/src/system.rs` | 22 | `SystemSnapshot::storage`, mesure au `capture()` | **réintégré** (copie intégrale) |
| `daemon/src/resources/linux.rs` | 15 | `storage_info` via `sysinfo::Disks` sur le montage du répertoire de données | **réintégré** (bloc) |
| `daemon/src/resources/macos.rs` | 15 | idem, même sémantique | **réintégré** (bloc) |
| `daemon/src/policy/glow_up.rs` | 1 | ligne de commentaire | sans effet |

Conséquence fonctionnelle : `total_storage_bytes` n'est plus une constante `0`
dans `runtime.rs`, mais l'espace réellement disponible mesuré. Une mesure
manquante reste `0`, donc toute demande de stockage est refusée — le
comportement fail-closed est conservé, seule la valeur publiée change.

Ce n'est pas une découverte de confort : l'audit `ETAT-REEL.md` classe la
capacité de stockage parmi les promesses, et publier `0` en dur revenait à
refuser une capacité que la machine possède réellement.

## 3. Ce qui reste à examiner (31 fichiers)

Les plus gros écarts de lignes uniques côté réel :

| Fichier | Uniques réel | Uniques canonique | Lecture |
|---|---:|---:|---|
| `crates/relay/src/relay.rs` | 206 | 226 | canonique plus récent : handshake `AUTH_INIT/CHALLENGE/AUTH_RESP`, tests d'usurpation. Les lignes réelles sont l'ancien `HELLO` + `AUTH_DENIED`. **Canonique retenu.** |
| `crates/client/src/net/mod.rs` | 71 | 287 | idem : le canonique porte le handshake authentifié. **Canonique retenu.** |
| `crates/client/src/mesh.rs` | 38 | 261 | le canonique porte `select_interfaces` (MESH.1) ; le réel diffuse depuis `0.0.0.0`. **Canonique retenu** (le réel est moins sûr). |
| `daemon/src/socket.rs` | 33 | 655 | le réel réserve du stockage dans le ledger (`grants: HashMap<String,(u32,u64,u64)>`). **À porter**, non fait dans cette session : cela change le protocole du socket. |
| `daemon/src/runtime.rs` | 38 | 15 | tests de mesure de stockage, réintégrés en partie via `snap.storage`. **À revoir** après le port du ledger. |
| `install.sh` (124 hunks) | 6 | 92 | canonique plus récent (fail-closed SHA256SUMS). **Canonique retenu.** |

Le détail ligne à ligne reste dans le TSV ; aucun de ces fichiers n'a été
écrasé.

## 4. Ce qui n'est pas dans le dépôt et ne doit pas y entrer

- `.freebuff-tasks/`, `.specify/`, `claude-flow.config.json`, `checklists/` :
  outillage d'agent, pas du produit.
- `AUDIT_VERACITE_VISION.md`, `FEATURE_SPEC`, `IMPL_PLAN` : identiques au
  canonique ou documents de travail déjà indexés.

## 5. Conclusion honnête

Sur 78 entrées : 23 identiques, 14 déjà couvertes par le canonique, 4 fichiers
porteurs d'une fonctionnalité réelle réintégrés, 31 restant à statuer
individuellement (dont un — la réservation de stockage dans le ledger — qui est
un vrai travail, pas une formalité).
