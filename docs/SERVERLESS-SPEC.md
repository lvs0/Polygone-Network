# SERVERLESS-SPEC.md — Polygone Serverless

> *Spécification du crate `polygone-serverless`. Décrit ce qui est livré, ce qui
> est partiel, et ce qui n'est pas branché. Aucune promesse au-delà du code.*

---

## 1. Concept

Exécuter une fonction sur le réseau : un demandeur décrit une fonction et sa
charge utile, `total` nœuds l'exécutent, le demandeur n'accepte un résultat que
si assez de nœuds **indépendants** sont d'accord.

Ce service s'appuie sur ce qui existe déjà : l'identité et les reçus ML-DSA-65
de `crates/core`, le split Shamir 4-of-7 du même crate, et le transport
`compute` des nœuds fantômes. Le crate `polygone-serverless` apporte la
**modélisation, le découpage, le décompte et la vérification** ; il ne remplace
pas le transport.

## 2. Sémantique honnête du quorum

Un résultat n'est accepté que si `threshold` **nœuds distincts** renvoient le
même hash de sortie.

- Un nœud ne vote qu'une fois : deux réponses du même `NodeId` ne comptent pas
  double.
- Des groupes de sorties différentes sont rapportés tels quels
  (`Disagreement`), avec les nœuds de chaque groupe — le demandeur voit qui
  diverge, le crate ne choisit pas à sa place.
- En dessous du seuil, la réponse est `Insufficient { needed, got }`. Il n'y a
  jamais de « meilleur effort » silencieux.
- Le hash de sortie est `BLAKE3(output)`. Comparer des hashes évite de
  comparer des sorties de plusieurs mégaoctets.

## 3. Pourquoi Shamir ne s'applique pas au calcul

Le partage de secret de Shamir est **linéaire** : il permet de reconstruire un
secret à partir de `t` parts, pas d'exécuter une fonction arbitraire sur chaque
part puis de recombiner les sorties. `f(a) + f(b) ≠ f(a + b)` dès que `f` n'est
pas linéaire.

Conséquence, dite noir sur blanc :

- **Shamir sert au transit.** La charge utile est découpée en 7 parts, 4
  suffisent à la reconstituer ; aucune part seule ne révèle la charge utile.
- **Le calcul est répliqué, pas fragmenté.** Les `total` nœuds reçoivent la
  demande, exécutent, et le quorum décide. C'est plus coûteux (7 exécutions
  pour un résultat) et c'est la seule sémantique correcte sans preuve
  cryptographique de calcul.

Toute documentation qui annonce « exécution sur ≥4 fragments puis résultat
reconstruit » décrit un mécanisme qui n'existe pas mathématiquement.

## 4. Flux

```
demandeur
  │  ServerlessRequest { id, function, payload, threshold=4, total=7 }
  ├─ shards()            → 7 Shard (Shamir 4-of-7 de payload + hash BLAKE3)
  ├─ dispatch            → chaque Shard part vers un nœud (transport externe)
  │                         chaque nœud exécute, renvoie NodeOutcome
  │                         + Receipt ML-DSA-65 optionnel
  ├─ verify_receipts()   → ne garde que les sorties dont le reçu est valide
  └─ tally(outcomes, 4)  → Agreed { output, signers }
                           | Disagreement { groups }
                           | Insufficient { needed, got }
```

`reassemble(shards)` existe et est testé : il reconstruit la charge utile
d'origine à partir de 4 parts et vérifie le hash. Il ne sert **pas** à
recombiner un résultat de calcul.

## 5. États

| Élément | État | Détail |
|---|---|---|
| Types de requête, de part, de sortie, de verdict | 🟢 Livré | `crates/serverless/src/` |
| Split Shamir 4-of-7 de la charge utile | 🟢 Livré | `shards()`, testé |
| Reconstitution + vérification de hash | 🟢 Livré | `reassemble()`, testé |
| Quorum par nœuds distincts | 🟢 Livré | `tally()`, testé |
| Groupement des désaccords | 🟢 Livré | `QuorumVerdict::Disagreement` |
| Vérification des reçus ML-DSA-65 | 🟢 Livré | `verify_receipts()` |
| Commande `polygone serverless` | 🟡 Partiel | décrit la demande et ses parts ; n'envoie rien |
| Dispatch sur le relay | ⚪ Non branché | le transport `compute` existe séparément |
| Exécution WASM côté serveur | ⚪ Non branché | `wasmi` est déjà utilisé par `compute` |

## 6. Modes d'échec

| Cas | Comportement |
|---|---|
| Moins de `threshold` réponses | `Insufficient` — aucune sortie rendue |
| Deux nœuds, même sortie | un seul vote chacun ; le seuil reste à 4 |
| Sorties divergentes | `Disagreement`, groupes et nœuds listés |
| Part manquante | `reassemble` échoue explicitement |
| Part altérée | hash de charge utile différent → erreur |
| Reçu invalide | la sortie est écartée par `verify_receipts` |
| `threshold > total` | `validate()` refuse la requête |

## 7. Non-objectifs

- Pas de preuve de calcul (pas de SNARK, pas de redondance vérifiable
  cryptographiquement) : le quorum suppose des nœuds indépendants, ce qui n'est
  pas garanti par le protocole.
- Pas de réputation globale dans ce crate : `reputation.rs` du client reste
  local.
- Pas de facturation, pas de priorité, pas de SLA.
- Pas de persistance : une demande vit le temps de son quorum.

## 8. Vérification

```bash
cargo test -p polygone-serverless
```

Les tests couvrent : split/reconstitution à 4-of-7, échec à 3 parts, détection
d'une part altérée, quorum atteint, quorum insuffisant, nœud compté une seule
fois, désaccord rapporté, reçu invalide écarté.

---

*AGPL-3.0 · Posture honesty-first · « Le silence est le produit. »*
