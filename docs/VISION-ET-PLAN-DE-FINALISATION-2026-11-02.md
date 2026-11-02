# Polygone — vision consolidée, dilemmes et plan de finalisation

**État du dossier :** analyse de reprise du 2 novembre 2026  
**Sources examinées :** extraction dédiée `Downloads/Data Graph/extract-polygone`, base historique `state.db.bak` (inventoriée en lecture seule dans le rapport de reprise), dépôt local `Projects/Polygone-real`, documentation et code du workspace.  
**Règle :** ce document distingue les demandes de l'utilisateur, les décisions réellement actées, les propositions d'agents et les faits vérifiés dans le code.

---

## 0. Synthèse exécutive

Polygone est un projet open source dont l'ambition est de construire une **couche de confidentialité et de coopération informatique décentralisée**, utilisable par des personnes ordinaires et extensible par des développeurs. Ce n'est ni seulement une messagerie chiffrée, ni un simple gestionnaire de ressources, ni une collection de prototypes indépendants.

La vision de long terme comporte deux axes qui doivent être construits dans le bon ordre :

1. **Souveraineté et confidentialité :** identité, communications, stockage et services sans dépendance obligatoire à une plateforme centralisée qui collecte les données.
2. **Réseau de ressources :** permettre à des appareils volontaires de mettre à disposition une partie réellement inutilisée de leurs capacités, de manière réversible, mesurée et sûre, afin de fournir du stockage, des relais et des tâches de calcul distribuées.

Les services comme Hide, Drive, Serverless, Petals, SDK et les intégrations Proton/Matrix/Signal/Telegram sont des usages de cette fondation. Le projet doit rester extensible, mais **ne doit pas essayer de livrer tous ces services avant d'avoir prouvé le socle réseau et le contrôle des ressources**.

Le défaut historique le plus coûteux n'a pas été un manque d'idées : c'est l'écart entre les promesses, le code réellement branché et les preuves de fonctionnement. La règle de livraison devient donc :

> Une fonctionnalité n'est « livrée » que si son chemin de bout en bout fonctionne dans un scénario reproductible, si ses limites sont explicites et si une preuve automatisée ou une démonstration indépendante existe.

---

## 1. Ce que l'historique établit

L'extraction dédiée contient :

- `messages.jsonl` : **32 923 messages** (3 936 utilisateur, 6 897 assistant, 22 090 outils) ;
- `sessions.jsonl` : **3 696 sessions**, dont 3 044 marquées `has_polygone` ;
- `sessions.csv` : index de sessions ;
- période observée : avril à août 2026 ; tags principaux : `polygone` et `reseau`.

L'extraction est déjà filtrée et n'est pas la totalité de la base SQLite. Elle a été utilisée comme corpus historique, sans recopier les conversations brutes dans le dépôt.

### Évolution de la vision

- **Avril–mai :** construire un produit réel, simple, accessible, autour d'un réseau pair-à-pair et de primitives de confidentialité post-quantiques. L'utilisateur insiste explicitement sur le fait que Polygone doit être un produit utilisable, pas une démonstration technique.
- **Mai :** apparition de l'écosystème de services : Msg, Hide, Drive, Mesh, Brain/Petals, Gateway/Server. L'interface unifiée et une expérience non technique sont déjà des attentes.
- **Juin :** frustration explicite face aux tâches déclarées terminées sans résultat concret. Les automatismes, cron jobs et plans de travail ne valent rien s'ils n'aboutissent pas à un artefact ou à une preuve.
- **Juin–juillet :** repositionnement vers une couche de confidentialité universelle et un écosystème de protocoles. Le projet doit également servir de base à un nouveau système de calcul, puis potentiellement à un OS dont ce moteur serait le socle.
- **18 juillet :** demande détaillée de synchronisation des nœuds, optimisation multiplateforme, Glow-Up recommandé, choix de ressources dans les paramètres, contribution contrôlée, Petals distribué, serverless décentralisé, domaines, et intégrations stratégiques.
- **20 juillet :** demande de poursuivre le polissage de manière autonome, avec sauvegarde restaurable et prudence absolue sur les données et les opérations incertaines.
- **23 juillet :** demande de pousser les changements locaux sur GitHub, puis de reprendre le travail. Cela ne dispense pas de réconcilier les branches avant toute opération distante.
- **7–9 août :** demande de finaliser Polygone, d'assainir le README, de faire des recherches sérieuses et de ne pas produire un discours artificiel ou rempli de promesses. La priorité est une version finale crédible.

