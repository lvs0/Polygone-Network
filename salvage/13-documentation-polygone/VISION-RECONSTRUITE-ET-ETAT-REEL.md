# Polygone — vision reconstruite et état réel

**Date de consolidation : 1 novembre 2026**  
**Sources historiques :** `Downloads/Data Graph/state.db.bak`, messages utilisateur du 10 mai 2026 (message DB 8723, session `20260510_084845_56e131`) et du 18 juillet 2026 (message DB 137560, session `20260718_112302_9df55408`).  
Les deux messages bruts sont préservés dans ce dossier. Le message du 10 mai apparaît plusieurs fois dans l'historique : ses répétitions ne sont pas traitées comme des décisions indépendantes.

> Ce document sépare l'intention historique, le code effectivement inspecté et ce qui reste à démontrer. Une phrase dans une note ou un README n'est pas une preuve d'implémentation.

## 1. L'idée centrale

La phrase qui organise le projet est :

> « Tes données sont à toi. Personne ne doit les voir, les toucher ou même savoir qu'elles existent. »

Polygone n'est pas d'abord une messagerie ni un VPN. C'est un **réseau personnel, décentralisé et orienté confidentialité**, dont les services doivent partager un même socle de transport, d'identité, de cryptographie, de découverte et de gestion des ressources. Chaque utilisateur participe comme nœud du réseau, sans dépendre d'un serveur central qui deviendrait la source de vérité ou le lieu de stockage des données privées.

La promesse de produit doit être compréhensible par quelqu'un qui ne sait pas ce qu'est un port, un nœud, une clé publique ou un relais. Le produit vise une commande unique (`polygone`), une interface cohérente et un installateur simple, tandis que la complexité réseau reste dans le runtime.

Les publics explicitement nommés dans la note historique sont les particuliers, journalistes, lanceurs d'alerte et personnes qui souhaitent reprendre le contrôle de leurs données.

## 2. L'écosystème envisagé

| Module | Rôle dans la vision | Sens produit |
|---|---|---|
| **Drive** | Stockage distribué et chiffré, objets fragmentés, liens éphémères, streaming | Conserver et partager des fichiers sans transformer un fournisseur central en coffre unique |
| **Msg** | Messagerie pair-à-pair, éphémère, effacement après lecture | Communiquer sans laisser un historique serveur permanent |
| **Hide** | Proxy SOCKS5 intégré | Sortir vers le web depuis un nœud du réseau, sans configuration complexe |
| **Mesh** | Réseau local Wi-Fi/Bluetooth et mutualisation des ressources | Utiliser plusieurs machines proches comme un ensemble, notamment un PC ancien assisté par un PC plus puissant |
| **Brain / Petals** | Inférence IA locale et, à terme, distribuée | Faire circuler des calculs ou des parties de modèles au lieu d'envoyer les données personnelles vers un cloud |
| **Gateway** | Pont facultatif entre Polygone et le web classique | Rendre une ressource hébergée sur le réseau accessible depuis un navigateur ordinaire |
| **Compute / Serverless** | Ressources prêtées et travaux répartis ; serveur logique réparti sur plusieurs nœuds | Exécuter des services sans imposer une machine serveur unique |

La note de juillet ajoute des extensions stratégiques envisagées : synchroniser ou sélectionner des nœuds de latence comparable, proposer un « Glow-Up » système facultatif et réglable, distribuer l'inférence de modèles, préparer des intégrations possibles avec des messageries ou des produits axés confidentialité, et rendre le déploiement d'un site/domaine simple. Ce sont des directions de recherche, pas des intégrations déclarées livrées.

## 3. L'expérience utilisateur recherchée

La maquette historique est une TUI élégante, pilotable aux flèches, avec quatre espaces :

1. **Accueil** : état du nœud, durée, solde interne POLY, consommation, activation, mise à jour, redémarrage et pause.
2. **Favoris** : accès aux services préférés et activation/désactivation rapide.
3. **Services** : contrôle fin de chaque service et pauses temporisées (10 min, 30 min, 1 h, 2 h, 4 h).
4. **Paramètres** : configuration du nœud, Drive, espace disponible et personnalisation des favoris.

L'utilisateur ne devrait pas avoir à ouvrir un terminal pour choisir un port ou modifier un fichier réseau. L'interface doit montrer les conséquences réelles d'une action, et non simuler un état.

