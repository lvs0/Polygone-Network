# Carnet d'observation — Premier Soir

> *Le livrable du Premier Soir. Rempli le soir même, commité le jour même.*
> *« La sortie n'est pas un tag. La sortie est une personne qui n'est pas
> toi qui utilise Polygone un soir, et ton carnet qui le prouve. »*
> *Modèle : v1 (2026-08-08) — à remplir au premier soir réel.*

---

## 0. Identité du soir

| Champ | Réponse |
|-------|---------|
| **Date** | `___` |
| **Lieu** | `___` |
| **Binaires** | tarball `polygone-<os>-<arch>.tar.gz` + `sha256sum` noté sur papier |
| **Build vérifié** | `./target/release/polygone test` → `___ /7` · `cargo test --workspace` → `___ /109` |
| **Participants** (pseudos) | Alice : `___` · Bob : `___` · Adversaire : `___` |

## 1. Le protocole (rappel — deux promesses, rien d'autre)

1. Un message éphémère traverse le relay : 7 fragments naissent, le TTL
   tourne réellement, 4/7 reconstruisent, `verite` montre « rien ».
2. Un fichier traverse le relay : contenu identique à l'arrivée, puis
   plus rien.

## 2. Grille d'observation

> Réponds aux 5 questions de `docs/PREMIER-SOIR.md` §5 — textuel.

**Q1 — Quelle promesse a été tenue ? Quelle promesse a failli ?**
(précis, cite la commande)

```
(à remplir)
```

**Q2 — Qu'ont dit Alice/Bob/l'adversaire textuellement ?**

```
Alice : « ... »
Bob : « ... »
Adversaire : « ... »
```

**Q3 — Qu'est-ce qui les a bloqués (install, clé, réseau) ?**

```
(à remplir)
```

**Q4 — Qu'est-ce qu'ils ont voulu faire que le produit ne permet pas ?**

```
(à remplir)
```

**Q5 — Décision stratégique de la semaine suivante (sur CE document).**

```
(à remplir)
```

## 3. Les trois preuves

| Preuve | Attendu | Constaté |
|--------|---------|----------|
| Message | « je n'ai pas pu lire le contenu » (le relay) | `☐` |
| Fichier | contenu identique à l'arrivée (`cmp`) | `☐` |
| Clé | carte échangée en personne (résidu social) | `☐` |

## 4. La vérité vérifiée par chacun

- [ ] Chaque participant a lancé `polygone verite` lui-même.
- [ ] Verdict lu à voix haute : « voici ce que j'ai de toi : rien. »
- [ ] Chacun a vu les 7 fragments naître et le TTL mourir.

## 5. Le résidu social

Clés échangées ce soir : `___` (noms ou pseudos).
Contacts ajoutés : `___`.

## 6. Les métriques du produit++ (pas de MAU)

| Métrique | Valeur |
|----------|--------|
| Taux de complétion du premier soir (sur N participants) | `___ / ___` |
| Retour au deuxième soir (à re-mesurer) | `___` |
| Échanges de clés | `___` |
| Runs de `verite` | `___` |

## 7. Verdict collectif

> Une phrase chacun : est-ce que vous croyez que le message est mort ?

```
(à remplir)
```

## 8. Après le soir

- [ ] Carnet commité le jour même (`git add docs/observation-premier-soir.md`).
- [ ] README mis à jour : « testé par N personnes le <date> » — le seul
      chiffre qui compte.
- [ ] Décision stratégique de la semaine consignée dans le plan.

---

*Ce document est le modèle. Le jour du soir, il devient le livrable.*
*AGPL-3.0 · Polygone · « Le message meurt. Regarde. »*
