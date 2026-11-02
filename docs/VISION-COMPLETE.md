# Polygone — vision unifiée et état vérifié

> Document de cadrage : distinguer les capacités réellement exécutées, les prototypes locaux et les objectifs futurs. Une fonction présente ou un test unitaire ne suffit pas à déclarer un service livré.
> Audit détaillé : `AUDIT_VERACITE_VISION.md`.

---

## 1. Calcul et exécution distribuée

- **Compute (partiel mais utilisable)** : un nœud peut recevoir une requête via le relay et exécuter une commande shell dans un scope systemd contraint, ou un module WASM via wasmi (`crates/client/src/net/mod.rs`, `crates/client/src/exec.rs`). Les grants sont signés ML-DSA-65 et vérifiés par le demandeur (`net/mod.rs`, `verify_grant`).
- **Limites Compute** : la réputation est locale et affichée ; elle ne sélectionne pas automatiquement les pairs. Il n'existe pas encore de reçu signé de résultat d'exécution ni d'échange de réputation réseau. Le grant signé prouve l'autorisation/réponse du nœud, pas la justesse du résultat.
- **Serverless (prototype local)** : le crate sait fragmenter une charge utile par Shamir, la reconstituer et calculer un quorum en fonctions pures. La commande actuelle vérifie seulement le split/reassemble local ; elle contacte zéro nœud. Le transport, l'exécution distante orchestrée et le décompte des réponses ne sont pas câblés.
- **Petals** : l'IA locale via Ollama est disponible sous `polygone petals`. Le planner de pipeline de couches existe, mais le transfert des poids et l'inférence distribuée entre pairs ne sont pas branchés.

## 2. Gateway et API

- Le crate `polygone-gateway` sert une interface statique et des routes JSON sur un vrai socket TCP. Routes présentes : statut, services, demande/acceptation d'appairage, deep-link et information d'un nœud.
- Le binaire écoute par défaut sur `127.0.0.1:8088` et refuse désormais tout bind non-loopback. Il ne termine pas TLS ; le Gateway ne peut pas encore être exposé publiquement même derrière une simple variable d'adresse.
- Les données de nœuds sont conservées en mémoire dans le processus Gateway. Elles ne sont pas alimentées par le relay ; les mesures de latence ne disposent pas encore de producteur en production.
- L'appairage est un MVP par jeton porteur : secret aléatoire de 256 bits, expiration à cinq minutes, plafond de demandes pendantes et consommation atomique à l'acceptation (rejeu refusé). Cela ne constitue pas encore une authentification mutuelle : aucune preuve signée de possession de la clé associée n'est exigée, et le Gateway ne termine pas TLS.

## 3. Site et PWA

- Le site statique se trouve dans `web/` (`index.html`, `landing.html`). Il n'est pas généré automatiquement depuis les documents et aucun workflow de déploiement web n'est présent dans `.github/workflows/`.
- La PWA possède un manifeste, un service worker, une coque mise en cache et une page hors ligne. L'installation dépend des capacités du navigateur.
- La PWA n'exécute pas de daemon réseau et ne transforme pas un téléphone en nœud Polygone.

## 4. Mobile comme nœud

- **Non implémenté** : consentement de contribution, participation au réseau, relais/compute mobile, pause selon batterie ou type de connexion, synchronisation en arrière-plan.
- Ces fonctions restent des objectifs de conception et ne doivent pas être présentées comme disponibles.

## 5. État du réseau

- `/api/v1/network/status` renvoie le nombre d'entrées, les entrées vues récemment, une médiane de latence optionnelle et l'uptime du processus Gateway.
- **Limite majeure** : la table est locale au processus ; elle n'est pas synchronisée avec le réseau Polygone. Le champ de latence est `null` tant qu'aucune mesure n'est alimentée. Pas de carte mondiale, de pays estimés ni d'historique 24 h.

## 6. Comptes utilisateurs

- Aucun compte réseau, `polygone login`, enrollment token ou agrégation multi-appareils n'est livré.
- L'identité actuelle est une paire de clés locale persistée sur l'appareil. Les relais facultatifs transportent les enveloppes ; ils ne constituent pas un système de compte.

## 7. QR, deep links et passkeys

- L'API Gateway peut retourner une chaîne `polygone://pair/<token>` pour un token présent en mémoire.
- Aucun générateur de QR, gestionnaire de deep-link iOS/Android, intent filter ou flux WebAuthn/passkey n'est câblé.
- Le parcours d'appairage doit être sécurisé avant d'être exposé comme fonctionnalité produit.

## 8. Sécurité et confidentialité

- Le chemin de message réseau utilise ML-KEM-1024, AES-256-GCM, Shamir 4-of-7 et une signature ML-DSA-65 sur l'enveloppe KEM ; le relay ne lit pas le contenu chiffré.
- Le relay reste un serveur réseau : il voit l'adresse IP de connexion et les identifiants de routage, et ses journaux peuvent contenir ces métadonnées. « Aveugle » signifie qu'il ne lit pas le payload, pas qu'il ne voit aucune métadonnée.
- `polygone duress --confirmer` est une commande manuelle qui détruit certains états locaux. Aucun déclenchement automatique sur compromission ni déclencheur USB/clavier/GPIO n'est implémenté.
- Le dépôt est sous AGPL-3.0 et possède `SECURITY.md` ainsi que `.well-known/security.txt`. Il n'existe pas de programme de bug bounty rémunéré.

## 9. Activité de mes nœuds

- `polygone node status` expose l'état local du nœud de stockage : bail, fraîcheur, blocs/octets servis et connexions acceptées/refusées.
- Pas de tableau de bord global des messages, fichiers, disponibilité de tous les services ou réputation réseau. Les compteurs du nœud de stockage ne doivent pas être extrapolés aux autres services.

## 10. Accès aux services

- Le CLI unifié expose les fonctions effectivement présentes : messagerie, écoute relay, mesh, compute, node, Serverless prototype, Petals local, Hide mono-saut, duress manuel.
- Le site statique et l'API Gateway sont distincts du transport pair-à-pair ; leur intégration à un compte et à une application mobile commune reste à faire.

---

## Ordre de finalisation proposé depuis l'état réel

1. Corriger les incohérences d'identité et de confiance (notamment `polygone id`, identité réseau persistante, appairage réellement authentifié).
2. Définir un reçu de résultat signé utile, distinct du grant d'autorisation, et brancher sa vérification à la réputation locale avant toute réputation réseau.
3. Alimenter Gateway avec des événements/heartbeats réels et des mesures de latence, ou retirer toute prétention de statut global jusqu'à ce que le flux existe.
4. Définir le contrat d'orchestration Serverless : sélection des nœuds, transport, exécution, réponses, reçus, quorum, timeouts, retrait et coûts ; puis l'implémenter et le tester sur plusieurs processus.
5. N'engager Petals distribué qu'avec un protocole de transfert des poids et d'exécution inter-étapes testable ; le planner seul ne suffit pas.
6. Concevoir le mobile comme nœud séparément, avec consentement, contraintes OS/batterie/réseau et tests sur appareils réels.
7. Préparer un déploiement public uniquement après authentification, TLS, gestion des secrets, observabilité et modèle de menace revus.

*Les statuts et limites détaillés sont dans `AUDIT_VERACITE_VISION.md` et `STAGING.md`. Les objectifs futurs ne sont pas des capacités livrées.*
