# Polygone — Axiomes (v2.0.0)

## Axiome 1 — Confidentialité par défaut
Tout message transmis sur le réseau Polygone est chiffré par ML-KEM-1024 (NIST FIPS 203) avant transmission. Aucun message n'est en clair sur le réseau.

## Axiome 2 — Intégrité par signature
Chaque message est signé par ML-DSA-65 (NIST FIPS 204). Le destinataire vérifie la signature avant déchiffrement.

## Axiome 3 — Minimalisme de l'interface
L'interface client expose exactement deux commandes visibles : **Envoyer** et **Quitter**. Toute complexité est cachée.

## Axiome 4 — Autonomie du daemon
Le daemon `polygoned` tourne indépendamment, écoute le port 7000, et stocke les messages dans une mémoire volatile chiffrée.

## Axiome 5 — Post-quantique
Toute la cryptographie est résistante aux attaques quantiques. Aucun algorithme classique (RSA, ECDSA, ECDH) n'est utilisé.

## Axiome 6 — Protocole ouvert
Le protocole est documenté et ouvert. Toute implémentation compatible est la bienvenue.

## Axiome 7 — Preuve de réception
Chaque livraison génère un reçu signé ML-DSA-65, preuve cryptographique de réception.