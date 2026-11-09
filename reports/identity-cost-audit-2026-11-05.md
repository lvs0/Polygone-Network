# Mesure du coût de création d'identités Polygone

> Mesure locale datée du 2026-11-05T22:07:06+0100. Ce rapport quantifie le coût de création sur cette machine ; il ne prouve pas une résistance Sybil.

## Protocole

- Binaire : /home/lvs0/Polygone-Atlas/03-REALIGNED/target/release/polygone
- SHA-256 du binaire : 3a6f4d2500ef2d2c0b80f2825f6c9c9a30ca081155b6c8f52095dcc8da95ed24
- Machine : Intel(R) Core(TM) i5-5300U CPU @ 2.30GHz (Linux-7.2.5-3-omarchy-x86_64-with-glibc2.44)
- Échantillons : 30 identités indépendantes, HOME temporaire neuf à chaque fois
- Action mesurée : lancement de polygone id, génération ML-KEM-1024 + ML-DSA-65, sérialisation et écriture de identity.json.
- Aucune identité existante n'est lue ou modifiée ; tous les HOME temporaires sont supprimés à la fin.

## Résultats

| Mesure | Résultat |
|---|---:|
| Durée médiane | 2.85 ms |
| Durée moyenne | 2.85 ms |
| P95 | 3.07 ms |
| Minimum / maximum | 2.70 / 3.19 ms |
| Débit séquentiel observé | 21065.1 identités/min (hors concurrence) |
| Identités distinctes | 30/30 |
| Taille identity.json | 21641–21641 octets |
| Modes de fichier observés | 600 |

## Interprétation et limites

- Une identité coûte ici une génération de deux paires post-quantiques, un lancement de processus et une écriture locale ; ce n'est ni un prix, ni une preuve d'identité humaine.
- Les identités sont créables hors ligne et aucune autorité centrale, preuve de personne, dépôt de garantie ou quota global ne limite leur nombre dans ce protocole.
- Les quotas par identité limitent un pair donné, mais un adversaire peut créer plusieurs identités : Polygone ne doit pas être présenté comme Sybil-résistant sur cette seule base.
- La mesure est séquentielle sur un seul ThinkPad ; elle n'est pas un benchmark parallèle, multi-machine ou adversarial.
