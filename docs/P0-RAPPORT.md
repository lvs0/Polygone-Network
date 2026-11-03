# P0 — preuves de protection (P0.1 archive, P0.2 inventaire)

> Copie du rapport produit le 2026-11-03 par `_work/p0-1-archive.sh`.
> L'archive elle-même vit hors dépôt : `/home/lvs0/Backups/`.

# P0.1 — Archive restaurable · rapport

| Champ | Valeur |
|---|---|
| Date | 20261103-191904 |
| Source | `/home/lvs0/Projects/Polygone-real` |
| Archive | `/home/lvs0/Backups/polygone-real-20261103-191904.tar.gz` |
| Taille | 85M |
| SHA-256 | `3bcccf9eda323f9134680b8803a19c835dffa93aceedaaae2f9ae6ada0ec5621` |
| Entrées listées | 6204 |
| tar exit | 0 |
| tar stderr | aucun |
| extraction exit | 0 |
| HEAD source | `f3d09c0109ff0298db7f41fef93ff5ab704a77e0` |
| HEAD extrait | `f3d09c0109ff0298db7f41fef93ff5ab704a77e0` |
| Fichiers modifiés/non suivis (source) | 65 |
| Fichiers modifiés/non suivis (extrait) | 65 |

## cmp sur 3 fichiers critiques

| Fichier | Résultat |
|---|---|
| `Cargo.toml` | identique |
| `crates/core/src/lib.rs` | identique |
| `daemon/src/socket.rs` | identique |

## Exclusions assumées

`target/` (11 Go, régénérable par `cargo`) et tout `node_modules/`
(régénérable par npm). Aucune autre exclusion : `.git`, les fichiers suivis
et les fichiers non suivis sont dans l'archive.

## Restauration

```bash
sha256sum -c /home/lvs0/Backups/polygone-real-20261103-191904.tar.gz.sha256
tar -xzf /home/lvs0/Backups/polygone-real-20261103-191904.tar.gz -C /chemin/de/restauration
```

## Verdict

**PASS**

---

## Incident de la première passe — à ne pas répéter

La première archive (`20261103-191443`) a été produite avec `tar exit=1` et le
script affichait quand même **PASS** : sa sortie d'erreur était jetée
(`2>/dev/null`) et son code de retour n'entrait pas dans le verdict. C'est
exactement le défaut que ce dépôt combat — déclarer vert sans preuve.

Cause : `git gc --auto`, déclenché par les commandes git lancées en parallèle, a
compacté `.git` **pendant** la lecture (1 341 objets épars remplacés par des
packfiles) → « file changed as we read it ».

Corrections apportées au script, et désormais vérifiées :

1. stderr de `tar` capturé et affiché, jamais jeté ;
2. `tar exit != 0` ⇒ `FAIL`, quel que soit le succès de l'extraction ;
3. le rapport porte le stderr de `tar` dans son tableau de champs.

Règle opérationnelle : **on archive `.git` sans aucune commande git
concurrente.** Une archive `.git` prise pendant un `gc` n'est pas une preuve.

## Ce que P0.1 prouve, et ce qu'elle ne prouve pas

**Prouvé** : l'archive se liste (6 204 entrées), s'extrait (exit 0), et restitue
le HEAD `f3d09c0` ainsi que **65 fichiers modifiés/non suivis à l'identique** de
la source, avec `cmp` identique sur `Cargo.toml`, `crates/core/src/lib.rs` et
`daemon/src/socket.rs`.

**Non prouvé** : que l'archive se **recompile**. `target/` est exclu par choix
(11 Go, régénérable) ; la première compilation après restauration prendra du
temps et n'a pas été chronométrée. À faire avant de considérer la restauration
comme éprouvée de bout en bout.
