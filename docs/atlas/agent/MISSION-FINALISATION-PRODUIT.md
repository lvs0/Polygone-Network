# MISSION — Finalisation complète de Polygone

> NOTE DE PORTAGE — 4 novembre 2026 : mission opérationnelle portée dans le dépôt produit ; STATE.md et ETAT-REEL.md à la racine font autorité pour l’état courant.

> **Ordre de travail destiné à l'agent qui reprend le produit.**
> État audité le 3 novembre 2026. Ce document complète `ORDRE.md`, `VISION.md`,
> `ETAT-REEL.md`, `ROADMAP.md` et `backlog.json`. Il ne remplace pas les
> décisions de Lévy et ne donne pas le droit de publier ou de fusionner.
>
> **Mission : transformer la vision documentée en produit réel, accessible,
> fiable et cohérent — pas seulement compiler un workspace ou produire des rapports.**

---

## 1. Ta responsabilité

Tu agis comme ingénieur produit principal, architecte, développeur, auditeur
sécurité, QA et concepteur UX. Tu dois comprendre l'intention, trouver les
trous que l'utilisateur n'a pas nommés, proposer les améliorations utiles et
les implémenter lorsqu'elles restent dans le périmètre autorisé.

Tu n'es pas autorisé à prétendre « penser comme Lévy » au sens de connaître
ses intentions non exprimées. Tu dois **reconstruire ses critères de décision**
à partir des messages utilisateur, des décisions confirmées et des limites
répétées, puis séparer explicitement :

- **[U] Demande utilisateur** : demande explicite et sourcée.
- **[C] Décision confirmée** : décision déjà prise dans le produit ou dans un document d'autorité.
- **[I] Inférence** : conséquence raisonnable de plusieurs demandes, à expliquer.
- **[P] Proposition** : amélioration nouvelle, utile mais non demandée explicitement.
- **[GATE-LÉVY]** : choix produit, légal, économique ou de publication qui exige l'accord du propriétaire.

Ne transforme jamais une inférence ou une proposition en décision utilisateur.

## 2. Sources à lire avant de choisir une tâche

Lis dans cet ordre, puis consulte les fichiers d'épique concernés avant chaque
modification :

1. `docs/atlas/agent/ORDRE.md` et le présent fichier.
2. `VISION.md` — vision actuelle de référence.
3. `ETAT-REEL.md` et `STATE.md` — à **rafraîchir** :
   leurs dates, branche et compteurs sont en partie périmés.
4. `ROADMAP.md`, `docs/atlas/plan/PLAN-FINITION.md`,
   `docs/atlas/plan/PLAN-ECOSYSTEME.md` et les six fichiers de `docs/atlas/plan/epiques/`.
5. `docs/atlas/agent/backlog.json` — file opérationnelle, à réconcilier avec les 132 tâches.
6. `NEEDS-LEVY.md`, `docs/atlas/plan/DECISION-POLY.md`,
   `SALVAGE.md` et `docs/RECONCILIATION.md`.
7. Les sources conversationnelles brutes restent privées et ne sont pas versionnées ; utiliser les synthèses et décisions portées dans ce dépôt.

Le corpus historique a été analysé localement. Les exports bruts et les données conversationnelles ne sont pas inclus dans ce dépôt.


**Ne publie jamais** `_work/`, les corpus, les conversations brutes, les bases
de données, les clés, les identités, les fichiers `.env`, les journaux privés
ou les chemins personnels. Le corpus sert à comprendre la vision, pas à nourrir
un commit ou un rapport public.

## 3. Vision produit à préserver

### 3.1 Le résultat attendu

**Polygone doit rendre la confidentialité et la puissance de calcul utilisables
par une personne non technique, sans plateforme centrale obligatoire.**

L'expérience doit être simple : une commande `polygone`, un parcours guidé,
des états compréhensibles, des commandes évidentes et aucun besoin de modifier
manuellement des fichiers de configuration ou d'ouvrir des ports pour l'usage
normal. Un développeur doit pouvoir intégrer le même socle via une API stable.

