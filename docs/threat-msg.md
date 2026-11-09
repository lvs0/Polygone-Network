# Métadonnées visibles par le relais

> **Instantané daté.** Mesure SEC.10 du 2026-11-05. Ce document décrit ce qu'un
> observateur du relais voit réellement pendant un envoi, pas une propriété
> intemporelle. L'état courant est suivi dans `STATE.md` ; le rapport brut daté
> est [`reports/relay-metadata-audit-2026-11-05.md`](../reports/relay-metadata-audit-2026-11-05.md).

Le message est chiffré de bout en bout : le relais ne lit pas le contenu. Il
lit en revanche **qui parle à qui, quand, et pendant combien de temps**. Ce
document nomme ces champs au lieu de laisser « on voit rien » suggérer une
anonymat qu'aucun code ne fournit.

## Ce qui est visible

| Champ | Visible | Détail mesuré |
|---|---|---|
| Identité source | **oui** | `HELLO <node_id>` puis champ `from` de chaque enveloppe |
| Identité destination | **oui** | champ `to` utilisé pour le routage |
| Taille des trames | **oui** | 21 640 à 47 566 octets par fragment lors de la mesure |
| Cadence | **oui** | 16 fragments en 121 ms, intervalle médian 3 ms |
| Horodatage | **oui** | daté à la réception par l'observateur |
| Identifiant de session | **oui** | champ `session`, corrélable entre fragments |
| Contenu du message ou du fichier | **non** | marqueur de test absent des 707 124 octets observés |
| Nom du fichier | **non** | chiffré hors bande, absent des trames |

## Ce que cela signifie

1. **Confidentialité du contenu** : tenue sur ce chemin (le marqueur n'apparaît
   ni sur le fil ni dans les journaux du relais).
2. **Confidentialité du graphe** : non tenue. Un opérateur du relais peut
   reconstruire qui envoie à qui, à quelle cadence et avec quelles tailles.
3. **Corrélation** : la session et les tailles permettent de rapprocher des
   fragments d'un même envoi sans déchiffrer quoi que ce soit.
4. **Nom de fichier** : non exposé par le relais, mais les tailles et la
   cadence peuvent révéler des propriétés du fichier.

## Limites de la preuve

- La mesure est faite **sur une seule machine**, en loopback, avec un proxy
  d'observation local. Elle ne prouve pas un comportement entre deux hôtes
  physiques (`BLOCKS.9` reste ouvert pour cela).
- L'observateur est placé devant le relais : c'est le pire cas côté opérateur
  pour les métadonnées, pas une preuve d'anonymat réseau.
- Le relais ne stocke rien par conception, mais rien n'empêche un opérateur de
  journaliser ce qu'il reçoit. Le code ne peut pas prouver l'absence de
  conservation chez un tiers.

## Reproduction

```bash
cargo build -p polygone-client --bin polygone -p polygone-relay
node scripts/relay-metadata-audit.mjs
```

Le script refuse de conclure si moins de deux connexions réelles, moins de deux
`HELLO` ou aucune trame `fragment` n'ont été observés, et échoue si le marqueur
de contenu apparaît sur le fil ou dans un journal.