### Conséquence

La vision a grandi, mais les demandes les plus récentes ne remplacent pas les principes fondateurs. Il faut **conserver l'ambition dans l'architecture et la roadmap, et limiter le périmètre de chaque livraison**.

---

## 2. Vision produit consolidée

### 2.1 Promesse centrale

Polygone doit permettre à chacun de reprendre le contrôle de ses données et de contribuer au réseau selon ses propres limites. Un utilisateur doit pouvoir installer l'application, comprendre ce qu'elle fait, choisir ce qu'il partage, l'interrompre et vérifier ce qui a réellement été utilisé.

La confidentialité doit être une propriété de conception et non une phrase marketing. Polygone doit réduire la collecte, limiter les métadonnées, éviter les dépendances centrales obligatoires et documenter honnêtement les informations qu'un pair, un relais, un fournisseur d'accès ou un observateur réseau peut encore voir.

### 2.2 Les couches

1. **Core / identité / cryptographie :** identités, clés, chiffrement authentifié, signatures, format de messages, expiration et sérialisation versionnée.
2. **Transport / réseau :** connexion entre pairs, découverte, relais facultatifs, routage, contrôle de flux, gestion des déconnexions et authentification des pairs.
3. **Daemon / ressources :** mesure des capacités, politique d'allocation, consentement, réservation, isolation des charges, arrêt et restitution.
4. **Blocs / stockage :** données adressées par contenu, vérification d'intégrité, réplication, quotas, durée de conservation et disponibilité.
5. **Services :** messagerie, Hide, Drive, Serverless et Petals. Chacun doit avoir son modèle de menace et ses critères d'acceptation.
6. **Produit :** installateur, commande unique, interface simple, paramètres compréhensibles, mises à jour sûres, diagnostics et documentation.
7. **Écosystème développeur :** SDK, API et ponts vers des services existants, après stabilisation du protocole.

### 2.3 Utilisateur cible et expérience

Le premier utilisateur est un particulier qui ne veut pas apprendre Rust, libp2p ou la cryptographie pour utiliser le produit. L'application doit expliquer les choix sans jargon, proposer des réglages sûrs par défaut et rendre visibles les états importants : connecté, en pause, ressources partagées, charge active, consommation, erreurs et dernière preuve de fonctionnement.

Une interface terminal peut rester utile aux développeurs et au diagnostic, mais elle ne doit pas être la seule expérience proposée à un public non technique. Les anciennes propositions TUI, Tauri, dashboard web et PWA ne sont pas toutes des décisions compatibles à appliquer simultanément : le produit doit avoir une interface de référence, puis des clients secondaires qui utilisent la même API stable.

---

## 3. Critères non négociables

### Confidentialité et sécurité

