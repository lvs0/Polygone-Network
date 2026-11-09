# MSG.6 — débit réel mesuré

> Mesure exécutée le **2026-11-07** sur loopback.
> Elle inclut le démarrage d'un processus, la génération de session, le chiffrement,
> la signature ML-DSA-65, la fragmentation 4/7, le relais et l'écriture du fichier reçu.
> Ce n'est **pas** un débit WAN et cela ne mesure pas le seul relais.

## Conditions

- Binaire client : `/home/lvs0/Polygone-Atlas/03-REALIGNED/target/debug/polygone`
- Relais : `/home/lvs0/Polygone-Atlas/03-REALIGNED/target/debug/polygone-relay` sur `127.0.0.1`
- Taille utile par envoi : **12288 octets** (12.0 Kio)
- Envois : **10**
- SHA-256 du contenu source : `6c4c3c0283cb111284874b5f4fc64abe5c06ae3301c6970e06902c3df096e3c3`
- Vérification : chaque fichier reçu a été relu et comparé octet à octet par SHA-256.

## Mesures

| Envoi | Durée (s) | Débit (Mio/s) |
|---:|---:|---:|
| 1 | 0.6597 | 0.018 |
| 2 | 0.4439 | 0.026 |
| 3 | 0.5392 | 0.022 |
| 4 | 0.7434 | 0.016 |
| 5 | 0.4547 | 0.026 |
| 6 | 0.3814 | 0.031 |
| 7 | 0.2702 | 0.043 |
| 8 | 0.2502 | 0.047 |
| 9 | 0.4217 | 0.028 |
| 10 | 0.2539 | 0.046 |

- Médiane : **0.027 Mio/s**
- Moyenne : **0.030 Mio/s**
- Écart-type (population) : **0.011 Mio/s**
- Minimum / maximum : **0.016 / 0.047 Mio/s**
- Durée médiane d'un envoi : **0.4328 s**

## Limites

- Loopback uniquement ; aucun pare-feu, aucune latence WAN, aucune perte.
- Le débit inclut le coût fixe du processus et de l'identité, significatif pour 12.0 Kio.
- Le relais plafonne une ligne à 64 Kio : un fichier plus gros est refusé avant envoi
  (`enveloppe trop volumineuse`). Cette mesure ne couvre donc pas le transfert de gros fichiers.
- Les 10 fichiers sont conservés dans un `HOME` temporaire supprimé après la mesure.
- Cette mesure **ne clôt pas MSG.6** : la preuve demandée est inter-hôtes, sur deux
  machines physiques. Le loopback ne mesure ni pare-feu, ni WAN, ni perte réelle.
