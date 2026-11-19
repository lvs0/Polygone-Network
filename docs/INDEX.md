# Index documentaire de `docs/`

> Inventaire vérifiable au **2026-11-04**. Il couvre récursivement chaque fichier
> `.md` sous `docs/`, y compris cet index et les documents Atlas importés.
> Le périmètre trouvé avant création était de 61 fichiers ; cet index inclus,
> puis les documents de pilotage ajoutés depuis (`FOUNDER_VISION.md`,
> `EXECUTION_STATUS.md`, `THREAT-MODEL-SERVICES.md`, `threat-msg.md`), et les
> deux documents P0 du 2026-11-09 (`PROTOCOL-RELAY-AUTH.md`,
> `P0-DEUX-MACHINES.md`, `MISSION-P0-RAPPORT-2026-11-09.md`,
> `adr/0001-authentification-du-relais.md`), le total attendu est **85 fichiers
> Markdown** (90 avec les trois notes de consolidation/P0 du 2026-11-14 ; l'ADR de réconciliation canonique du 2026-11-09 et les sept preuves
> modales du 2026-11-10 s'y ajoutent).
>
> **Règle de date :** une date de contenu ou de nom de fichier est indiquée
> lorsqu'elle est explicite. Pour les documents Atlas sans date de rédaction
> fiable, « 2026-11-04 (import) » désigne uniquement leur portage dans ce dépôt.
> « Non indiquée » signifie qu'aucune date de rédaction fiable n'a été établie.
>
> **Règle de statut :**
> - **autorité** : fait foi dans le seul périmètre indiqué dans la justification ;
> - **daté** : instantané, rapport ou plan historique qui ne décrit pas seul l'état courant ;
> - **brouillon** : proposition, spécification ou document qui attend une vérification/validation ;
> - **à supprimer** : réservé aux doublons exacts ou artefacts dont le retrait est démontré.
>
> Les statuts « autorité » sont strictement locaux à leur sujet. Pour la vision,
> l'état produit et les décisions de projet générales, les sources de référence
> restent les fichiers racine `VISION.md`, `ETAT-REEL.md` et `STATE.md`.
> Aucun fichier n'est classé « à supprimer » : aucun doublon exact n'a été prouvé.
> Les documents datés ne sont pas réécrits pour faire disparaître leurs anciennes
> affirmations : depuis DOC.3, chacun porte le bandeau `Instantané daté` et un
> lien vers `ETAT-REEL.md`/`STATE.md`. `scripts/check-docs.mjs` refuse un
> document classé `daté` qui n'aurait plus son bandeau.
>
| Fichier | Titre | Date du document | Statut | Thème | Justification / preuve |
|---|---|---|---|---|---|
| `docs/INDEX.md` | Index documentaire | 2026-11-04 | autorité | Inventaire documentaire | Présent index ; couverture vérifiée automatiquement par `crates/client/tests/doc_index.rs`. |
| `docs/architecture/API-CONTRACTS.md` | Contrats d'API existants | 2026-11-15 | autorité | Contrats / interfaces | Décrit uniquement les interfaces implémentées (socket daemon v1, gateway `/api/v1` dont diagnostics et rôles Device Reuse, relais) et marque explicitement les domaines absents (usage, événements, réglages). |
| `docs/architecture/COMPONENT-MAP.md` | Carte des composants | 2026-11-09 | autorité | Architecture / inventaire | Mesures par crate (fichiers, lignes, tests) et responsabilités ; les composants sans appelant de production sont signalés. |
| `docs/architecture/DATA-FLOWS.md` | Flux de données réels | 2026-11-09 | autorité | Architecture / flux | Décrit les flux identité, messages, relais, ledger, calcul, blocs et gateway, avec les échecs observés ; liste les flux absents. |
| `docs/architecture/SYSTEM-ARCHITECTURE.md` | Architecture système canonique | 2026-11-09 | autorité | Architecture / socle | Décrit les couches, la topologie, les états, la persistance et les frontières de confiance du code au HEAD `e2936a3`. |
| `docs/prototypes/FABRIC-2026-11-14.md` | Prototype fabric — calcul bidirectionnel | 2026-11-14 | daté | Prototype / calcul | Tranche expérimentale non intégrée au produit : 32 tests dédiés, 544 tests workspace, limites explicites (loopback, pas de TLS, pas de sandbox). |
| `docs/prototypes/TRACEABILITY-2026-11-14.md` | Matrice de traçabilité des concepts | 2026-11-14 | daté | Prototype / traçabilité | Associe chaque concept à sa source, son implémentation et son état ; Babel reste non implémenté (`DEC-028`). |
| `docs/prototypes/FABRIC-DECISION-2026-11-14.md` | Décision de consolidation Compute | 2026-11-14 | daté | Décision / Compute | Décision C : Fabric reste un lab opt-in, sans transport ni sandbox dans le produit ; cible B = porter le planificateur seul après mesures réelles. |
| `docs/prototypes/BABEL-RESEARCH-2026-11-14.md` | Recherche Babel et décision requise | 2026-11-14 | daté | Recherche / Babel | Sépare les trois interprétations retrouvées (adressage total, CAS + calcul, reconstruction générative) ; aucune implémentation avant `DEC-028`. |
| `docs/security/THREAT-MODEL.md` | Modèle de menace du socle | 2026-11-09 | brouillon | Sécurité / menace | Auto-évaluation citant les protections testées et les risques résiduels ; aucun audit externe, donc pas « autorité ». |
| `docs/testing/TEST-STRATEGY.md` | Stratégie de test et niveaux de preuve | 2026-11-09 | autorité | Tests / portes qualité | Définit L0–L5, ce que chaque niveau prouve et ne prouve pas, les contre-épreuves exigées et les trous connus. |
| `docs/testing/VALIDATION-RESULTS.md` | Résultats de validation du 2026-11-09 | 2026-11-09 | daté | Tests / preuves d'exécution | **Instantané daté** des commandes, codes de sortie et limites ; il ne décrit pas seul l'état courant. |
| `docs/ARCHITECTURE_RESEAU.md` | Architecture réseau et partage de ressources | 2026-10-01 (mise à jour 2b-13) | daté | Architecture réseau | En-tête : état 2b-13 et test entre deux machines encore non exécuté ; les exigences réseau actuelles sont reprises dans `docs/atlas/plan/epiques/01-transport-confidentialite.md`. |
| `docs/BUDGET.md` | Budget et soutenabilité du relay | Non indiquée | brouillon | Financement / coûts | Le texte présente des estimations pour un premier relay public, pas une facture ni des coûts mesurés ; aucun état de dépenses n'est joint. |
| `docs/CORE-COMPLETE.md` | Rapport de complétude du core | 2026-08-31 | daté | Historique des tests et du workspace | Le bandeau du fichier le déclare instantané historique (4 crates / 109 tests) ; `ARCHITECTURE.md` racine porte les mesures récentes. |
| `docs/DAEMON_STATE.md` | État du daemon | 2026-11-02 (mise à jour d'un rapport du 29-09) | daté | Daemon / ressources | L'en-tête distingue explicitement l'état courant vérifié du 2 novembre des sections historiques ; `STATE.md` racine est plus récent. |
| `docs/EXECUTION_STATUS.md` | État de reprise et prochaine tâche | 2026-11-04 | autorité | Pilotage des sessions | Fait foi pour « où en est la session et que faire ensuite » ; les exigences restent dans `FOUNDER_VISION.md`, les preuves dans `STATE.md`. |
| `docs/FOUNDER_VISION.md` | Registre exhaustif des exigences du fondateur | 2026-11-04 | autorité | Vision et exigences | Fait foi pour la liste des exigences et leur état 1–7 ; dérivé du corpus et des documents de vision, sans remplacer `VISION.md` pour le périmètre. |
| `docs/HERMES-FICHE-REPRISE-2026-08-12.md` | Fiche de reprise Hermes | 2026-08-12 | brouillon | Reprise / feuille de route | L'en-tête dit « Draft de travail » et précise que la fiche n'engage pas le projet sans confirmation ; `VISION.md` racine prime. |
| `docs/HIDE-SPEC.md` | Spécification Hide | 2026-08-09 | brouillon | Proxy Hide / SOCKS5 | Spécification de réintroduction ; le périmètre livré doit être confronté à `ETAT-REEL.md` et au code avant toute promesse produit. |
| `docs/P0-DEUX-MACHINES.md` | Validation P0 sur deux machines physiques | 2026-11-14 (exécutée le 19) | daté | Réseau / preuve physique | Protocole messagerie + Compute ; exécuté le 2026-11-19 sur Hope ↔ cue-sandbox (WAN, deux noyaux distincts) ; tableau rempli dans la preuve datée ; LAN/NAT non testés. |
| `docs/proofs/P0-DEUX-HOTES-2026-11-19.md` | Preuve P0 deux hôtes — Hope ↔ cue-sandbox | 2026-11-19 | daté | Réseau / preuves d'exécution | Exécution réelle : identités distinctes, AUTH_OK, message 4/7 fragments, Compute E2E + L1 persisté, rejeu/clair/usurpation refusés sur le fil, coupure sans faux succès, rotation + réappairage, confidentialité vérifiée sur les deux jambes (0 chaîne interdite, 0 clé privée). Limites : LAN/NAT, pcap non rapatrié, révocation locale. |
| `docs/P0-COMPUTE-SECURITY-2026-11-14.md` | Sécurité Compute P0 — E2E, sandbox, vérification | 2026-11-14 (corrigé le 15) | daté | Sécurité / Compute | Instantané des preuves : chiffrement ML-KEM/AES-GCM des tâches et grants, 12 tests sandbox réels, plafond mémoire par cgroup (`MemorySwapMax=0`) + `RLIMIT_AS`, quota CPU mesuré (4,2x), vérification L1 opt-in ; validation physique et quorum non faits. |
| `docs/adr/0001-authentification-du-relais.md` | ADR-0001 — Authentification du relais | 2026-11-09 | autorité | Sécurité / décision d'architecture | Décision acceptée et implémentée : identité dérivée de la clé de signature, preuve de possession KEM + ML-DSA, slot non volable, alternatives écartées et conséquences négatives (graphe visible, rotation = changement d'identité) déclarées. |
| `docs/decisions/ADR-0001-base-canonique-et-reconciliation.md` | ADR-0001 — Base canonique et réconciliation des lignées (dépôt) | 2026-11-09 | autorité | Décision d'architecture / historique Git | Tranche la décision ouverte `D1` : base `Polygone-real/main`, contenu `realign/v2` rejoué, annexe listant les 65 conflits `add/add` et leur arbitrage. Preuve : `comm -23` sur les deux arbres, 0 fichier perdu. |
| `docs/decisions/ADR-0002-identite-noeud-cle-signature.md` | ADR-0002 — Identité de nœud : clé de signature ML-DSA-65 (D2) | 2026-11-19 | autorité | Sécurité / décision d'architecture | Tranche `D2` et remplace `DEC-SEC-03` : `node_id` dérivé de la clé ML-DSA-65 par BLAKE3 avec contexte (32 hex). Confirme `docs/adr/0001`. Rotation = nouvelle adresse, révocation locale ; migration sans perte (`peers.json.v1.bak`). |
| `docs/decisions/ADR-0003-preuve-l1-persistee.md` | ADR-0003 — Preuve L1 persistée (`polygone.evidence.v1`) | 2026-11-19 | autorité | Compute / format persisté | Chaque vérification `--verifier-local` (accord ou divergence) écrit `~/.polygone/evidence-<ts>-<digest8>.json`, canonique, atomique 0600. Engagement = sortie signée du producteur ; observation = recalcul local. Aucun champ réseau ajouté. Tests `net::tests::l1_evidence_*`. |
| `docs/decisions/ADR-0004-reconciliation-github-main.md` | ADR-0004 — Réconciliation `github/main` ↔ `reconcile/2026-11` | 2026-11-19 | autorité | Décision d'architecture / historique Git | Rejeu inspecté de la règle ADR-0001 : tout fichier préexistant garde la version canonique (20 conflits + 11 auto-fusions arbitrés un par un, table complète), 24 ajouts purs de septembre conservés comme couche historique, 2 fichiers mock exclus. Aucun code Rust modifié par la fusion. Retour arrière documenté. |
| `docs/MISSION-P0-RAPPORT-2026-11-09.md` | Mission P0 — rapport de preuves | 2026-11-09 | daté | Réseau / preuves d'exécution | Instantané d'une exécution : 450 tests verts, parcours réel à trois processus, et liste explicite de ce qui n'a pas été vérifié (deux machines physiques, NAT, version de protocole). |
| `docs/COMPANY.md` | Company OS — vision d'organisation (15 rôles) | 2026-09-10 | daté | Gouvernance / historique | Instantané de septembre (lignée `github/main`) : org chart + RACI + KPIs en agents. Couche historique conservée par ADR-0004 ; ne décrit pas la gouvernance actuelle. |
| `docs/RAPPORT-PRE-SHIP-2026-09-10.md` | Rapport pré-ship du 2026-09-10 | 2026-09-10 | daté | Archive / historique | Vérifications et diffs proposés de la lignée septembre, sans push. Couche historique conservée par ADR-0004 ; compteurs et verdicts datés. |
| `docs/P0-RAPPORT.md` | Rapport de preuves P0 | 2026-11-03 | daté | Archive / inventaire historique | Copie d'un rapport d'exécution daté ; il précise lui-même ce que l'archive prouve et ce qu'elle ne prouve pas. |
| `docs/PLAN_BLOC_HASH.md` | Plan du magasin de blocs adressés par hash | Non indiquée | brouillon | Stockage distribué | Le texte est un plan de preuve à deux machines, pas la preuve que le transfert inter-hôtes fonctionne ; voir tâche `BLOCKS.9`. |
| `docs/PLAN_CONSOMMATEUR.md` | Plan du consommateur d'allocation | 2026-09-29 | daté | Daemon / allocation | L'en-tête précise qu'il s'agit d'un incrément sans code ; `PLAN_CONSOMMATEUR_MAJ.md` le remplace sur les points qu'il énumère. |
| `docs/PLAN_CONSOMMATEUR_MAJ.md` | Mise à jour du plan consommateur | 2026-09-30 | daté | Socket / allocation | L'en-tête le présente comme une mise à jour partielle du document précédent ; c'est un état de conception, pas une description actuelle du code. |
| `docs/PLAN_FREIN.md` | Plan de décision sur l'activité utilisateur | Non indiquée (incrément 2b-4a) | brouillon | Ressources / activité utilisateur | Le texte dit « plan de décision, aucun code écrit » ; les contrats actuels sont décrits dans `ETAT-REEL.md`. |
| `docs/PLAN_INTEGRATION.md` | Plan d'intégration du socket | Non indiquée (incrément 2b-3a) | brouillon | Daemon / IPC | L'en-tête indique « plan seulement, aucun code écrit » ; le résultat livré doit être vérifié dans le code du daemon. |
| `docs/PLAN_PRIORITES_2B5.md` | Plan de priorisation 2b-5 | 2026-09-29 (date des mesures citées) | daté | Cgroups / allocation | Le document se déclare plan sans code et s'appuie sur des mesures datées d'un binaire précis ; il ne décrit pas seul l'implémentation actuelle. |
| `docs/PLAN_REPRISE-2026-10-03.md` | Plan de reprise et état extrait | 2026-10-03 | daté | Reprise des dépôts | L'en-tête borne les observations à cette journée et affirme qu'aucun code n'a été modifié. |
| `docs/PLAN_SOCKET.md` | Plan du protocole socket daemon | Non indiquée | brouillon | Daemon / IPC | L'en-tête dit « plan seul, aucun code écrit » ; le contrat réel est à vérifier dans `daemon/src/socket.rs`. |
| `docs/PROTOCOL-RELAY-AUTH.md` | Protocole d'authentification du relais | 2026-11-09 | autorité | Sécurité / handshake relais | Fait foi pour le handshake `AUTH_INIT`/`CHALLENGE`/`AUTH_RESP`/`AUTH_OK` et la dérivation `node_id = BLAKE3(sign_pk)` ; chaque propriété renvoie au test qui la prouve, et les limites (pas de version de protocole, rotation = changement d'identité) sont déclarées. |
| `docs/PREMIER-SOIR.md` | Protocole Premier Soir | 2026-08-08 (modèle v1 ; carnet daté du 2026-09-02) | brouillon | Parcours utilisateur | Le document présente un protocole et un modèle à remplir ; il ne constitue pas à lui seul une preuve d'un test utilisateur indépendant. |
| `docs/RAPPORT-FINAL-COMPLET-2026-08-12.md` | Rapport final de mission | 2026-08-12 (nom du fichier) | daté | Historique de mission | Rapport de clôture historique ; ses formulations de mission accomplie ne remplacent pas les preuves actuelles du code. |
| `docs/RAPPORT-IDEES-MINEES-2026-08-12.md` | Rapport d'idées extraites | 2026-08-12 (nom du fichier) | daté | Historique / idées | Rapport rétrospectif daté ; aucune idée listée n'est réputée livrée sans appelant et preuve actuels. |
| `docs/RAPPORT-MISSION-2026-08-12.md` | Rapport de mission | 2026-08-12 (nom du fichier) | daté | Historique de mission | Rapport historique ; les anciens nombres de crates/tests sont contredits par l'état mesuré actuel. |
| `docs/RECONCILIATION.md` | Réconciliation de l'historique Git | 2026-11-03 | daté | Git / branches divergentes | L'en-tête borne l'audit au 3 novembre et précise qu'aucun merge, reset ou push n'a été exécuté. |
| `docs/REPRISE-ET-EXTRACTION-2026-11-02.md` | Reprise et extraction historique | 2026-11-02 | daté | Sources historiques / reprise | Le document déclare l'extraction en lecture seule et identifie les sources et dates consultées ; il décrit un instantané. |
| `docs/SERVERLESS-SPEC.md` | Spécification Serverless | Non indiquée | brouillon | Exécution distribuée | La spécification décrit des comportements partiels et des fonctionnalités non branchées ; `ETAT-REEL.md` fait foi sur l'état livré. |
| `docs/SESSION-REPORT-2026-08-06.md` | Rapport de session | 2026-08-06 | daté | Historique des sessions | Le titre et le contenu datent la session ; les versions et comptes de tests ne sont pas des mesures actuelles. |
| `docs/SESSION-REPORT-2026-08-08.md` | Rapport de session architecte | 2026-08-08 | daté | Historique des sessions / décisions | Le rapport décrit une session passée ; les décisions courantes doivent être lues dans `VISION.md`, `STATE.md` et les décisions datées ultérieures. |
| `docs/STRATEGIE.md` | Stratégie de Polygone | 2026-08-06 | daté | Vision historique / stratégie | L'en-tête identifie une consolidation du 6 août ; la vision de référence actuelle est `VISION.md` à la racine. |
| `docs/VISION-COMPLETE.md` | Vision unifiée et état vérifié | Non indiquée | daté | Vision historique / état produit | Cette vision antérieure est explicitement remplacée dans `VISION.md` racine ; l'état factuel relève de `ETAT-REEL.md`. |
| `docs/VISION-ET-PLAN-DE-FINALISATION-2026-11-02.md` | Vision consolidée et plan de finalisation | 2026-11-02 | daté | Reprise / planification | Le document se présente comme une analyse de reprise du 2 novembre ; les décisions confirmées depuis sont dans les fichiers racine et l'Atlas. |
| `docs/VISION_CANONIQUE.md` | Ancienne vision canonique | Non indiquée | daté | Vision historique | `VISION.md` racine la nomme parmi les visions contradictoires qu'il remplace ; elle est conservée comme trace. |
| `docs/atlas/README.md` | Atlas d'ingénierie Polygone | 2026-11-04 (import) | autorité | Navigation dans l'Atlas | Point d'entrée du corpus Atlas ; il distingue les instantanés et renvoie vers les sources de statut actuelles. |
| `docs/atlas/STATUS-2026-11-04.md` | État de synchronisation Atlas | 2026-11-04 | daté | Portage documentaire | Rapport d'état daté du portage ; son statut est celui de cette synchronisation, pas une vérité intemporelle. |
| `docs/atlas/agent/FREEBUFF-FORK.md` | Guide de branchement Freebuff | 2026-11-04 (import) | brouillon | Orchestration / runner agent | Guide d'intégration ; `agentCommand` reste désactivé et aucun worker réel n'est déclaré actif. |
| `docs/atlas/agent/MISSION-FINALISATION-PRODUIT.md` | Mission de finalisation produit | 2026-11-04 (import) | brouillon | Mission / critères de livraison | Brief de mission, pas preuve d'exécution ; les tâches et leurs preuves sont suivies dans le backlog opérationnel. |
| `docs/atlas/agent/ORDRE.md` | Constitution de l'agent Polygone | 2026-11-04 (import) | autorité | Règles de travail des agents | Référence normative pour les missions d'agent dans ce dépôt ; ne remplace ni `VISION.md` ni les décisions humaines. |
| `docs/atlas/agent/README.md` | Mode d'emploi de l'orchestrateur | 2026-11-04 (import) | autorité | Utilisation de l'orchestrateur | Référence locale d'utilisation ; elle indique que l'orchestrateur est séquentiel et que le runner reste désactivé. |
| `docs/atlas/audit/INVENTAIRE.md` | Inventaire des quinze versions | 2026-11-04 (import) | daté | Historique du projet | Inventaire historique de versions et de sources ; ne certifie pas que tous les artefacts privés ont été intégrés. |
| `docs/atlas/ecosystem/HOPE-GATEWAY.md` | Note d'aménagement Hope Gateway | 2026-11-04 (import) | brouillon | Écosystème / passerelle IA | Note de conception externe ; le document précise que ce rapport n'est pas du code d'exécution intégré à Polygone. |
| `docs/atlas/plan/DECISION-POLY.md` | Décision de cadrage POLY | 2026-11-04 (import) | autorité | Décision économique / réciprocité | Décision de cadrage retenue pour ce dépôt : POLY n'est pas une cryptomonnaie ; les autres éléments restent conditionnés aux portes listées. |
| `docs/atlas/plan/epiques/01-transport-confidentialite.md` | Épiques transport et confidentialité | 2026-11-04 (import) | brouillon | Transport / confidentialité | Spécifications de tâches futures ; elles ne valent pas preuve d'un test réseau entre hôtes physiques. |
| `docs/atlas/plan/epiques/02-stockage.md` | Épiques stockage | 2026-11-04 (import) | brouillon | Stockage / Drive / Blocks | Plan de tâches ; le transfert physique entre deux machines reste une preuve ouverte (`BLOCKS.9`). |
| `docs/atlas/plan/epiques/03-calcul-ia.md` | Épiques calcul et IA | 2026-11-04 (import) | brouillon | Daemon / calcul / IA | Plan de tâches ; les décisions D2 et mesures d'isolation qui bloquent certaines étapes restent ouvertes. |
| `docs/atlas/plan/epiques/04-interfaces.md` | Épiques interfaces | 2026-11-04 (import) | brouillon | CLI / Gateway / UI | Plan de fonctionnalités ; le statut de chaque interface doit être confirmé par une preuve de code ou d'exécution. |
| `docs/atlas/plan/epiques/05-livraison.md` | Épiques livraison | 2026-11-04 (import) | brouillon | Installation / SDK / CI | Plan de livraison ; les éléments ne sont pas réputés achevés par leur seule présence dans l'épic. |
| `docs/atlas/plan/epiques/06-confiance.md` | Épiques confiance et observabilité | 2026-11-04 (import) | brouillon | Sécurité / documentation / observabilité | Spécifications et dépendances de tâches ; statut à actualiser par preuves, notamment DOC.2 et DOC.3. |
| `docs/atlas/plan/HISTORIQUE-GIT.md` | Historique Git et diagnostic des lignées | 2026-11-03 | daté | Historique Git | Le document indique sa date de mesure et précise qu'aucune réécriture d'historique n'a été exécutée. |
| `docs/atlas/plan/PLAN-ECOSYSTEME.md` | Plan de l'écosystème | 2026-11-03 | brouillon | Dépendances de l'écosystème | Complément de planification ; les statuts exécutés et bloqués sont ceux du backlog et de `STATE.md`. |
| `docs/atlas/plan/PLAN-FINITION.md` | Plan de finition | 2026-11-03 | autorité | Portée des 132 tâches épiques | Référence de spécification des tâches ; l'état opérationnel courant reste dans `docs/atlas/agent/backlog.json`. |
| `docs/atlas/plan/PLAN-MAITRE.md` | Plan maître historique | 2026-11-04 (import) | daté | Plan directeur historique | Sa note de portage le qualifie de plan directeur historique et renvoie vers les documents d'état actuels. |
| `docs/atlas/plan/preuves-poly/ARCHIVE-CODE.md` | Preuves du code POLY archivé | 2026-11-04 (import) | daté | Historique / preuves de code | Dossier d'éléments historiques ; il n'active aucune fonctionnalité POLY dans le produit courant. |
| `docs/atlas/plan/preuves-poly/CONTEXTE-DOCS.md` | Évolution documentaire autour de POLY | 2026-11-04 (import) | daté | Historique des décisions POLY | Compare des documents historiques datés ; la décision actuelle est résumée dans `DECISION-POLY.md`. |
| `docs/atlas/plan/preuves-poly/MAQUETTE-TUI.md` | Maquette TUI historique | 2026-11-04 (import) | daté | Historique UI / POLY | Archive une maquette et ses promesses ; une maquette n'est pas une preuve d'écran livré. |
| `docs/atlas/plan/preuves-poly/README.md` | Index des preuves POLY | 2026-11-04 (import) | autorité | Navigation dans le dossier de preuves | Index local du dossier de preuves ; il décrit les pièces du corpus, pas l'état fonctionnel du produit. |
| `docs/cli.md` | Référence CLI | Non indiquée (version v2.0.0-rc2) | daté | Commandes CLI | La référence est explicitement rattachée à une ancienne version candidate ; elle doit être reconfrontée à `crates/client/src/main.rs`. |
| `docs/config.md` | Référence de configuration | Non indiquée (version v2.0.0-rc2) | daté | Configuration | Le document se rattache à v2.0.0-rc2 et décrit aussi des restes v1 ; la configuration réelle doit être vérifiée dans le code. |
| `docs/deployment/README.md` | Guide de déploiement Ghost Node | Non indiquée | brouillon | Déploiement / Ghost Node | Le guide décrit des plateformes et commandes à valider ; aucun déploiement réel n'est cité comme preuve de fonctionnement. |
| `docs/kill-switch.md` | Documentation du mode Duress | 2026-08-06 (version 1.0) | daté | Sécurité / Duress | La version est explicitement datée ; `ETAT-REEL.md` décrit la portée réelle et distingue le déclenchement manuel des déclencheurs non câblés. |
| `docs/letters/ANSSI.md` | Lettre à l'ANSSI | 2026-09-02 | brouillon | Correspondance externe / sécurité | Lettre datée ; le dépôt ne contient pas de preuve d'envoi ou de réponse, donc elle n'est pas classée comme communication envoyée. |
| `docs/letters/CNIL.md` | Lettre à la CNIL | 2026-09-02 | brouillon | Correspondance externe / données | Lettre datée ; le dépôt ne contient pas de preuve d'envoi ou de réponse, donc elle n'est pas classée comme communication envoyée. |
| `docs/letters/EFF.md` | Lettre à l'EFF | 2026-09-02 | brouillon | Correspondance externe / confidentialité | Lettre datée ; le dépôt ne contient pas de preuve d'envoi ou de réponse, donc elle n'est pas classée comme communication envoyée. |
| `docs/observation-premier-soir.md` | Carnet d'observation Premier Soir | 2026-09-02 (date renseignée dans le carnet) | brouillon | Validation utilisateur | Le texte conserve un modèle « à remplir au premier soir réel » ; aucune preuve ne confirme que l'observation a été réalisée par un utilisateur indépendant. |
| `docs/THREAT-MODEL-SERVICES.md` | Modèle de menace par service | 2026-11-05 | brouillon | Sécurité / modèle de menace | Auto-évaluation SEC.1 : chaque contrôle cite un test ou un chemin de code ; aucun audit externe, les risques résiduels sont listés et `FV-SEC-08` reste à l'état 5. |
| `docs/threat-msg.md` | Métadonnées visibles par le relais | 2026-11-05 | daté | Sécurité / métadonnées réseau | Mesure SEC.10 sur un flux réel : source, destination, taille, cadence, session visibles ; contenu et nom de fichier absents ; limites loopback explicites. |
| `docs/threat-commodity.md` | Modèle de menace utilisateur quotidien | Non indiquée | brouillon | Modèle de menace courant | Document à réconcilier avec les limites mesurées de `ETAT-REEL.md` ; les modèles par service de SEC.1 sont désormais dans `THREAT-MODEL-SERVICES.md`. |
| `docs/threat-high-value.md` | Modèle de menace utilisateur haute-valeur | Non indiquée | brouillon | Modèle de menace haute-valeur | Document à réconcilier avec les limites mesurées de `ETAT-REEL.md` ; les modèles par service de SEC.1 sont désormais dans `THREAT-MODEL-SERVICES.md`. |
| `docs/proofs/modal-two-host-20261110T214504Z.md` | Preuve réseau à deux hôtes — 20261110T214504Z | 2026-11-10 | daté | Preuve réseau / deux hôtes | Exécution datée de `scripts/p0-modal-two-host.sh` avec journaux bruts joints ; elle prouve un échange entre deux hôtes à cet instant, pas l'état courant du produit. |
| `docs/proofs/modal-three-kernels-20261110T215027Z.md` | Preuve réseau à trois noyaux — 20261110T215027Z | 2026-11-10 | daté | Preuve réseau / trois noyaux | Exécution datée de `scripts/p0-modal-three-kernels.sh` avec journaux bruts joints ; instantané de la topologie testée, sans valeur de contrat permanent. |
| `docs/proofs/modal-reconnect-20261110T215246Z.md` | Coupure et reprise en pleine session — 20261110T215246Z | 2026-11-10 | daté | Preuve réseau / reconnexion | Exécution datée de `scripts/p0-modal-reconnect.sh` ; mesure la reprise après coupure sur ce montage précis, pas la reconnexion inter-hôtes générale. |
| `docs/proofs/modal-relay-restart-20261110T215622Z.md` | Redémarrage du relais en pleine session — 20261110T215622Z | 2026-11-10 | daté | Preuve réseau / relais | Exécution datée de `scripts/p0-modal-relay-restart.sh` ; le tunnel public est conservé, seul le processus du relais est remplacé. |
| `docs/proofs/modal-relay-restart-20261110T215706Z.md` | Redémarrage du relais en pleine session — 20261110T215706Z | 2026-11-10 | daté | Preuve réseau / relais | Réexécution datée du même scénario de redémarrage du relais ; consignée séparément pour ne pas écraser la preuve précédente. |
| `docs/proofs/modal-relay-restart-20261110T221117Z.md` | Redémarrage du relais en pleine session — 20261110T221117Z | 2026-11-10 | daté | Preuve réseau / relais | Troisième exécution datée du scénario de redémarrage du relais, avec ses propres journaux ; instantané, pas état courant. |
| `docs/proofs/modal-relay-restart-20261110T221636Z.md` | Redémarrage du relais en pleine session — 20261110T221636Z | 2026-11-10 | daté | Preuve réseau / relais | Quatrième exécution datée du scénario de redémarrage du relais ; conservée comme trace horodatée indépendante. |

## Contrôle

Le test d'intégration Rust `doc_index` parcourt récursivement `docs/`, vérifie
que chaque fichier Markdown apparaît exactement une fois, refuse les statuts
inconnus ou vides, et exige un titre, une date déclarée (ou « Non indiquée »),
un thème et une justification. Il est lancé par :

```bash
cargo test -p polygone-client --test doc_index
```
