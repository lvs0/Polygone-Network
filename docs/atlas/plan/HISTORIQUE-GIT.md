# HISTORIQUE GIT — diagnostic et plan de nettoyage

> **Instantané daté — ne pas lire comme l'état courant.** Ce document est rédigé le 2026-11-03 ; il ne décrit pas l'état vérifié aujourd'hui, qui se lit dans [`ETAT-REEL.md`](../../../ETAT-REEL.md) et [`STATE.md`](../../../STATE.md).

> Mesuré le 2026-11-03. **Aucune réécriture n'a été exécutée.** Ce document dit
> ce qui est cassé, ce qu'un historique propre devrait être, et les commandes
> exactes — à lancer seulement après sauvegarde et accord.

---

## 1. Ce que les deux dépôts contiennent réellement

| | **Polygone-Network** (checkout Polygone-Network) | **Polygone-Atlas** |
|---|---|---|
| Commits sur la branche locale | 14 (`realign/v1`) | 7 (`main`) |
| Auteurs distincts | 3 (`lvs0`, `Buffy`, `Levy Verpoort Scherpereel`) | 3 (les mêmes) |
| Période | 2 jours (2026-11-02 → 03) | 2 jours |
| `origin/main` | **142 commits**, 2026-06-05 → 2026-09-11, auteur `Relay` à 136/142 — **ligne non liée** (aucune racine commune) | à jour sauf 3 commits locaux |
| Branches distantes parasites | `fix/ci-rustfmt`, `wip/ship-v1.0.0` | — |
| Tags | `v2.0.0`, `v2.0.0-rc2` — posés sur la ligne `main` | — |
| Fichiers suivis | 268 | 58 |
| Blobs > 500 Ko dans l'historique | 3 (un `block-1MiB.bin` de 1 Mio, deux PNG de ~700 Ko) | — |

**Le fait qui change tout :** `git merge-base HEAD origin/main` ne rend **rien**.
`realign/v1` part de sa propre racine (`3e8d78a`, 2 novembre 2026, 268 fichiers
dans un seul commit) ; `origin/main` part de `69293a4` (5 juin 2026) et compte
142 commits jusqu'au tag `v2.0.0`. Ce ne sont pas « 142 commits de retard » mais
**deux historiques sans ancêtre commun**.

Deux corrections en découlent :

1. Le chiffre de 17 commits dans `ETAT-REEL.md` §8 ne décrit pas la relation à
   `origin/main` : il vient d'une comparaison avec une autre copie locale. Sur
   ce dépôt-ci, il n'y a pas de divergence à réconcilier — il y a une ligne
   abandonnée à archiver, ou à ressusciter entièrement.
2. Un `merge` est impossible sans `--allow-unrelated-histories`, et le faire
   produirait un dépôt à deux racines : exactement ce qu'un historique propre
   évite.

**Ce que ça dit du passé :** les deux lignes d'historique sont des rafales
automatiques. `Relay`, `Ruflo`, `Zoe` sont les identités des anciens harnais
(`polygone-company`, `polygone-team`) ; les 14 commits locaux sont ceux d'une
session de deux jours. Il n'existe, nulle part, un historique « humain » à
nettoyer : il y a deux productions de machine et un tag `v2.0.0` posé sur l'une.

---

## 2. Ce que « propre » veut dire ici

Sept critères, dans l'ordre d'importance. Un historique qui les respecte se lit
comme du travail de senior, parce qu'il **raconte des décisions**, pas des
sauvegardes.

1. **Un seul auteur.** `git config user.name/user.email` identiques partout.
2. **Chaque commit compile et passe ses tests** au moment où il est écrit.
3. **Un commit = un problème résolu**, pas une séance de travail. Pas de
   `wip`, `fix fmt`, `oups`, ni de commit de bookkeeping (`STATE.md`).
4. **Le message dit le pourquoi**, avec la preuve : `module: décision.
   Preuve : commande → résultat.`
5. **Aucune ligne morte dans l'historique** : pas de binaire de 1 Mio ajouté
   puis retiré, pas de fichier `.bak`, pas de secret (même supprimé ensuite).
6. **Une seule ligne de vie par dépôt** : `main`, linéaire, taguée aux jalons.
7. **L'historique ne raconte pas autre chose que la réalité.** Un commit daté
   du 12 mars alors qu'il a été produit le 3 novembre est un faux — et c'est
   exactement ce que `DOC.1` interdit désormais aux documents.