- Pas de compte ou cloud obligatoire pour les fonctions locales et pair-à-pair de base.
- Minimisation des métadonnées, rétention limitée et absence de télémétrie non consentie.
- Chiffrement de bout en bout et authentification des pairs ; les relais ne doivent pas pouvoir lire le contenu.
- Les relais, points d'entrée, serveurs bootstrap et passerelles publiques restent des observateurs possibles : leur rôle et leurs journaux doivent être bornés et documentés.
- Aucun mot « anonyme », « zéro métadonnée », « impossible à tracer », « résistant à tout » ou « plus sûr que Tor » sans définition de menace et preuve correspondante.
- Les primitives cryptographiques doivent être standardisées, maintenues, testées sur vecteurs connus et enveloppées dans des formats versionnés. Une primitive PQ seule ne rend pas l'ensemble du protocole post-quantique si l'identité, les signatures ou les chemins de négociation restent classiques.
- Le seuil Shamir 4-sur-7 est un mécanisme de partage de secret, pas une garantie d'anonymat, de routage indépendant ou de disponibilité. Le sens exact du secret partagé et le placement des fragments doivent être définis dans le protocole.
- TTL et effacement doivent être décrits avec précision : expiration logique, suppression des buffers, effacement mémoire best-effort et limites des SSD/swap/crash dumps ne sont pas équivalents.

### Ressources et autonomie du propriétaire

- Contribution entièrement volontaire, désactivée par défaut jusqu'au consentement explicite.
- Plafonds configurables et valeurs sûres recommandées ; ne jamais utiliser « toutes les ressources libres » comme raccourci sans marge mesurée.
- Mesure dynamique de CPU, mémoire disponible, stockage, réseau, batterie, température et pression système lorsqu'une source fiable existe.
- Si une mesure est inconnue, la politique doit être conservatrice. `Unknown` ne signifie ni « inactif » ni « sûr ».
- Pause et retrait doivent être rapides, idempotents et testés en cas de crash, perte réseau, veille et arrêt brutal.
- Les charges distribuées doivent tourner dans une isolation dédiée, jamais en modifiant le cgroup de la session graphique ou d'un parent que Polygone ne possède pas.
- Le comportement capot fermé, sur batterie, en veille et à la reprise doit être explicite, configurable et conforme aux possibilités réelles de chaque OS.
- Aucune charge distante ne doit pouvoir provoquer une saturation durable, lire les fichiers de l'utilisateur ou accéder aux secrets locaux.

### Qualité produit

- Une installation propre et une désinstallation propre ; mises à jour vérifiées, réversibles et sans scripts opaques.
- Pas de bouton, option ou réglage sans effet réel testé.
- Pas de statistiques inventées, de valeurs en dur présentées comme mesures, de mocks en production ou de faux états « connecté ».
- Une documentation courte pour l'utilisateur, une documentation technique détaillée pour les contributeurs.
- Le produit doit rester léger, réactif et compatible avec des machines modestes ; éviter les sous-agents et services permanents qui consomment plus de ressources que le produit lui-même.
- Chaque modification doit être petite, traçable, sauvegardée et vérifiée. Pas de `git reset --hard`, `git clean`, force-push ou remplacement global de fichiers pour gagner du temps.

---

## 4. Dilemmes et arbitrages à traiter explicitement