La vision de juillet précise aussi que le Glow-Up doit être **opt-in**, proposé comme recommandé mais jamais imposé, réglable à l'installation et dans les paramètres. Un utilisateur choisit les ressources qu'il accepte de prêter ; son ordinateur doit conserver une marge pour sa propre utilisation.

## 4. Les invariants de confidentialité à préserver

- Les clés privées restent sous le contrôle local de leur propriétaire.
- Les données applicatives sont chiffrées avant de quitter le nœud d'origine.
- Les fragments ne sont pas lisibles individuellement et leur reconstruction exige un seuil.
- Un relais transporte des fragments sans devenir un dépôt permanent de contenu.
- L'absence de stockage central ne doit pas être confondue avec l'absence de métadonnées : adresses IP, horaires, volumes, fréquence, disponibilité et topologie peuvent encore être observables.
- Les modules réseau doivent avoir un modèle de menace propre et une revue indépendante avant toute promesse d'anonymat.

**Écart documentaire important :** la note du 10 mai nomme ML-DSA-87, tandis que le code actuel consulté déclare et utilise ML-DSA-65. La spécification doit donc être corrigée pour refléter la primitive réellement utilisée, ou le code devra évoluer et être audité avant d'annoncer un autre niveau. La documentation actuelle du dépôt nomme ML-KEM-1024, ML-DSA-65, AES-256-GCM, BLAKE3 et Shamir 4-of-7.

## 5. État observé dans le dépôt au 1 novembre 2026

Dépôt principal inspecté : `~/Projects/Polygone-real`, branche `main`, dernier commit observé `f3d09c0` du 29 octobre 2026. Le worktree contenait des changements utilisateur et des fichiers non suivis avant cette intervention ; ils ont été préservés. Aucun commit, reset, stash, checkout ou nettoyage n'a été effectué.

| Domaine | État constaté | Ce que cela signifie |
|---|---|---|
| Crates Rust | Client, core crypto, relay, daemon, gateway, petals, serverless présents | Un véritable workspace est là, pas un projet vierge |
| Msg / Drive | Déclarés live dans la table de services ; le README décrit un transport chiffré et fragmenté | Parcours existants, à auditer service par service avant une validation externe |
| Hide | SOCKS5 via relais et exit node, documenté **single-hop** | Ce n'est pas Tor, pas du multi-hop validé, ni une preuve d'anonymat |
| Brain / Petals | Inférence locale via Ollama disponible ; planner de distribution présent, distribution des poids non branchée | L'IA distribuée reste partielle |
| Mesh | Découverte LAN et voisins annoncés dans la documentation | Mutualisation locale réelle à vérifier sur plusieurs machines physiques |
| Compute / Glow-Up | Daemon, inventaire de ressources, paliers, politique de limites et allocation présents | Il existe un moteur de décision et des contrôles ciblés ; ce n'est pas encore un optimiseur universel de Windows/macOS/Linux |
| Serverless | Crate et CLI, quorum et Shamir de transit présents ; dispatch sur relais non branché | Infrastructure préparatoire, pas hébergement serverless distribué complet |
| Gateway | API HTTP, site statique, appairage et PWA présents | Pont applicatif réel ; la publication publique sécurisée et la liaison DNS/domaine demandent encore une conception |
| UI | TUI/CLI présents | La maquette quatre onglets et l'expérience grand public ne sont pas encore prouvées comme produit fini |
| Sécurité externe | Le dépôt indique qu'aucun audit externe n'a encore été réalisé | Ne pas annoncer « audité », « invisible » ou « plus sûr que Tor » |

### Preuve ajoutée pendant cette session

Le gateway avait 28 tests unitaires. Un test d'intégration réseau a été ajouté sous `crates/gateway/tests/serve.rs`, sur un vrai socket TCP local. Il vérifie les endpoints d'état et de services, le JSON, les erreurs 404/405, le flux d'appairage et le rejet des deux formes de traversée de chemin.

Le même test a révélé un défaut concret : un corps HTTP annoncé supérieur à 1 Mio finissait auparavant en erreur 400 au lieu de 413. Le parseur renvoie maintenant un marqueur borné sans lire le corps contrôlé par le client ; le routeur répond 413. Vérifications du crate gateway : **28 tests unitaires + 2 tests d'intégration réussis**, `cargo fmt --check`, Clippy `-D warnings` et `git diff --check` réussis. La suite workspace complète avait auparavant échoué une fois sur un test de persistance du daemon qui passe isolément ; le problème de concurrence autour de `POLYGONE_LEDGER_PATH` reste à traiter.

