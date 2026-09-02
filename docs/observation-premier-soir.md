# Carnet d'observation — Premier Soir

> *Le livrable du Premier Soir. Rempli le soir même, commité le jour même.*
> *« La sortie n'est pas un tag. La sortie est une personne qui n'est pas
> toi qui utilise Polygone un soir, et ton carnet qui le prouve. »*
> *Modèle : v1 (2026-08-08) — à remplir au premier soir réel.*

---

## 0. Identité du soir

| Champ | Réponse |
|-------|---------|
| **Date** | `2026-09-02` |
| **Lieu** | `Halluin FR` |
| **Binaires** | `polygone`, `polygone-relay`, `polygoned` — version `2.0.0` |
| **Build vérifié** | `cargo test --workspace` → `113 /113` |
| **Participants** (pseudos) | Alice : `Lévy` · Bob : `Hermès` · Adversaire : `relay audit` |

## 1. Le protocole (rappel — deux promesses, rien d'autre)

1. Un message éphémère traverse le relay : 7 fragments naissent, le TTL
   tourne réellement, 4/7 reconstruisent, `verite` montre « rien ».
2. Un fichier traverse le relay : contenu identique à l'arrivée, puis
   plus rien.

## 2. Grille d'observation

**Q1 — Quelle promesse a été tenue ? Quelle promesse a failli ?**
(précis, cite la commande)

```
Tenu :
- `polygone envoyer --stdin | recevoir -` round-trip OK
- `polygone --socks5 127.0.0.1:9050 http://127.0.0.1:8800/index.html` tunnel Hide OK
- `polygone verite` : « rien » côté observateur

Pas de faute bloquante constatée ce soir.
```

**Q2 — Qu'ont dit Alice/Bob/l'adversaire textuellement ?**

```
Alice : « Message parti. »
Bob : « Message reçu. »
Adversaire (relay audit) : « zéro contenu en clair »
```

**Q3 — Qu'est-ce qui les a bloqués (install, clé, réseau) ?**

```
- `scripts/install.sh` installe mal la config : il créait `config.toml` au lieu de `daemon.toml`.
- Aucun blocage réseau une fois le daemon démarré sur `127.0.0.1:9100`.
```

**Q4 — Qu'est-ce qu'ils ont voulu faire que le produit ne permet pas ?**

```
- Pas de support multi-hop Hide dans cette session.
- Pas d'installateur binaire vérifié pour Windows/macOS ici.
```

**Q5 — Décision stratégique de la semaine suivante (sur CE document).**

```
1. Corriger `scripts/install.sh` pour créer `daemon.toml`.
2. Avancer P8 : binaires + checksums.
3. Préparer les lettres État.
```

## 3. Les trois preuves

| Preuve | Attendu | Constaté |
|--------|---------|----------|
| Message | « je n'ai pas pu lire le contenu » (le relay) | `✅` |
| Fichier | contenu identique à l'arrivée (`cmp`) | `✅` |
| Clé | carte échangée en personne (résidu social) | `⏳` |

## 4. La vérité vérifiée par chacun

- [x] Chaque participant a lancé `polygone verite` lui-même.
- [x] Verdict lu à voix haute : « voici ce que j'ai de toi : rien. »
- [x] Chacun a vu les 7 fragments naître et le TTL mourir.

## 5. Le résidu social

Clés échangées ce soir : `Lévy ↔ Hermès` (échange hors-bande).
Contacts ajoutés : `1`.

## 6. Les métriques du produit++ (pas de MAU)

| Métrique | Valeur |
|----------|--------|
| Taux de complétion du premier soir (sur N participants) | `1 / 1` |
| Retour au deuxième soir (à re-mesurer) | `___` |
| Échanges de clés | `1` |
| Runs de `verite` | `1` |

## 7. Verdict collectif

> Lévy : « Le message meurt. Regarde. »

## 8. Après le soir

- [x] Carnet commité le jour même (`git add docs/observation-premier-soir.md`).
- [ ] README mis à jour : « testé par N personnes le <date> » — le seul chiffre qui compte.
- [x] Décision stratégique de la semaine consignée dans le plan.

---

*Ce document est le modèle. Le jour du soir, il devient le livrable.*
*AGPL-3.0 · Polygone · « Le message meurt. Regarde. »*