| Dilemme | Risque si on ignore le problème | Arbitrage de travail |
|---|---|---|
| Décentralisation vs relais | Un relais public peut devenir un point d'observation ou de blocage. | Relais facultatifs, multiples et remplaçables ; modèle de menace explicite ; pas de prétention « sans point d'observation ». |
| Confidentialité vs découverte | La découverte de pairs révèle potentiellement présence, adresse IP et horaires. | Découverte locale/privée par défaut ; bootstrap séparé et facultatif ; documenter ce qui fuit. |
| Post-quantique vs compatibilité | Mélanger signatures classiques et KEM PQ peut invalider une promesse globale. | Définir une suite crypto complète, versionnée, avec migration et tests d'interopérabilité. |
| Shamir vs fragmentation réseau | 4/7 ne garantit pas que les parts empruntent des chemins indépendants. | Séparer le partage cryptographique du routage ; mesurer la diversité réelle des opérateurs/chemins. |
| Éphémère vs fiabilité | Un message qui disparaît vite peut ne pas être livré à un pair hors ligne. | Séparer délai de livraison, durée de conservation et effacement ; choix explicite par service. |
| Ressources partagées vs confort local | Une allocation calculée n'est pas une limite appliquée par le système. | Ne compter une ressource comme « allouée » qu'après isolation et contrôle OS réellement actifs. |
| CPU/RAM distants vs latence | Une RAM distante ne peut pas remplacer la RAM locale ; la latence réseau est trop élevée. | Partager des blocs immuables, des tâches batch et des lots d'inférence, pas une mémoire transparente. |
| Synchronisation rapide vs physique | Tous les nœuds ne peuvent pas avoir le même temps de réponse à travers Internet. | Synchroniser état/protocole et choisir les pairs par classe de latence ; ne pas promettre une latence uniforme. |
| Petals distribué vs débit réseau | Le découpage d'un modèle peut coûter plus cher en transfert que le calcul gagné. | Commencer par LAN haut débit et modèles compatibles ; batch/quantification ; mesurer avant d'élargir. |
| Serverless décentralisé vs domaine public | Un domaine public a besoin d'un point d'entrée DNS/HTTP et expose disponibilité et métadonnées. | Séparer hébergement P2P, passerelle publique et DNS ; expliquer les compromis et les coûts. |
| Hide vs Tor | « Mieux que Tor » est une promesse extraordinairement forte et non démontrée. | Construire d'abord un tunnel honnête ; ne pas prétendre remplacer Tor sans analyse indépendante et réseau multi-hop réel. |
| Écosystème vs livraison | Trop de modules simultanés crée des interfaces mortes et des prototypes éternels. | Conserver les modules dans l'architecture, mais livrer dans l'ordre des dépendances et avec des critères de sortie. |
| TUI vs application graphique | Une TUI peut être excellente pour les experts mais insuffisante pour l'utilisateur ordinaire. | CLI/TUI pour administration ; interface graphique de référence quand le backend et les parcours sont stables. |
| Gratuité vs économie POLY | Un jeton prématuré ajoute complexité, risques réglementaires et friction d'adoption. | Pas de token/marketplace tant qu'un service mesurable n'a pas été rendu et qu'un modèle n'est pas nécessaire. |
| MIT vs AGPL | Les dépôts et documents historiques divergent sur la licence. | Ne pas changer silencieusement ; inventorier les dépendances et demander une décision propriétaire avant publication finale. |
| Monorepo vs dépôts spécialisés | Plusieurs dépôts peuvent fragmenter la source de vérité ; un monorepo peut devenir trop couplé. | Garder le workspace actuel comme source de vérité de la version réseau ; extraire un protocole seulement si ses frontières sont stables. |

Ces arbitrages sont des décisions d'ingénierie proposées pour guider le travail. Ils ne doivent pas être présentés comme des décisions juridiques ou commerciales déjà validées par l'utilisateur.

---

## 5. État technique vérifié au 2 novembre 2026

### Dépôt

- Dépôt local canonique utilisé pour cette reprise : `/home/lvs0/Projects/Polygone-real`.
- Branche : `main`, HEAD local `f3d09c0`.
- Remote : `origin` → `lvs0/Polygone-Network`.
- Écart constaté : **6 commits locaux non présents sur `origin/main`, 17 commits distants absents du HEAD local**.
- Le dépôt comporte des modifications non commitées préexistantes. Elles ne doivent pas être écrasées.
- Aucun merge, reset, clean, commit ou push n'a été réalisé pendant cette analyse.

### Vérifications déjà réalisées sur cet état local

Le contrôle précédent du 2 novembre a donné : `cargo fmt --all -- --check` OK, `cargo test --workspace` OK (**266 tests**), `cargo clippy --workspace --all-targets -- -D warnings` OK, `git diff --check` OK. Ces résultats sont des contrôles de compilation/tests unitaires sur cette machine ; ils ne prouvent pas le fonctionnement entre deux machines physiques ni la compatibilité Windows/macOS.

### Réalité du daemon et du Glow-Up