### 3.2 Invariants

- **Souveraineté des données** : identité et clés créées localement, contrôle par
  l'utilisateur, pas de compte cloud obligatoire, pas de télémétrie cachée.
- **Confidentialité honnête** : chiffrement de bout en bout, relais aveugle au
  contenu, mais documentation précise des métadonnées visibles. Le relais voit
  au minimum l'adresse IP source et des éléments de routage ; ne jamais promettre
  « zéro métadonnée », « anonyme » ou « impossible à tracer » sans preuve.
- **Post-quantique** : préserver les primitives réellement utilisées
  (notamment ML-KEM-1024, ML-DSA-65, AES-256-GCM, BLAKE3 et le partage Shamir
  4-sur-7 là où il est effectivement implémenté). Ne change jamais de primitive
  ou de format de protocole silencieusement.
- **Pair-à-pair** : le cloud et les relais peuvent être des options de transport,
  mais le cœur ne doit pas dépendre d'un fournisseur unique.
- **Gratuité réelle du socle** : pas de carte bancaire, d'abonnement ni de
  paywall pour les capacités gratuites annoncées. Les modèles locaux sont la
  base durable ; un free tier fournisseur peut disparaître.
- **Contribution volontaire** : le partage CPU/RAM/GPU/stockage/réseau est
  désactivé par défaut, activé par consentement explicite, borné et révocable.
- **Maîtrise immédiate** : pause, reprise, arrêt, limites, choix des services et
  retrait d'une contribution doivent être compréhensibles et observables.
- **Un produit cohérent** : un point d'entrée `polygone`, une API versionnée,
  des services intégrés, pas une collection d'applications sans continuité.
- **Accessibilité** : parcours non technique, localisation, navigation clavier,
  diagnostics lisibles, comportement adapté aux machines modestes.

### 3.3 Les attentes historiques à ne pas perdre

La source utilisateur décrit explicitement :

- une interface simple en quatre espaces : Accueil, Favoris, Services et
  Paramètres ; contrôle du nœud, des services et de la contribution ;
- un installateur guidé : langue, pseudo facultatif, services à activer,
  réseau/protocoles disponibles, mises à jour automatiques facultatives ;
- pause avec durée (10 min, 30 min, 1 h, 2 h ou 4 h), reprise, arrêt,
  redémarrage, mise à jour, réinstallation et désinstallation propre ;
- Msg éphémère, Drive distribué/chiffré avec liens temporaires, Hide en proxy,
  Mesh local (Wi-Fi/Bluetooth selon les décisions), calcul partagé par daemon,
  IA locale puis éventuellement distribuée, Gateway facultative et SDK ;
- une expérience utilisable sur Linux, macOS, Windows et les distributions
  Linux courantes.

Le code et les documents actuels ne réalisent pas encore toutes ces attentes.
Ne réduis pas la vision au seul état de ce qui compile aujourd'hui.

**Conflit de conception à résoudre sans arbitrage arbitraire :** la vision
historique demande quatre espaces d'interface, tandis que d'autres notes
décrivent une interface plus minimale à deux actions et `VISION.md` prévoit
un GUI grand public et une CLI/TUI d'administration. `UI.4` demande quatre
onglets. Documente les variantes et demande une décision avant de figer
l'architecture de navigation.

### 3.4 Ce que signifie le partage de puissance

Le daemon doit mesurer ce qui est réellement libre, garder une marge pour
l'utilisateur, réserver et libérer des ressources, appliquer les limites par
l'OS lorsque c'est possible et s'arrêter proprement en cas de perte de bail,
de veille, de batterie faible, de surcharge thermique ou d'activité utilisateur.

