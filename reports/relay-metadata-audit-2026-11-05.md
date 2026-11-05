# Audit des métadonnées visibles par le relais

> **Instantané daté.** Rapport généré par
> `scripts/relay-metadata-audit.mjs` — SEC.10. Il décrit une exécution
> locale du 2026-11-05T21:00:25.655Z, pas une propriété intemporelle.
> L'état courant et les limites sont suivis dans `STATE.md`.

## Dispositif réellement exécuté

- Un relais `polygone-relay --port 38025` (TCP réel).
- Un émetteur `polygone envoyer --fichier` (~13 Kio, marqueur de contenu).
- Un récepteur `polygone ecouter` (identité `9bf23f9f196beec0`).
- Un proxy d'observation local (port 37833) qui relaie chaque octet
  et journalise les trames dans les deux sens.
- 2 connexions TCP, 2 HELLO, 16 fragments, 1 session(s).

## Ce que le relais voit

| Champ | Visible | Preuve mesurée |
|---|---|---|
| Identité source (`from`/HELLO) | **oui** | 2 HELLO, sources : c4eaaa56c2a7d307 |
| Identité destination (`to`) | **oui** | destinations : 9bf23f9f196beec0 |
| Taille des trames | **oui** | 21640–47566 octets (médiane 47425) |
| Cadence | **oui** | 16 fragments en 45 ms ; intervalle médian 1 ms |
| Horodatage d'observation | **oui** | chaque trame est datée à la réception par l'observateur |
| Identifiant de session | **oui** | 1 session(s) : 1d09ab588ad38065 |
| Contenu du message/fichier | **non** | marqueur `POLYGONE-METADATA-AUDIT-CONTENT-MARKER-7f3c` absent des 707496 octets observés et des journaux |
| Nom du fichier | **non** | chiffré hors bande ; aucune trame ne le contient |

## Limites de cette mesure

- Tout est sur **une seule machine** (loopback) : la mesure prouve ce que
  l'observateur voit sur le fil, pas la topologie entre deux hôtes physiques.
- L'observateur est un proxy local : un opérateur qui modifie le binaire du
  relais verrait la même chose, mais pourrait aussi conserver les données.
- Les horodatages sont ceux de l'observateur ; ils incluent l'ordonnancement
  local et ne constituent pas une mesure de latence réseau.
- Le relais ne voit pas le contenu, mais voit **qui parle à qui** : la
  confidentialité porte sur le message, pas sur le graphe social.

## Reproduction

```bash
cargo build -p polygone-client --bin polygone -p polygone-relay
node scripts/relay-metadata-audit.mjs
```