- Le daemon capture des snapshots, calcule une politique, lisse les allocations et publie un état.
- `GlowUpEngine::apply()` applique actuellement la priorité CPU via `nice`. Il ne pose pas de quota CPU effectif, n'applique pas d'affinité au processus consommateur et n'impose pas de limite mémoire à une charge isolée.
- Le code CPU dispose de fonctions d'allocation et d'affinité Linux, mais leur présence ne signifie pas qu'elles sont branchées au cycle de vie d'un consommateur.
- La bande passante est mesurée/estimée ; le plafond calculé n'est pas un shaping réseau effectif.
- La détection d'activité Linux retourne `Unknown` faute de source fiable reliée à la bonne session graphique. C'est préférable à une fausse détection d'inactivité.
- Le client `node servir` obtient un bail du daemon et s'arrête quand le bail est perdu ou rendu. C'est une vraie étape d'intégration, mais un bail n'est pas à lui seul une isolation OS des ressources.
- Le ledger gère des réservations de ressources et possède des tests de persistance. Il faut auditer le comportement réel en cas de crash et de redémarrage avant d'en faire une garantie de quota durable.
- Le stockage annoncé dans l'état du daemon reste indisponible tant qu'une mesure fiable et une réservation cohérente ne sont pas branchées.

**Conclusion :** le daemon est une base de politique et de réservation utile, mais le « partage dynamique CPU/RAM/GPU/réseau » n'est pas encore un mécanisme complet de limitation et d'isolation pour des charges arbitraires. Le README ne doit pas confondre capacité calculée, bail accordé et ressource effectivement contrainte.

### Réseau et stockage

- Le relay TCP transmet des enveloppes de fragments vers des pairs enregistrés et possède des tests de routage. Cela ne prouve pas l'anonymat, la résistance à l'analyse de trafic, ni une topologie multi-hop.
- Le magasin de blocs sait stocker et servir un bloc par hash, vérifier l'intégrité côté client et rejeter un contenu altéré. Les tests décrits dans les documents sont essentiellement des tests de processus locaux.
- Le test entre **deux machines physiques** (LAN réel, découverte, pare-feu, authentification, reprise) est toujours identifié comme manquant dans les documents de reprise.
- Le hash prouve que les octets reçus correspondent au hash demandé ; il ne prouve pas que le contenu est vrai, utile ou sûr.

### Gateway, Serverless et Petals

- Gateway a reçu une implémentation HTTP et des tests de routes/fichiers/PWA. Il faut encore vérifier que chaque endpoint expose des mesures réelles et que les services qu'il représente sont branchés au réseau.
- Serverless possède une spécification prudente et un module client, mais le dispatch distribué réel et le cycle de vie de bout en bout restent à prouver.
- Petals possède un planner/pipeline, mais la distribution effective des poids, le transport, la synchronisation et le bénéfice mesuré sur un cluster réel ne sont pas démontrés.
- Hide possède un chemin SOCKS5 single-hop décrit comme livré ; le multi-hop n'est pas implémenté. Ce n'est pas un équivalent démontré de Tor.

### Documentation à corriger

Des documents plus anciens contiennent des affirmations devenues fausses. En particulier, `daemon/src/runtime.rs` contient encore un commentaire qui évoque une limite mémoire appliquée par `GlowUpEngine::apply()`, alors que le code actuel n'applique que `nice`. `docs/DAEMON_STATE.md` doit être confronté au code courant avant d'être utilisé comme état de référence. Le README actuel est déjà plus honnête sur plusieurs limites, mais certaines commandes et affirmations doivent encore être exécutées dans une installation propre.

---

## 6. Ordre de finalisation recommandé

### Phase 0 — Préserver et réconcilier (obligatoire)

