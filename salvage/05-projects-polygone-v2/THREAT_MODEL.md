# Polygone — Threat Model (v2.0.0)

## Menaces identifiées

### 1. Attaque par déchiffrement quantique
**Niveau** : Critique
**Mitigation** : Toute la cryptographie utilise ML-KEM-1024 (KEM) et ML-DSA-65 (signature), résistants aux attaques quantiques. Un ordinateur quantique ne peut pas déchiffrer les messages passés.

### 2. Attaque par l'homme du milieu (MITM)
**Niveau** : Élevé
**Mitigation** : Chaque message est signé par le sender avec ML-DSA-65. Le destinataire vérifie la signature avant déchiffrement. Un attaquant ne peut pas se faire passer pour un pair sans la clé de signature.

### 3. Attaque par répétition / replay
**Niveau** : Moyen
**Mitigation** : Chaque message contient un timestamp et un identifiant unique. Le destinataire peut rejeter les messages dupliqués ou trop anciens.

### 4. Saturation du réseau
**Niveau** : Moyen
**Mitigation** : Le daemon limite le nombre de connexions (64 par défaut) et applique un rate-limit sur les connexions entrantes.

### 5. Compromission d'un nœud
**Niveau** : Faible
**Mitigation** : La compromise d'un nœud relais ne compromet que les messages passés par ce nœud. Les messages entre autres pairs restent chiffrés.

### 6. Attaque par le serveur
**Niveau** : Faible
**Mitigation** : Le serveur relais ne stocke que des enveloppes chiffrées. Il ne peut pas les déchiffrer sans la clé privée du destinataire.

## Conclusion
Le modèle de menaces de Polygone est conçu pour la résilience post-quantique. Aucune menace identifiée n'est non mitigée.