Ne présente pas de la **RAM distante** comme de la RAM locale extensible :
la mémoire d'une autre machine est une ressource réseau pour des tâches
déportées, sauf si une véritable couche de mémoire distribuée, avec ses
contraintes et ses latences, est implémentée et mesurée. Préfère des unités de
travail distribuées à une promesse trompeuse de « RAM infinie ».

Mesure le coût réseau, la latence et l'énergie avant d'affirmer qu'un ancien
ordinateur « booste » un autre. Si le transfert coûte plus que le calcul gagné,
le résultat correct est de refuser cette promesse ou de limiter le cas d'usage.

### 3.5 POLY : réciprocité, pas cryptomonnaie

La décision du 3 novembre est : **pas de cryptomonnaie POLY**. POLY peut devenir,
plus tard, une unité de compte de réciprocité non transférable dans le ledger du
daemon. Aucun marché, transfert, convertibilité, blockchain, rendement ou
portefeuille externe. Ne mélange jamais POLY avec les crédits de tokens/requêtes
du `hope-gateway`.

Ne commence pas l'unité de réciprocité avant un service mesurable, un réseau
réel et la décision D8 sur les plafonds de dette/crédit. Les reçus devront
attester l'exécution et être signés des deux côtés. Le choix final du nom et les
paramètres économiques restent soumis au propriétaire.

### 3.6 Hors périmètre immédiat

Conserve la provenance de ces idées, mais ne les ajoute pas au produit avant
que le socle soit prouvé et qu'une décision soit prise : OS Polygone, interface
neuronale, IA biomimétique/essaims cognitifs, téléphone comme nœud de calcul,
marketplace, token, Hide multi-hop présenté comme anonyme, moteur de recherche
distribué et nouvelles applications indépendantes. Ne confonds pas une idée
ancienne avec une priorité actuelle.

## 4. État réel vérifié lors de cet audit

- Dépôt de travail audité : `ce dépôt Polygone Network`, branche locale
  `realign/v2`, HEAD `1c34c8f`, arbre propre au moment du contrôle.
- Vérifications sur cette branche : `cargo fmt --all -- --check` PASS ;
  `cargo test --workspace -j 2` PASS, **362 tests au total** (361 tests
  unitaires/intégration et 1 test de documentation) ; Clippy avec
  `-D warnings` PASS ; `node scripts/check-docs.mjs --tests 362` PASS.
- Workspace : 7 membres Rust, 4 binaires, environ 29 322 lignes Rust.
- La branche `realign/v2` n'a pas de branche distante `origin/realign/v2`.
  `realign/v1` et `realign/v2` n'ont pas de merge-base commun ; le calcul
  comparatif donne 14 commits propres à v1 et 11 propres à v2. La branche
  locale v1 est en avance de 6 commits sur `origin/realign/v1`.
- `origin/main` est une autre lignée. **Ne merge, rebase, force-push ou
  synchronise aucune de ces lignées automatiquement.**
- `checkout historique Polygone-real` reste sur `main`, HEAD `f3d09c0`, avec
  **65 entrées locales non commitées** (39 modifiées, 26 non suivies).
  C'est une source à préserver en lecture seule, pas le checkout de travail.
- `Polygone-Atlas` est un dépôt de documentation distinct, branche
  `main-v2`. Garde les trois dépôts et leurs historiques séparés.

### Écarts documentaires à corriger avant de se fier aux compteurs

1. Plusieurs docs annoncent encore `realign/v1`, alors que le checkout actif
   est `realign/v2` ; le statut et le README doivent distinguer la branche
   publiée v1 de la branche locale v2 non publiée.
2. `PLAN-FINITION.md` annonce 15 épiques, mais énumère 17 modules/épiques
   (132 identifiants de tâches répartis dans six fichiers).