1. Sauvegarder l'état complet du dépôt, y compris les modifications non commitées et les fichiers non suivis.
2. Comparer les 17 commits distants et les 6 commits locaux un par un, sans merge automatique.
3. Établir un arbre de vérité : quels fichiers sont la source actuelle, lesquels sont obsolètes, lesquels doivent être repris sélectivement.
4. Faire tourner la CI locale sur l'état exact avant toute intégration.
5. Ne rien pousser tant que la divergence et les effets de bord ne sont pas compris.

**Sortie :** un arbre de travail sauvegardé, un plan de réconciliation documenté, aucun changement perdu.

### Phase 1 — Contrat de produit et de sécurité

1. Fixer le périmètre exact de la première version réellement publiable.
2. Définir le threat model : adversaires, observateurs, relais, pairs compromis, réseau local, machine hôte et limites physiques.
3. Auditer la composition cryptographique, les formats wire, les nonces, les signatures, les rejeux, les TTL et la gestion des clés.
4. Corriger la documentation et les promesses avant d'ajouter des fonctions.
5. Décider la licence (MIT historique vs AGPL actuelle) avant une release.

**Sortie :** un protocole et une promesse cohérents, avec un tableau « prouvé / partiel / absent ».

### Phase 2 — Daemon et sécurité des ressources

1. Définir le contrat de bail : ressources, durée, heartbeat, expiration, révocation, crash, reprise et idempotence.
2. Séparer la politique d'allocation de l'application OS.
3. Créer un consommateur isolé dédié, contrôlé par systemd/cgroup ou mécanisme équivalent ; ne jamais limiter le cgroup du terminal ou du bureau.
4. Prouver CPU et RAM réellement plafonnés sur Linux ; définir des capacités explicites et des modes conservateurs sur macOS et Windows.
5. Tester pause/retrait immédiat, perte du daemon, kill du consommateur, pression mémoire, batterie, veille et redémarrage.
6. Brancher ensuite stockage et bande passante, chacun avec quota et mécanisme d'application réellement testés.

**Sortie :** une charge de test est limitée par l'OS, le propriétaire garde la main et la restitution est prouvée.

### Phase 3 — Premier vrai réseau multi-machine

1. Définir identité, authentification des pairs, transport chiffré, découverte et protocole versionné.
2. Faire communiquer deux appareils physiques sur un LAN, puis sur deux réseaux distincts.
3. Tester perte de paquets, latence, NAT, reconnexion, pair malveillant, replay, enveloppes trop grandes et épuisement de ressources.
4. Faire transiter un bloc et un message réels ; vérifier intégrité, confidentialité et comportement de suppression.
5. Ajouter les mesures de latence réelles et la sélection de pairs par classe de réseau.

**Sortie :** un test E2E reproductible entre deux machines, sans dépendre d'un seul processus local.

### Phase 4 — Premier parcours utilisateur

1. Installation propre sur Linux, Windows et macOS (ou afficher explicitement les plateformes non supportées).
2. Première exécution guidée, création d'identité, appairage de deux nœuds, envoi/réception, pause et désinstallation.
3. Interface de référence simple ; paramètres de contribution compréhensibles, profil recommandé et plafonds.
4. Diagnostics actionnables, mises à jour signées et rollback.
5. Tester sur une machine modeste et un réseau domestique réel.

**Sortie :** une personne non technique peut installer, connecter deux appareils, échanger un message et contrôler la contribution sans terminal avancé.

### Phase 5 — Services additionnels

Ordre recommandé par dépendances :

1. **Msg** : produit de communication E2E fiable et éphémère.
2. **Drive / bloc store** : stockage adressé par hash, quotas, réplication et disponibilité.
3. **Hide** : tunnel single-hop robuste, puis multi-hop seulement après analyse de menace et transport adapté.
4. **Serverless** : hébergement de services/blocs, passerelles publiques optionnelles, domaines et limites de disponibilité.
5. **Petals** : inférence distribuée sur LAN, puis lots sur réseau étendu si les mesures justifient le transfert.
6. **SDK / bridges** : API stable, intégrations Proton/Matrix et autres, avec consentement et compatibilité explicites.