---

## 3. Niveau A — consolidation honnête *(recommandé, ~1 h)*

On garde les commits réels, on supprime le bruit, on unifie l'auteur, on remet
les messages au format. Résultat : **10 commits** dans Network, **4** dans Atlas,
tous datés d'aujourd'hui ou d'hier, tous déjà verts au moment où ils ont été
écrits.

### Polygone-Network — série cible

| # | Message | Contenu |
|---:|---|---|
| 1 | `realign: un seul workspace canonique pour les 15 versions` | `3e8d78a` — **l'arbre entier** : c'est le commit qui rend le niveau B nécessaire |
| 2 | `daemon: mesurer, calculer, proposer — sans prétendre contraindre` | capacité mesurée / réservable / contrainte (`0884449`) |
| 3 | `daemon: un bail porte une échéance et se renouvelle` | `315f0eb` |
| 4 | `daemon: prouver qu'un crash ne bloque rien` | `d074b7c` (script) |
| 5 | `msg: le TTL de session tient les 30 s publiés` | `ac71668` |
| 6 | `web: retirer les chiffres inventés et les promesses fausses` | `f2e0a59` + `3921408` |
| 7 | `docs: vérifier les chiffres publiés au lieu de les recopier` | `288d67e` + `66c4ec8` |
| 8 | `docs: état réel, réconciliation des commits, roadmap` | `beda7de`, `4993761`, `3235383` |
| 9 | `scripts: archiver et rejouer les preuves` | `dc59714` |
| 10 | `state: tracer les tâches fermées et leur preuve` | `205bb70` |

Les commits `STATE.md` (2 et 10) peuvent aussi disparaître complètement : un
fichier d'état n'a pas besoin d'un commit à lui. Un senior le mettrait dans le
commit qui le rend vrai.

### Polygone-Atlas — série cible

| # | Message | Contenu |
|---:|---|---|
| 1 | `atlas: consolider 15 versions et poser le plan maître` | `93771ec` |
| 2 | `decisions: pas de cryptomonnaie — une unité de réciprocité bornée` | `25f95c2` |
| 3 | `plan: 132 tâches, 6 portes, et le reste de l'écosystème` | `e8b6745` |
| 4 | `agent: backlog exécutable et dépendances appliquées` | `5600ed2`, `b883564` |

### Méthode

```bash
# 1. Sauvegarde d'abord — un bundle n'est pas « un backup quelque part ».
git bundle create ../network-before-rewrite.bundle --all
git tag archive/avant-nettoyage-2026-11-03

# 2. Un seul auteur pour toute la série.
git config user.name "Levy Verpoort Scherpereel"
git config user.email "<ton adresse>"

# 3. Réécriture interactive sur la racine (14 commits, faisable à la main).
git rebase -i --root

# 4. Vérification : l'arbre final doit être IDENTIQUE, octet pour octet.
git diff archive/avant-nettoyage-2026-11-03 HEAD --stat   # doit être vide

# 5. Publication, seulement après (4) : --force-with-lease, jamais --force.
git push --force-with-lease origin realign/v1
```

Étape 4 non négociable : si `git diff` n'est pas vide, la réécriture a changé du
contenu, pas seulement l'emballage.

---

## 4. Niveau B — reconstruire par domaine *(lourd, plusieurs jours)*

Une vraie série « senior » aurait un commit par domaine, chacun compilable :

```
core: identité post-quantique, reçus signés, partage 4/7
daemon: protocole de bail sur socket Unix
daemon: isolation cgroup v2, vérifiée
relay: relais aveugle qui refuse l'usurpation d'expéditeur
client: transport chiffré et signé
client: messagerie éphémère (7 fragments, réassemblage)
client: stockage de blocs, exécution sandboxée (systemd, wasmi)
gateway: API HTTP locale
petals / serverless: inférence et calcul délégués
web + install: parcours utilisateur, installateur fail-closed
docs: menaces, état réel, limites
```

Ce découpage existe déjà dans les 15 épiques. Le produire **à partir de l'arbre
final** oblige à fabriquer des états intermédiaires qui compilent : c'est un
travail de reconstruction, pas de nettoyage, et chaque étape doit être vérifiée
par `cargo test` sur un arbre partiel. Faisable, mais c'est plusieurs jours, et
un commit intermédiaire cassé ruine tout l'effort.