3. `backlog.json` a actuellement 21 tâches ouvertes, 13 clôturées et
   3 bloquées. Il ne couvre que 29 des 132 identifiants d'épiques ;
   103 identifiants sont absents de la file. Cinq tâches de la file
   (`AGT.1`, `AGT.2`, `GWX.1`, `GWX.2`, `RECIP.1`) sont transversales
   et n'appartiennent pas aux six fichiers d'épiques. Réconcilie chaque ID
   avec le code avant de décider s'il est ouvert, terminé, remplacé ou sans objet.
4. `STATE.md` annonce encore v1, 361 tests et un ancien nombre de tâches
   clôturées ; ses compteurs et sa prochaine tâche ne correspondent plus tous
   au backlog courant.
5. `ORDRE.md` référence `BACKLOG.md`, alors que la file réelle est
   `docs/atlas/agent/backlog.json`. `dispatch.json` doit rester cohérent avec la
   limite de deux tâches simultanées.
6. `ETAT-REEL.md` est un instantané daté. Rejoue les preuves après tout
   changement avant de reprendre ses chiffres comme vérité courante.

Ces divergences sont des tâches réelles : un agent ne doit pas piloter le
produit à partir d'un état périmé.

## 5. Fonctionnalités à finir et preuves attendues

### G0 — Vérité, sauvegarde et cohérence Git

- Relever les branches, HEAD, upstream, arbre et commits de chacun des trois
  dépôts ; enregistrer un inventaire reproductible.
- Garder `Projects/Polygone-real` intact. Ne jamais copier par-dessus ses
  65 changements sans comparaison octet par octet et sauvegarde restaurable.
- Conserver les lignées v1, v2 et main. Préparer une analyse commit par commit
  et une proposition de stratégie ; attendre l'accord avant fusion ou push.
- Réconcilier tous les identifiants des 132 tâches avec le backlog et les
  preuves, sans cocher une tâche sur la foi d'un document ancien.
- Mettre à jour README, STATE, ETAT-REEL, PLAN-FINITION, index des documents
  et ordre de l'agent. Ajouter un test qui détecte les compteurs périmés.
- Ne jamais publier les corpus, les fichiers de travail ou les secrets.

**Sortie G0 :** les trois dépôts, branches, tâches, preuves et prochaines
actions sont compris et cohérents ; aucun travail utilisateur n'est perdu.

### G1 — Ressources et daemon

- Confirmer les baux : échéance, renouvellement, expiration, récupération après
  SIGKILL du client et du daemon, libération idempotente et persistance.
- Vérifier l'isolation Linux dans un cgroup dédié, les limites CPU/RAM pendant
  l'exécution et la disparition du scope à l'arrêt.
- La décision D2 (`--isoler` par défaut) appartient au propriétaire. En
  attendant, ne transforme pas une simple licence logicielle en limite OS.
- Vérifier que l'activité inconnue, l'absence de mesure, la batterie, la
  température, la veille et la fermeture du capot entraînent un comportement
  conservateur, jamais une allocation risquée.
- Tester les plafonds et le retrait sur la vraie machine, avec marge pour
  le système et l'utilisateur ; ne jamais immobiliser durablement la machine.
- Exposer clairement mesuré / réservable / contraint dans `doctor` et `status`.

**Sortie G1 :** une charge contrôlée est réellement bornée par l'OS sur la
plateforme annoncée ; un crash libère les ressources ; l'utilisateur garde
toujours la main.

### G2 — Réseau entre machines physiques

- Priorité aux tests `BLOCKS.9`, `MSG.6`, `MESH.1`, `SEC.3` et `SEC.4`.
- Tester deux machines physiques, un pare-feu réel, puis deux réseaux/NAT ;
  consigner hôtes, versions, commandes, horodatages et résultats.
- Transférer message et fichier ; comparer les octets reçus à la source ;
  tester délai, perte, redémarrage, reconnexion, pair inconnu et trame hostile.
- Prouver l'anti-rejeu à travers le relais et l'impossibilité d'usurper
  l'expéditeur ; ne pas se contenter des tests unitaires existants.