### Phase 6 — Durcissement et publication

- Audit de sécurité indépendant, fuzzing des parsers et tests de protocoles.
- CI Linux/Windows/macOS, builds reproductibles, SBOM, provenance et signatures de release.
- Documentation d'installation, guide utilisateur, threat model, politique de divulgation et procédure de mise à jour.
- Benchmarks avec matériel, conditions et intervalles clairement indiqués.
- Release uniquement après vérification des scénarios de sortie.

---

## 7. Critères de sortie transversaux

Aucune phase ne passe à « terminée » sans :

- tests unitaires et d'intégration pertinents ;
- tests négatifs (refus, corruption, perte de pair, dépassement de quota) ;
- preuve du comportement observable sur le système réel ;
- tests de crash/restart et de nettoyage ;
- documentation alignée sur le code ;
- résultats reproductibles avec commandes, OS et environnement ;
- sauvegarde restaurable ;
- état Git propre ou liste explicite des changements conservés.

Les tests locaux sur deux processus ne remplacent pas un test entre deux machines. Un hash valide ne remplace pas une preuve de justesse. Un bail ne remplace pas une limite OS. Une compilation Windows ne remplace pas un test fonctionnel Windows.

---

## 8. Règles de conduite pour la suite du développement

1. **Lire avant de modifier.** Comprendre les modules, les dépendances et l'historique du fichier.
2. **Un incrément à la fois.** Une seule intention par changement ; tests ciblés puis workspace complet.
3. **Aucune suppression sans preuve.** Garder les implémentations utiles, retirer les doublons seulement après cartographie.
4. **Aucun mock présenté comme production.** Les mocks sont confinés aux tests.
5. **Aucune promesse sans preuve.** README et site reflètent l'état mesuré, pas l'ambition future.
6. **Pas d'automatisation décorative.** Pas de cron qui répète des diagnostics sans corriger un problème ni produire un livrable.
7. **Sauvegarder avant les opérations risquées.** Préserver les changements non committés et les fichiers non suivis.
8. **GitHub après réconciliation.** Aucun merge ou push aveugle ; aucune force-push.
9. **Respecter le matériel et les données.** Ne pas réparer/formater/écrire sur un SSD externe ou une partition sans identification certaine et consentement spécifique.
10. **Garder l'ambition, réduire le scope.** Une fondation solide vaut mieux que dix modules qui semblent terminés mais ne fonctionnent pas ensemble.

---

## 9. État de sauvegarde de cette reprise

Une archive de sauvegarde du dépôt local a été créée et vérifiée :

- Chemin : `/home/lvs0/Backups/Polygone-real-pre-finalisation-2026-11-02.tar.gz`
- Taille : 34 Mo
- Contenu : dépôt, `.git`, fichiers suivis, modifications locales et fichiers non suivis ; répertoire Cargo `target/` exclu car il s'agit de résultats de build reproductibles.
- SHA-256 : `d687fd33890317efd0f453e80dfc874fa7f69803fb29a2f2071b5332372493bc`
- Vérification : `tar -tzf` réussie ; aucun chemin `target/` dans l'archive.

Cette archive est une copie de restauration du code et de son état Git, pas une sauvegarde du disque entier.

---

## 10. Première décision technique de la reprise

La prochaine tâche de code ne doit pas être d'ajouter une nouvelle fonctionnalité visible. Elle doit être la **réconciliation contrôlée de la divergence GitHub/local**, puis un audit de vérité sur les chemins critiques `daemon → bail → consommateur isolé` et `client → bloc → transport → pair physique`.

Ces deux chemins conditionnent le reste de la vision : sans ressources réellement contrôlées, le réseau de calcul est une promesse ; sans échange réel entre deux machines, le réseau décentralisé reste partiellement local. Une fois ces deux fondations prouvées, l'interface et les services peuvent être finalisés sans empiler de dette technique.