## 6. Les écarts structurants à fermer

### A. Un contrat de sécurité vérifiable
Définir les acteurs, secrets, menaces, métadonnées observables, durée de conservation, compromission de nœuds, relais malveillants, rejouabilité, corrélation de trafic et limites de l'effacement. Une propriété de cryptographie ne suffit pas à prouver l'anonymat du réseau.

### B. Un protocole commun aux services
Stabiliser identité, découverte, transport, adressage, fragmentation, accusés, expiration, reprise après perte et version de protocole. Un même contrat doit être utilisé par Msg, Drive, Hide, Petals et les futures intégrations.

### C. Synchronisation et routage mesuré
La latence comparable évoquée en juillet doit devenir une politique mesurable : RTT observé, perte, stabilité, capacité, disponibilité, coût et diversité de chemins. Ne pas confondre « latence semblable » et synchronisation d'horloge ; ne pas concentrer toutes les requêtes sur un groupe rapide au détriment de la résilience.

### D. Glow-Up respectueux de la machine
Garder une politique opt-in, des plafonds explicites, une marge système, des profils Éco/Équilibré/Performance et une suspension automatique quand l'utilisateur travaille ou quand la température/charge augmente. L'état actuel contient un ordonnanceur/politique de ressources, mais le code du daemon précise qu'il n'écrit pas dans les cgroups et que certaines ressources ne sont que des réservations déclaratives. L'effet réel de chaque action d'optimisation doit être mesuré par OS.

### E. Petals / inférence distribuée
Séparer le plan de calcul de l'exécution. Il faut définir partition de modèle, compatibilité des poids, transport, scheduling, reprise, intégrité des résultats, confidentialité des prompts et coût réseau. Le planner seul ne constitue pas de l'inférence distribuée.

### F. Serverless / domaine
Définir la localisation et réplication de l'état, quorum, disponibilité, mise à jour, certificats, DNS, protection DDoS, abuse handling et responsabilité des nœuds. « Aucun serveur unique » ne doit pas cacher les points de contrôle qui restent centralisés.

## 7. Ordre de travail recommandé

1. **Verrouiller le modèle de menace et les contrats du protocole** avant de multiplier les services.
2. **Rendre les preuves reproductibles** : tests socket, scénarios multi-nœuds, perte de relais, redémarrage, expiration, analyse métadonnées.
3. **Consolider l'expérience utilisateur** autour d'une commande, de la TUI et d'un état réel des services.
4. **Terminer Compute/Glow-Up** avec budgets, consentement, limites mesurées et contrôles multiplateformes ; commencer par Linux.
5. **Terminer Petals local puis distribué** avec un premier modèle et un test multi-machines documenté.
6. **Terminer Serverless/Gateway** seulement après les invariants réseau et le modèle de confiance.
7. **Préparer les intégrations externes** comme SDK/protocoles documentés, sans prétendre remplacer les garanties propres à Signal, Telegram, Proton ou Tor.

## 8. Ce qui ne doit pas être fait

- Ne pas réécrire le dépôt vivant ni repartir d'un template générique : il contient déjà des fonctionnalités et un grand volume de travail non commité.
- Ne pas mélanger dans une release les capacités `live`, `partial` et `staging`.
- Ne pas présenter Hide single-hop comme Tor ou comme un anonymat démontré.
- Ne pas présenter un planner Petals ou Serverless comme une exécution distribuée.
- Ne pas activer des optimisations système agressives, ni retirer les marges nécessaires au fonctionnement personnel de l'ordinateur.
- Ne pas publier de secrets, dumps de conversations, identifiants ou données personnelles dans le graphe de connaissances.

## 9. Sources locales de référence

- `00-vision-source-2026-05-10.md` — message utilisateur historique complet du 10 mai, conservé sans réécriture.
- `01-glow-up-and-network-expansion-2026-07-18.md` — message utilisateur du 18 juillet, conservé sans réécriture.
- `~/Projects/Polygone-real/README.md`, `STAGING.md`, `THREAT_MODEL.md`, `ARCHITECTURE.md`.
- `~/Projects/Polygone-real/crates/gateway/src/routes.rs` — statuts live/partial.
- `~/Projects/Polygone-real/daemon/src/policy/glow_up.rs`, `daemon/src/socket.rs` — politique ressources et limites.
- `~/Projects/Polygone-real/crates/client/src/demo.rs` — démo crypto locale ; ne remplace pas un audit réseau externe.