- Empêcher les annonces Mesh de partir sur VPN ou interfaces non autorisées.
  Le choix Wi-Fi/Bluetooth/mDNS/Wi-Fi Direct doit être explicite et documenté.
- Mesurer latence, débit, mémoire, CPU et coût du relais. Un résultat négatif
  doit réduire la promesse, pas être masqué.

**Sortie G2 :** un transcript reproductible entre deux hôtes distincts, avec
pare-feu, transfert vérifié et cas d'échec testés. Des processus locaux ne
remplacent pas cette preuve.

### G3 — Parcours grand public

- Parcours de référence : installer → choisir langue/services → créer ou
  récupérer une identité → appairer → envoyer/recevoir → contrôler la
  contribution → pause/reprise → mise à jour → désinstaller.
- Pas de terminal avancé, de port à ouvrir ni de configuration cachée dans le
  parcours normal.
- Ajouter des diagnostics actionnables : connecté/inconnu/hors ligne, dernière
  preuve, ressources mesurées et réellement contraintes, cause d'un refus.
- Implémenter pause et reprise avec durée réelle, et prouver qu'elles libèrent
  puis recréent les baux.
- Rendre l'installation idempotente et la désinstallation symétrique. Séparer
  « retirer l'application » de « supprimer définitivement identité et données » :
  la seconde action exige une confirmation explicite.
- Tester sur VM Linux vierge et matériel modeste. Ajouter des tests réels macOS
  et Windows avant d'annoncer ces plateformes.
- Résoudre le choix de navigation (deux actions minimales ou quatre espaces)
  avant de figer la structure de l'interface.

**Sortie G3 :** une personne non technique termine le parcours sur une machine
vierge sans aide terminal et sans perdre ses données par surprise.

### G4 — Services, dans l'ordre des dépendances

- **Msg** : 30 secondes annoncées et vérifiées ; accusé de réception explicite,
  échec visible si non livré, comportement de reconnexion et politique de file
  hors ligne tranchée.
- **Blocks / Drive** : chiffrer au repos, découper les gros fichiers, manifester
  les blocs, vérifier l'intégrité, préserver provenance et identité de
  l'expéditeur, quotas, réplication, récupération et liens temporaires.
  `drive.rs` n'est pas un service tant qu'il n'est pas monté et appelé en
  production. La décision D3 précède son implémentation.
- **Hide** : livrer et mesurer le mono-saut honnête. Pas de promesse d'anonymat
  ni de multi-hop sans architecture, modèle de menace et test externe.
- **Serverless** : le quorum et les reçus unitaires ne prouvent pas une
  exécution multi-nœuds. Implémenter le dispatch réseau réel, la validation
  des résultats et le traitement d'un nœud défaillant avant toute promesse.
- **Petals / Brain** : Ollama local est distinct de l'inférence distribuée.
  Connecter les workers, mesurer transfert des poids, mémoire, débit et
  latence ; arrêter ou réduire le périmètre si le réseau annule le gain.
- **SDK / Gateway / bridges** : geler une API versionnée, documenter les
  erreurs, quotas et compatibilités ; tester un client externe indépendant.
  Gateway reste loopback tant que TLS, authentification mutuelle, limites et
  modèle de menace ne sont pas prouvés et que D7 n'est pas tranchée.

**Sortie G4 :** chaque service annoncé a un appelant de production, un modèle
de menace, des tests positifs et négatifs, une preuve de fonctionnement et un
mode dégradé.

### G5 — Sécurité et livraison

- Faire du fail-closed une règle pour permissions de clés, appairage, erreurs
  de signature, limites de taille, anti-rejeu, désérialisation et réseau.
- Auditer les métadonnées du relais et résoudre D6 sur la journalisation IP.
- Ajouter scan de secrets, audit des dépendances, fuzzing ciblé des parseurs,
  tests de charge et revue indépendante.
- CI par OS réellement supporté ; ne jamais mettre Windows en blocage vert/rouge
  si le code ne le supporte pas.
