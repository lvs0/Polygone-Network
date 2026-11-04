# Décision de cadrage — POLY et réciprocité

> Version de portage du 4 novembre 2026. Cette note formalise les garde-fous
> techniques retenus ; elle ne remplace pas les décisions encore ouvertes dans
> NEEDS-LEVY.md.

## Décision actuelle

**Ne pas créer de cryptomonnaie POLY.** Pas de blockchain, jeton transférable,
marché, conversion monétaire, portefeuille spéculatif ou accès conditionné à
un actif.

L'idée produit à préserver est la réciprocité : une personne peut contribuer
volontairement des ressources et obtenir une mesure compréhensible de sa
contribution au réseau. Cette idée ne justifie pas à elle seule un mécanisme
comptable en production.

## Ce qui peut être étudié

Une unité interne non transférable peut être étudiée uniquement après :
1. Une preuve réelle de service rendu et consommé.
2. Des reçus mesurés, vérifiables et signés par les parties concernées.
3. Des limites explicites de dette et de crédit.
4. Une protection contre les doubles comptes, la collusion et les nœuds Sybil.
5. Une procédure de contestation et de révocation.
6. Une décision explicite du fondateur sur D8 dans NEEDS-LEVY.md.

Cette piste reste bloquée tant que les preuves de service distribué ne sont pas
réelles. Elle ne doit pas être codée en avance sur l'infrastructure.

## Invariants

- POLY n'est pas une cryptomonnaie.
- Aucun transfert entre utilisateurs.
- Aucune convertibilité, émission spéculative, intérêt ou marché.
- Aucun lien avec un fournisseur d'inférence ou un quota de modèle.
- Aucun solde attribué sur la seule base d'une intention ou d'une annonce.
- Un crédit ne peut être attribué qu'à partir d'un service effectivement vérifié.
- L'utilisateur doit pouvoir consulter, contester et comprendre les reçus.

## Preuve minimale avant toute implémentation

BLOCKS.9 doit démontrer un transfert réel entre deux machines physiques, avec
hashes identiques et mesure réseau. Ensuite seulement, le fondateur peut
réexaminer D8. RECIP.1 reste bloquée jusqu'à cette preuve et à la décision humaine.

## Références

- NEEDS-LEVY.md — décision D8 et limites réservées au fondateur.
- daemon/src/socket.rs — ledger local et allocations.
- docs/atlas/plan/epiques/03-calcul-ia.md — capacités de ressources.
- docs/atlas/plan/epiques/06-confiance.md — preuves et confiance.