---

## 5. Ce que je ne ferai pas, et pourquoi une phrase suffit

Je ne **antidaterai** pas les commits et je ne fabriquerai pas de métadonnées
destinées à faire croire que ce travail a été écrit à la main sur plusieurs mois
sans assistance.

Raison : c'est la même faute que les 18 capacités fausses retirées le 2 novembre
et que le `TTL ≤ 30 s` corrigé ce soir — un document qui affirme autre chose que
ce qui s'est passé. Ce projet a passé une journée à rendre ses affirmations
vérifiables ; un historique fabriqué serait la première chose qu'un relecteur
technique remarque, et la seule qui ne se corrige pas par un commit.

**Le résultat cherché s'obtient sans ça.** Un historique de senior se reconnaît
à trois choses, aucune n'étant la date : des commits qui racontent une décision,
un seul auteur, et des messages qui portent leur preuve. Les niveaux A et B
donnent exactement ça, avec des dates vraies. Et une ligne dans le README —
« développé avec assistance IA, décisions et revue par l'auteur » — vaut mieux
qu'un passé inventé : elle est vérifiable, elle ne se retourne pas contre toi,
et elle n'enlève rien au travail.

---

## 6. Risques à traiter avant toute réécriture

1. **`v2.0.0` est un tag public** posé sur la ligne des 142 commits. Réécrire
   `main` sans décider du sort de ce tag laisse une version publiée qui ne
   correspond plus à rien.
2. **Les 142 commits ne sont pas inventoriés.** `RECONCILIATION.md` en couvre 17,
   et pas ceux-là. Avant de choisir entre « archiver `main` » et « rebaser
   dessus », il faut refaire l'inventaire (auteur `Relay` = ancien harnais,
   donc probablement rien à sauver — mais « probablement » n'est pas un
   inventaire).
3. **Deux branches distantes parasites** (`fix/ci-rustfmt`, `wip/ship-v1.0.0`) :
   à supprimer explicitement, pas à oublier.
4. **Trois blobs lourds** dans l'historique : `phase0/preuves/block-1MiB.bin`,
   `brand/examples/landing-final.png`, `landing-full.png`. Si un jour le dépôt
   doit être publiable, ils partent avec `git filter-repo --strip-blobs-bigger-than`.
5. **Un `--force` sur un dépôt public casse les clones existants.** D'où
   `--force-with-lease`, une branche de secours poussée avant, et un tag
   d'archive.

---

## 7. Résultat du niveau A — exécuté le 2026-11-03

Fait, localement, sur de nouvelles branches ; les anciennes branches et leurs
tags d'archive sont intactes.

| Dépôt | Avant | Après | Vérification |
|---|---:|---:|---|
| `Polygone-Network` | 14 commits, 3 auteurs | **11 commits**, 1 auteur | `git rev-parse^{tree}` identique octet pour octet |
| `Polygone-Atlas` | 7 commits, 3 auteurs | **5 commits**, 1 auteur | idem |

Auteur unique : `Levy Verpoort Scherpereel <levy11vs@gmail.com>`. Les dates
d'auteur d'origine sont **conservées** : aucune antidatation.

### Ce qui a été fait exactement

1. Fusion des commits de bookkeeping dans le commit qui les rend vrais :
   `STATE.md` + `BACKLOG` rejoignent la décision qu'ils enregistrent ; le script
   d'archive rejoint la preuve d'archive qu'il rejoue.
2. Messages réécrits au format `module: décision.` puis le problème, la décision
   et la preuve — chaque message nomme la commande qui l'a validé.
3. Auteur unifié ; branches nouvelles (`realign/v2`, `main-v2`) et non
   réécriture en place, pour que l'ancien état reste consultable.
4. Tags `archive/avant-nettoyage-2026-11-03` posés dans les deux dépôts.

### Ce qui reste à faire, et qui demande un accord

- `git push --force-with-lease origin realign/v2:realign/v1` (Network) et
  `git push --force-with-lease origin main-v2:main` (Atlas) — **non exécuté** :
  cela réécrit l'historique publié.
- Décider du sort de `v2.0.0` et des branches distantes `fix/ci-rustfmt` et
  `wip/ship-v1.0.0`.
- Inventorier les 142 commits de `origin/main` avant de choisir entre les
  archiver et les ressusciter.