- Paquets reproductibles, SBOM, sommes de contrôle, provenance, signatures,
  mise à jour vérifiée et rollback.
- Guide d'installation, d'usage, de sauvegarde/restauration, de récupération
  d'identité, de confidentialité et de divulgation de vulnérabilité.

**Sortie G5 :** une tierce personne peut vérifier l'artefact et sa provenance,
installer, utiliser, mettre à jour et désinstaller sans dépendre de l'agent.

## 6. Améliorations à rechercher même si elles n'ont pas été demandées

À chaque revue, cherche explicitement les risques invisibles suivants. Ajoute
une tâche avec preuve lorsqu'un défaut réel est découvert ; ne code pas une
fonctionnalité spéculative :

1. **Récupération** : sauvegarde chiffrée de l'identité, export/import,
   migration de version, restauration testée et prévention de perte silencieuse.
2. **Cycle de vie** : coupure réseau, veille, redémarrage, crash, bail expiré,
   espace disque plein, horloge incorrecte, mise à jour interrompue.
3. **Consentement et limites** : arrêt instantané, plafonds CPU/RAM/GPU/disque/
   réseau, température, batterie, activité utilisateur et capot fermé.
4. **Réseau** : IPv4/IPv6, DNS, NAT, pare-feu, timeouts, retry borné,
   back-pressure, anti-rejeu, version de protocole et pair malveillant.
5. **Sécurité locale** : permissions strictes, stockage des clés, séparation
   des privilèges, sandbox, moindre privilège, absence de secret dans logs.
6. **UX** : états inconnus explicites, erreurs réparables, accessibilité
   clavier/lecteur d'écran, traduction, petits écrans, absence de fausses
   métriques et parcours de première utilisation.
7. **Performance** : benchmark reproductible sur matériel modeste, mémoire
   plafonnée, démarrage, consommation au repos, débit réel et coût réseau.
8. **Compatibilité** : migration des formats, version de protocole, downgrade,
   coexistence avec une ancienne version et rollback.
9. **Vie privée** : aucun appel réseau implicite, aucun analytics caché,
   journaux locaux bornés et audit de ce que voit le relais.
10. **Maintenance** : API stable, messages d'erreur exploitables, tests de
    non-régression, dépendances minimales et documentation générée à partir
    des faits mesurés.

Pour toute amélioration non demandée, consigne : problème observé, lien avec
la vision [I] ou [P], bénéfice, coût, risque, dépendances, preuve d'acceptation
et décision nécessaire. Traite immédiatement les défauts P0 de sécurité,
perte de données ou promesse mensongère ; place le reste au bon niveau de
priorité.

## 7. Décisions humaines interdites à l'agent

Ne tranche pas à la place du propriétaire :

- D1 licence AGPL vs MIT et stratégie d'adoption entreprise ;
- D2 `--isoler` par défaut sous Linux ;
- D3 réactivation et périmètre minimal de Drive ;
- D4 protocoles Mesh (mDNS, Bluetooth, Wi-Fi Direct) ;
- D5 file hors ligne de Msg ;
- D6 conservation ou non des IP dans les logs du relais ;
- D7 ouverture de Gateway sur Internet ou loopback seulement ;
- D8 plafonds de dette et de crédit pour la réciprocité POLY ;
- D9 format de distribution prioritaire ;
- D10 sécurisation/exposition du Gateway ;
- D11 publication ou maintien privé du fork Freebuff ;
- nombre et organisation définitifs des écrans ;
- publication d'une release, changement de marque, suppression d'archives,
  activation d'un service payant ou création d'un actif financier.

Écris la question précise dans `NEEDS-LEVY.md`, marque les tâches dépendantes
`blocked` et continue les tâches indépendantes.

## 8. Règles d'exécution

1. Avant chaque tâche : vérifier branche, HEAD, `git status`, dépendances,
   appelants de production et tests actuels.
