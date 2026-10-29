# Polygone — Vision canonique

> Document de référence unique. Il a été rédigé uniquement à partir du bundle de
> synthèse déjà clusterisé (`bundle-polygone-v2`, 12 thèmes + vision + index).
> Rien n'a été inventé : quand une information n'existe pas dans le bundle, c'est dit
> explicitement. Les artefacts techniques (journaux, identifiants de session, compteurs)
> ont été retirés.

---

## 1. Vision en une page

**Ce qu'est Polygone.**
Polygone est un projet open source français de confidentialité et de sécurité.
Son cœur, *Polygone-Network*, est un réseau pair-à-pair chiffré de bout en bout et
résistant à l'informatique quantique. Le slogan d'origine : « Tes données sont à toi.
Personne ne doit les voir, les toucher ou même savoir qu'elles existent. » Polygone ne
se contente pas de chiffrer : il cherche à rendre la communication inexistante aux yeux
d'un observateur — pas de métadonnées, pas de serveur central, pas de point de collecte.
Autour de ce cœur devait se construire un écosystème de services (stockage, tunnel
anonyme, inférence d'IA distribuée, nœuds serveurs) accessibles depuis une seule
interface. La version la plus aboutie de cette promesse s'appelle **Polygone v2.0**,
présentée comme « la messagerie qui résiste aux ordinateurs quantiques ».

**Pour qui.**
D'abord le particulier — « sans friction, sans connaissances techniques », avec une
version par interface graphique *et* une version terminal pour les plus expérimentés.
Ensuite les profils à risque : journalistes sous surveillance, lanceurs d'alerte, et
« tous ceux qui en ont marre que leurs données valent de l'or sans leur consentement ».
Les entreprises ne viennent qu'après validation du grand public.

**Pourquoi maintenant.**
Parce que le chiffrement d'aujourd'hui sera lisible demain : « En 2030, ton ordinateur
quantique pourra lire tous tes messages d'aujourd'hui. Polygone chiffre pour 2030,
aujourd'hui. » Deuxième raison, plus personnelle et récurrente : le manque de puissance
et l'envie de rendre la vie privée et les ressources « gratuites pour tout le monde ».
Troisième raison : un objectif de reconnaissance et de partenariat — rendre le projet
assez solide pour être montré à Proton (ou un partenaire équivalent).

---

## 2. Principes non négociables

1. **Souveraineté totale des données** : personne ne doit voir, toucher ou même savoir
   que les données existent.
2. **Zéro métadonnée, zéro serveur central, zéro point de collecte** : chaque utilisateur
   est son propre nœud, et chaque nœud ne sait rien de ce qu'il transporte.
3. **Post-quantique par défaut** : le chiffrement doit résister aux ordinateurs
   quantiques, aujourd'hui et pour la décennie à venir.
4. **Une promesse du README vaut un test vérifiable** — jamais une troisième voie
   (les docs gouvernent le code, pas l'inverse).
5. **Ne jamais casser le cœur existant** (cryptographie et exécution du réseau).
6. **Aucune décision produit non tranchée n'est supposée** : elle est écrite et
   documentée avant d'être implémentée.
7. **Chaque étape finit par une vérification qui passe et une sauvegarde propre.**

---

## 3. Briques (architecture haut niveau)

> Le bundle décrit un écosystème historique de 8 modules (*Network, Drive, Hide, Petals,
> Server, Brain, Shell, CLI*) auquel se sont ajoutés des composants v2 (*Msg, Mesh,
> Computer, SDK*). La liste ci-dessous suit la structure demandée ; pour les briques que
> le bundle ne nomme pas, c'est indiqué honnêtement.

### Hide
- **Rôle** : tunnel réseau anonyme (proxy SOCKS5 local) pour faire passer le trafic sans
  être vu.
- **État actuel** : **codé** (un module Hide avec tunnel SOCKS5 existe dans les sources
  archivées) ; l'intégration au produit unifié est **spécifiée** mais pas terminée.

### Drive
- **Rôle** : stockage chiffré et décentralisé, avec fragmentation des données et accès
  par liens temporaires.
- **État actuel** : **codé partiellement** (le module de stockage et de réseau existe) ;
  la fragmentation, l'API de téléchargement et les liens à durée limitée sont
  **spécifiés** mais restaient sur la liste des tâches à faire.

### Photos
- **Rôle** : **absent du bundle.** Aucun module « Photos » n'est nommé dans les 12
  thèmes. La seule occurrence du mot « photos » concerne un carrousel d'images à usage
  visuel, sans rapport avec une brique produit.
- **État actuel** : **non documenté** — à retirer ou à confirmer (voir section 5).

### Petals
- **Rôle** : inférence d'IA distribuée — faire tourner des modèles sur le réseau
  Polygone plutôt que sur une seule machine.
- **État actuel** : **codé** sous forme de prototype (scripts d'inférence, d'entraînement
  et de passerelle pair-à-pair). Le protocole d'échange neuronal chiffré entre IA était
  **spécifié** et non terminé.

### Serverless (Polygone-Server / Polygone-Computer)
- **Rôle** : transformer un serveur ou un service en nœud Polygone — héberger un site ou
  un service sans navigateur spécial, avec compatibilité domaines et images conteneur ;
  « Polygone-Computer » pousse l'idée vers du calcul dans le nuage via le réseau.
- **État actuel** : **codé** pour la partie serveur (services persistants, conteneur,
  scripts de démarrage) ; le déploiement public a échoué à répétition en production.
  Polygone-Computer reste **concept** (« pas urgent »).

### Agents
- **Rôle** : permettre à des agents d'IA de se transférer un état, une mémoire et des
  tâches de façon standardisée et signée — l'équivalent de HTTP, mais pour l'échange
  d'état cognitif entre agents (protocole **COGNET**).
- **État actuel** : **spécifié** (spécification v0.1 rédigée) ; aucune implémentation
  livrée dans le bundle.

### API
- **Rôle** : exposer Polygone aux développeurs et aux outils externes — un SDK, un point
  d'accès compatible avec les API d'IA existantes, et une passerelle HTTP vers le réseau.
- **État actuel** : **spécifié** (création du SDK et d'un point d'accès compatible
  figuraient dans les tâches) ; la passerelle était **codée** mais simulait encore le
  réseau au lieu de l'interroger réellement.

### Daemon
- **Rôle** : le moteur local, léger et intelligent, qui mesure les ressources inutilisées
  de la machine et les met à disposition du réseau, avec une marge de sécurité.
- **État actuel** : **codé et testé en local** — le démon répond aux commandes de statut
  et de simulation, avec une politique d'allocation progressive (« Glow-Up ») à plusieurs
  niveaux, dont un niveau recommandé activable par l'utilisateur.

**État général honnête** : le cœur cryptographique est le plus avancé ; le produit
utilisable de bout en bout — interface, démonstration, installation en une commande —
n'est pas encore livré. C'est le constat répété dans les thèmes.

---

## 4. Décisions déjà actées

- **Public visé →** le particulier d'abord, l'entreprise après validation.
- **But final →** un projet assez solide pour être présenté à Proton ou un partenaire.
- **Langage →** Rust, organisé en espace de travail multi-modules (et non un seul bloc).
- **Structure →** séparer les responsabilités en modules spécialisés (types partagés,
  cryptographie, réseau, interface), pour isoler les dépendances et accélérer les tests.
- **Cryptographie →** primitives post-quantiques (échange de clés post-quantique,
  chiffrement symétrique fort) plus un découpage de type Shamir pour fragmenter.
- **Format de fragmentation →** un découpage de référence de type « 4 parts sur 7 ».
- **Purge d'architecture →** supprimer tout ce qui a rendu le projet lourd (interface
  web, passerelle, service de santé séparé, images et scripts de déploiement) et repartir
  du cœur, en gardant la cryptographie intacte.
- **Expérience utilisateur →** une seule commande (`polygone`) plus un menu navigable aux
  flèches, des versions Windows, Linux et macOS, un logiciel et une interface accessibles
  aux non-techniciens, et une version terminal pour les experts.
- **Communication →** retirer la mention « Enterprise Ready » du manifeste ; pas de
  manifeste exagéré ; un message sobre vers un partenaire.
- **Nom des versions →** la refonte aboutie porte le nom **Polygone v2.0**.
- **Discours produit →** se présenter comme un produit, pas comme un projet en chantier.
- **Auto-destruction des messages →** durée par défaut de 30 secondes.
- **Licence →** ouverte, de type MIT (mentionnée dans les sources archivées).
- **Réseau →** ne pas se baser sur la position géographique des nœuds ; préférer un
  mécanisme plus complexe et simulé, de type résolveur privé.
- **Synchronisation entre nœuds →** un algorithme de synchronisation « sans temps »
  (approches de type CRDT et pondération par capacité) est la piste retenue.
- **Polygone-Brain →** à annuler, car Petals ferait déjà le travail (décision de
  l'utilisateur, mais contredite ailleurs — voir section 5).
- **Interface v2.0 →** une application de bureau, un tableau de bord web, des ponts vers
  des messageries existantes (Proton, Matrix, courriel), un SDK, une interface terminal
  et une page d'accueil publique.
- **Vitrine →** http://localhost:9050 comme adresse d'accueil du tableau de bord après
  démarrage.
- **Méthode de travail →** toute promesse vérifiable, les documents font autorité, et
  chaque phase se termine par une vérification verte et une sauvegarde propre.

---

## 5. Contradictions à trancher

1. **Module Brain →** le garder (listé comme module de l'écosystème dans deux thèmes et
   dans la structure de dépôt) **vs** le supprimer (décision explicite de l'utilisateur :
   « Petals le fait déjà »).
2. **Serveur →** « pas de serveur central, pas de point de collecte » (principe fondateur)
   **vs** le déploiement de nœuds persistants et d'une flotte de serveurs publics qui
   deviennent eux-mêmes des nœuds du réseau.
3. **Organisation du code →** un seul dépôt unifié (monorepo) **vs** plusieurs dépôts
   spécialisés : la question a été posée à l'utilisateur et n'a jamais été tranchée.
4. **Feuille de route →** deux plans coexistent : un plan « produit++ » en phases
   (vérité des docs, honnêteté du transport, sécurité, sortie, synthèse, graphe, CI)
   **vs** un plan de développement en phases numérotées (espace de travail, interface
   terminal, messagerie et stockage, maillage local, cerveau, installateur).
5. **Brique « Photos » →** jamais nommée dans le bundle **vs** présente dans la liste de
   briques attendue : soit elle a une autre source, soit il faut la retirer.
6. **Briques « Serverless », « Agents », « API » →** absentes en tant que briques
   nommées **vs** présence de concepts équivalents (serveur transformé en nœud,
   protocole d'échange entre agents, SDK et point d'accès) : à clarifier pour éviter les
   doublons de vocabulaire.
7. **Modèle économique →** distribuer un jeton « POLY » pour louer de la puissance
   (mémoire et calcul) entre machines **vs** la réticence de l'utilisateur, qui craint que
   cela freine l'intégration du projet par des entreprises et « casse tout ».
8. **Nombre de modules de l'écosystème →** huit modules historiques (Network, Drive, Hide,
   Petals, Server, Brain, Shell, CLI) **vs** une liste plus large incluant messagerie,
   maillage, ordinateur dans le nuage et SDK.
9. **Maturité déclarée →** un discours de sortie « urgente » **vs** le constat, écrit noir
   sur blanc dans le bundle, que le projet « n'est pas prêt du tout », « pas intuitif »,
   « sans interface » et que l'écosystème « n'est pas fonctionnel ».

---

## 6. Prochaine étape recommandée

**Recommandation : finaliser le Daemon (allocation dynamique des ressources).**

C'est la seule brique pour laquelle une consigne explicite d'ordre de départ existe :
« il faut tout refaire en reprenant ce qu'on avait déjà, crée un démon dynamique, hyper
léger et intelligent qui alloue les ressources inutilisées de sa machine, avec une petite
marge de sécurité, et les alloue au réseau — commence par ça proprement ». Elle est déjà
la plus avancée de tout l'écosystème : elle répond en local, affiche son état et sait
simuler son comportement. Elle est aussi la **dépendance de tout le reste** : stockage
distribué, inférence d'IA distribuée et calcul dans le nuage n'existent que si la
puissance des machines est d'abord captée, plafonnée et redistribuée proprement. Enfin,
elle est **vérifiable immédiatement** sur la machine de développement, ce qui respecte le
principe « chaque étape finit par une vérification qui passe ».

**Candidat alternatif, tout aussi soutenu par le bundle** : le **binaire unique
`polygone`** (une commande, un menu aux flèches, un installateur en une ligne). C'est la
brique qui répond au reproche le plus répété — « pas intuitif, pas d'interface,
accessible à tous » — et sans elle aucune autre brique n'est montrable à un partenaire.

Les deux se complètent : le Daemon donne la puissance, le binaire unique donne le
produit. Si une seule doit être choisie, commencer par le Daemon, puis l'envelopper
immédiatement dans le binaire unique afin d'obtenir une première démonstration de bout
en bout.

---

*Sources : `00-VISION.md`, `INDEX.md`, `cluster-00` à `cluster-11` (bundle déjà
clusterisé, 327 sessions). Aucune affirmation de ce document ne vient d'ailleurs.*
