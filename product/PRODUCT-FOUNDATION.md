# PRODUCT-FOUNDATION — ce que Polygone peut être construit dessus

> Date : **2026-11-09**, HEAD `e2936a3` + correctif de persistance du ledger.
> Ce document décrit le produit **réellement supporté par le moteur**, pas la
> vision complète. La vision est dans `VISION.md`/`ECOSYSTEM.md` ; les preuves
> sont dans `project/FOUNDATION-STATUS.md`.

---

## 1. Public visé

| Persona | Besoin | Ce que le socle permet aujourd'hui |
|---|---|---|
| propriétaire d'une machine | partager une partie de ses ressources sans perdre le contrôle | choisir un profil, mettre en pause, refuser, inspecter l'état, désinstaller |
| utilisateur non technique | voir si son appareil participe, combien, à quoi | lecture de l'état et des ressources via CLI/PWA ; pas d'écran guidé complet |
| développeur | construire une application sur le moteur | API locale daemon + gateway ; pas encore d'API tâches/événements |
| opérateur de nœud | diagnostiquer un problème | `doctor`, `verite`, `ecosysteme`, compteurs de refus |
| chercheur / pair | échanger un message chiffré | identité, message hors ligne ou via relais |

**Non public aujourd'hui** : quelqu'un qui veut louer du calcul ou lancer une
inférence distribuée sans lire la documentation. Le moteur ne fournit pas encore
ce parcours de bout en bout.

---

## 2. Promesse produit autorisée

> « Polygone mesure les ressources de votre machine, vous laisse fixer ce que
> vous acceptez de partager, et permet à des appareils de s'échanger des
> messages chiffrés et de confier un calcul borné à un pair, via un relais qui
> ne lit pas le contenu. »

Ce qui n'est pas autorisé :

- « réseau décentralisé validé » (aucun test entre deux machines physiques) ;
- « anonymat » ou « comme Tor » (le relais voit les métadonnées) ;
- « calcul distribué » au sens d'un moteur de tâches complet ;
- « limite automatiquement les ressources sur toutes les plateformes » ;
- « IA distribuée » (le pipeline Petals n'est pas câblé) ;
- « disponible sur Windows ».

---

## 3. Les six espaces produit (hypothèse de travail)

Cette organisation est une **hypothèse fonctionnelle**, pas une obligation
d'architecture. Chaque espace est adossé à des données réelles ou marqué
bloqué dans [`UI-API-MAPPING.md`](./UI-API-MAPPING.md).

| Espace | Question de l'utilisateur | État des données |
|---|---|---|
| **Overview** | « Est-ce que ça marche, et qu'est-ce que je partage ? » | partiel : état nœud + ressources + pairs |
| **Network** | « À quels appareils suis-je connecté ? » | partiel : nœuds appairés, pairs mesh ; pas de graphe live |
| **My Node** | « Que fait mon appareil, et comment le limiter ? » | bon : identité, ressources, profil, pause |
| **Compute** | « Quelles tâches tournent, et quels résultats ? » | **bloqué** : pas d'API de tâches ni d'historique |
| **Activity** | « Que s'est-il passé, qu'est-ce que ça a consommé ? » | **bloqué** : pas d'événements ni d'historique de consommation |
| **Settings** | « Identité, sécurité, permissions, préférences » | partiel : identité et config fichier ; pas d'API settings |

---

## 4. Parcours utilisateur réels

| # | Parcours | Ce qui existe | Ce qui manque |
|---:|---|---|---|
| 1 | Première installation | `install.sh`, service utilisateur, `polygone doctor` | test sur VM neuve ; écran guidé |
| 2 | Création / récupération d'identité | création automatique, `clef`, `id`, `clef-tourner` | restauration depuis sauvegarde ; export/import guidé |
| 3 | Initialisation du nœud | `polygone node servir`, bail du daemon | assistant pas à pas |
| 4 | Configuration du partage | `polygone profil sobre|equilibre|genereux`, config TOML | écran de limites par ressource ; plafond CPU par palier non appliqué |
| 5 | Connexion au réseau | `ecouter --relay`, `annoncer`, `voisins` | découverte inter-réseaux ; reconnexion prouvée |
| 6 | Consultation de l'état | `polygone doctor`, socket `status`, gateway `/api/v1/network/status` | vue agrégée UI |
| 7 | Suspension du partage | `polygone pause --duree` | bouton UI ; l'intention ne survit pas au reboot (conçu ainsi) |
| 8 | Reprise | `polygone reprendre` | idem |
| 9 | Consultation des tâches | **rien** | modèle de tâche, API, historique |
| 10 | Consultation de l'activité | compteurs de refus, `verite` | événements, consommation, journal exploitable |
| 11 | Résolution d'un problème | `doctor`, `ecosysteme`, codes de refus | rapport exportable, métriques |
| 12 | Retrait du réseau | `duress`, `desinstaller` | révocation publique d'identité |

---

## 5. Capacités réellement supportées par le moteur

### 5.1 Solides

- Identité post-quantique persistante, rotation de clé de signature.
- Message chiffré et signé, hors ligne ou via relais.
- Relais authentifié, anti-usurpation, anti-rejeu, limites de débit.
- Réservations de ressources cohérentes, persistantes, avec expiration.
- Mesure réelle CPU/RAM/stockage/GPU, sans invention.
- Diagnostic local qui distingue « non mesuré » de « zéro ».
- Pause, retrait, désinstallation sans résidu.
- Gateway locale d'appairage et de battement de cœur signé.

### 5.2 Partielles

- Réseau : prouvé sur loopback, pas entre hôtes.
- Exécution : un nœud peut exécuter un calcul borné ; pas de placement ni de
  cycle de tâche complet.
- Isolation : réelle sous Linux avec `--isoler`, absente ailleurs.
- PWA : installable, mais non testée sur appareils réels.
- Hide : mono-saut.

### 5.3 Absentes

- API de tâches, d'historique, de consommation, d'événements.
- Métriques et rapport de diagnostic exportable.
- Chiffrement au repos.
- Mise à jour signée et rollback.
- Pipeline d'inférence distribué.
- Quorum serverless réel.
- Support Windows.

---

## 6. Parcours minimum pour une première application

Une application peut être construite **maintenant** si elle se limite à :

1. lire l'état du nœud et les ressources mesurées ;
2. afficher l'identité et les pairs connus ;
3. changer de profil, mettre en pause, reprendre ;
4. lancer un diagnostic ;
5. afficher les refus d'allocation par motif ;
6. ne jamais afficher de valeur non fournie par une API.

Tout écran de tâches, d'historique ou de consommation doit attendre les contrats
correspondants. Voir `PRODUCT-READINESS.md`.

---

## 7. Non-objectifs immédiats

Marketplace, paiement, token, gouvernance, catalogue de modèles, intégrations
multiples. L'architecture doit rester extensible ; ces systèmes ne doivent pas
être construits avant que le socle de tâches et d'API soit stable.

---

## 8. Critère de réussite du produit (rappel)

Une personne non ingénieure doit pouvoir : installer, créer son identité,
constater ce qu'elle partage, limiter ou suspendre, voir à quels appareils elle
est connectée, et diagnostiquer un problème — **sans manipuler de fichier
interne ni connaître un socket**. Aujourd'hui, les parcours 1 à 8 existent en
CLI ; les parcours 9 à 11 n'ont pas de moteur ; aucun écran graphique complet
n'existe.