2. Reproduire le défaut avant de le corriger. Ajouter un test négatif qui échoue
   avant le correctif quand c'est possible.
3. Lire les fichiers complets concernés, leurs appelants et leurs consommateurs.
   Une fonction isolée ou un test isolé ne vaut pas intégration.
4. Un changement doit rester petit, réversible, lié à une tâche et à une preuve.
5. Deux agents ne modifient jamais le même fichier. Maximum **deux tâches
   simultanées** sur cette machine ; sérialise si la cohérence l'exige.
6. Après chaque lot, un reviewer distinct relit le diff complet et vérifie
   signatures, erreurs, sécurité, documentation et compatibilité.
7. Exécuter les tests ciblés, puis les checks de workspace pertinents.
   N'affaiblis aucune assertion, ne supprime aucun test et ne masque aucun échec.
8. Un commit par unité cohérente et preuve exécutée. Pas de commit « fini »
   simplement parce que le code compile.
9. Mets à jour `backlog.json`, `STATE.md`, `ETAT-REEL.md` et les documents
   qui citent la capacité après une modification réelle.
10. Pas de cron qui tourne à vide, de rapport « RAS » ou de boucle sans
    livrable. Arrête-toi proprement si la preuve est impossible ou si un choix
    appartient au propriétaire.

### Interdictions Git et données

- Aucun `push --force`, `reset --hard`, `clean -fd`, réécriture d'historique,
  merge ou rebase automatique entre les lignées actuelles.
- Ne touche pas au checkout source `checkout historique Polygone-real` tant que ses
  65 modifications ne sont pas sauvegardées et réconciliées explicitement.
- Ne pousse rien sur `main`, ne publie aucune release et ne supprime aucune
  version originale sans accord.
- Ne lis, ne copies et ne commits aucune identité privée, clé, `.env`,
  `ledger.json`, base personnelle ou conversation brute.
- Ne mets pas en réseau le Gateway par défaut et n'exécute pas de job distant
  sans consentement et sandbox vérifiée.

## 9. Définition stricte de « terminé »

Une tâche est terminée seulement si les sept conditions suivantes sont vraies :

1. un appelant de production utilise réellement la fonctionnalité ;
2. des tests positifs **et négatifs** la couvrent ;
3. le comportement est observé sur le système réel ;
4. crash, redémarrage et retrait ne laissent pas d'état fantôme ;
5. documentation, interface et comportement concordent ;
6. commande, OS, environnement, date et sortie rendent la preuve reproductible ;
7. une sauvegarde restaurable existe et Git est propre ou ses écarts sont listés.

Une capacité qui ne remplit pas ces conditions reste `partielle`, `non câblée`
ou `bloquée`. Une promesse retirée avec raison et preuve est un progrès réel.

## 10. Première séquence obligatoire

1. Réconcilier l'état de Git, les compteurs, les IDs de tâches, le README,
   `STATE.md`, le backlog et les ordres de l'agent. Ne faire aucun merge/push.
2. Mettre en cohérence `dispatch.json` avec la limite de deux tâches et
   vérifier que le mode dry-run est clairement indiqué tant que
   `agentCommand` est nul.
3. Reproduire et terminer la prochaine tâche non bloquée selon les gates,
   en commençant par les preuves réseau physiques dès que deux hôtes sont
   disponibles ; préparer les scripts et le protocole de test si ce prérequis
   humain manque.
4. Avancer ensuite vers le parcours grand public et les services, sans sauter
   les dépendances ni les décisions `GATE-LÉVY`.
5. À chaque session, annoncer une tâche et sa preuve, puis rapporter seulement
   ce qui a été réellement exécuté, ce qui est bloqué et la prochaine action.

**L'objectif n'est pas de cocher 132 cases. L'objectif est de livrer un produit
que quelqu'un peut installer, comprendre, contrôler, utiliser et vérifier —
tout en respectant la vie privée et sans mentir sur ses capacités.**
