# FOUNDER_VISION — registre exhaustif des exigences

> **But** : ne plus jamais perdre une ambition du fondateur, et ne jamais
> déclarer « fait » ce qui n'a pas de preuve exécutée.
>
> **Sources** (lues, pas supposées) : `_work/vision-long.md` (26 798 lignes de
> corpus, avril–août 2026), `_work/corpus/user-87.md`, `00-SOURCES/VISION-EXTRAITE.md`,
> `VISION.md` (document d'autorité), `ECOSYSTEM.md`, `SPEC.md`, `ROADMAP.md`,
> `NEEDS-LEVY.md`, `DECISIONS.md`, `02-PLAN/PLAN-MAITRE.md`,
> `docs/atlas/plan/PLAN-FINITION.md` (132 tâches), `docs/atlas/plan/DECISION-POLY.md`.
>
> **États** (chiffre entre 1 et 7, seule autorité pour l'avancement) :
> 1 non étudiée · 2 à concevoir · 3 à implémenter · 4 en cours ·
> 5 implémentée mais non vérifiée · 6 testée et validée · 7 bloquée.
>
> Une classe, une route ou un binaire qui existe ne vaut pas « 6 ». Une
> exigence n'est **6** que si la preuve citée a été exécutée et reproduite.
> Le pourcentage est calculé par `node scripts/vision-status.mjs`, jamais à la main.

## A. Infrastructure décentralisée

| ID | Exigence | État | Preuve / réalité vérifiée | Travail restant | Critère d'acceptation | Priorité |
|---|---|---|---|---|---|---|
| FV-INF-01 | Identité locale post-quantique, sans compte | 6 | `crates/client/src/identity.rs`, `~/.polygone/identity.json` chmod 600, tests core | Rotation/révocation de clé absente (FV-SEC-05) | `polygone clef` et `id` produisent une identité stable, aucun réseau requis | P0 |
| FV-INF-02 | Transport chiffré E2E, relais aveugle sur le contenu | 6 | `crates/core` (ML-KEM-1024, AES-GCM), `polygone-relay`, tests SEC.3/SEC.4 | Audit externe (FV-SEC-08) | Démo E2E : 4/7 fragments, signature vérifiée, relais sans clair | P0 |
| FV-INF-03 | Découverte de pairs sur le LAN | 6 | `mesh.rs`, UDP 7642, MESH.1 refuse les interfaces VPN/loopback | Découverte WAN (DHT) absente | Deux processus sur le LAN se trouvent sans configuration | P1 |
| FV-INF-04 | Communication entre deux machines physiques | 7 | **Bloquée** : BLOCKS.9 exige un second hôte ; les tests prouvent des processus locaux | Préparer le script, puis exécuter sur deux hôtes réels | Journal horodaté avec deux hôtes, transfert comparé octet à octet | P0 |
| FV-INF-05 | Fonctionnement sans serveur central obligatoire | 5 | LAN + relais auto-hébergeable ; aucun bootstrap central dans le code | Prouver deux hôtes sans relais tiers (FV-INF-04) | Message direct LAN entre deux hôtes, sans service central | P0 |
| FV-INF-06 | Tolérance aux pannes et reprise | 6 | `scripts/crash-recovery.mjs` 9/9 (SIGKILL daemon, bail, redémarrage) | Panne de relais en cours de session non testée de bout en bout | Un nœud tué ne bloque aucune ressource au-delà de son bail | P1 |
| FV-INF-07 | Scalabilité du routage | 3 | Relais en mémoire, plafond 16 connexions ; aucune mesure de charge | Banc de charge et backpressure mesurés | Débit et latence publiés pour N pairs, sans invention | P2 |
| FV-INF-08 | Synchronisation distribuée / horloge entre nœuds | 7 | `time_sync` archivé par D9 (1 019 LOC, zéro consommateur) | Décider réintroduction avec un vrai protocole | Décision écrite puis protocole testé, ou maintien de l'archive | P3 |
| FV-INF-09 | Résilience aux réseaux réels (NAT, pertes, reconnexion) | 3 | Aucun test NAT/WAN ; reconnexion locale seulement | Scénarios NAT, coupure, reprise | Test WAN documenté, latence et pertes mesurées | P1 |
| FV-INF-10 | Pas de dépendance à un relais unique | 4 | Relais remplaçable par configuration ; un seul à la fois dans les tests | Bascule automatique entre relais | Perte du relais courant → bascule vers un second, message livré | P1 |

## B. Économie des Polygones et réciprocité

| ID | Exigence | État | Preuve / réalité vérifiée | Travail restant | Critère d'acceptation | Priorité |
|---|---|---|---|---|---|---|
| FV-ECO-01 | Contribution volontaire rémunérée en crédits internes | 7 | **Bloquée par D8** : DECISION-POLY interdit tout crédit avant service distribué prouvé | Après FV-INF-04 : reçus mesurés, limites de dette, contestation | Un crédit ne naît que d'un service vérifié, jamais d'une intention | P2 |
| FV-ECO-02 | Comptabilisation transparente et auditable | 3 | Ledger d'allocations (`daemon/src/socket.rs:738`) ≠ ledger de crédits ; aucun solde utilisateur | Concevoir le registre de crédits avec reçus signés | Chaque crédit référence un reçu vérifiable et un service mesuré | P2 |
| FV-ECO-03 | Utilisation des crédits pour accéder aux ressources | 7 | Bloquée par FV-ECO-01/02 | Définir le prix, la dette maximale, la révocation | Consommation débitée sur reçu, solde consultable | P3 |
| FV-ECO-04 | Prévention du double comptage et de la fraude | 3 | Aucun registre de crédits ; anti-rejeu des messages existe (SEC.3) | Idempotence des reçus, unicité par service | Rejeu du même reçu refusé, compteur inchangé | P2 |
| FV-ECO-05 | Pas de paiement obligatoire pour participer | 6 | Aucun compte, aucun abonnement, AGPL ; tout le produit actuel est gratuit | Maintenir cette contrainte dans chaque service | Toutes les commandes fonctionnent sans carte ni compte | P0 |
| FV-ECO-06 | Marketplace de ressources | 7 | **Hors périmètre** tant que le palier gratuit n'est pas prouvé (VISION §6/§8) | Aucun travail avant décision explicite | Décision du fondateur écrite avant toute conception | P4 |
| FV-ECO-07 | Jeton transférable / cryptomonnaie POLY | 7 | **Rejeté** : DECISION-POLY, VISION §6 (pas de blockchain, pas de spéculation) | Aucun ; trace conservée dans `salvage/` | Reste rejeté tant que D8 n'est pas rouvert | P4 |
| FV-ECO-08 | Équilibrage contributeurs / consommateurs | 3 | Aucune mesure de contribution consolidée | Modèle de réciprocité borné, sans transfert entre users | Simulation sur reçus réels, pas de monnaie | P3 |

## C. Infrastructure partagée et calcul

| ID | Exigence | État | Preuve / réalité vérifiée | Travail restant | Critère d'acceptation | Priorité |
|---|---|---|---|---|---|---|
| FV-COMP-01 | Mesure réelle CPU/RAM/GPU/stockage | 6 | `SystemSnapshot`, CLI.1 (`doctor` lit cgroup/memory.max/cpu.max) | GPU non mesuré sur toutes plateformes | `doctor` distingue mesuré / réservable / contraint, jamais un zéro inventé | P0 |
| FV-COMP-02 | Plafonds appliqués par l'OS, pas seulement calculés | 5 | `--isoler` crée un scope systemd/cgroup v2 vérifié ; sans lui, auto-imposé | `--isoler` par défaut = **bloqué D2** | Charge contrainte par cgroup, scope déchargé après | P0 |
| FV-COMP-03 | Bail avec expiration et restitution | 6 | DAEMON.1/DAEMON.3/DAEMON.13, 15 min, renew, crash test 9/9 | — | Bail expiré libère la marge, aucun blocage | P0 |
| FV-COMP-04 | Attribution et orchestration des tâches | 3 | Aucun scheduler de tâches ; `serverless` ne contacte aucun nœud | Concevoir file, priorités, reprise, vérification | Une tâche distribuée revient avec un résultat vérifiable | P1 |
| FV-COMP-05 | Exécution isolée (shell/WASM) | 5 | `exec.rs` systemd-run + wasmi, `compute --executer/--wasm` | Vérifier les limites réelles sous charge, pas seulement le lancement | Une tâche ne lit ni n'écrit hors de son sandbox | P1 |
| FV-COMP-06 | Vérification des résultats de calcul | 3 | `calc.rs` cache BLAKE3(version‖entrée) ; pas de quorum réseau | Brancher le quorum serverless sur le transport | Résultat divergent rejeté, quorum ≥ 4 nœuds distincts | P1 |
| FV-COMP-07 | Reprise des tâches interrompues | 3 | Aucun ordonnanceur persistant | Journal de tâches + reprise idempotente | Nœud tué en cours de tâche → reprise sans double effet | P2 |
| FV-COMP-08 | Mesure de performance publiée | 5 | `polygone-bench` existe dans le corpus, non branché ici | Brancher un banc reproductible | Débit/latence mesurés avec matériel et conditions | P3 |
| FV-COMP-09 | Quotas et priorités par utilisateur | 3 | Plafonds par bail ; pas de priorité multi-utilisateur | Politique de priorité documentée | Deux consommateurs concurrents servis selon une règle écrite | P3 |

## D. Intelligence artificielle distribuée

| ID | Exigence | État | Preuve / réalité vérifiée | Travail restant | Critère d'acceptation | Priorité |
|---|---|---|---|---|---|---|
| FV-IA-01 | Inférence IA locale, zéro cloud | 6 | `petals.rs` → Ollama local, tests | Modèles absents gérés sans invention | `polygone petals ask` répond sans réseau cloud | P1 |
| FV-IA-02 | Inférence distribuée entre pairs | 3 | Planner de pipeline sans consommateur ; aucun transfert de poids | Transport de requêtes + shards | Deux nœuds produisent une réponse, résultat attribuable | P2 |
| FV-IA-03 | Orchestration de fournisseurs et de nœuds | 5 | `hope-gateway` hors de ce dépôt : rotation, quotas, fallback testés (44 tests) | Intégration produit non branchée | Une demande est routée selon capacité et quota mesurés | P3 |
| FV-IA-04 | Compatibilité multi-modèles | 5 | Ollama accepte un nom de modèle ; pas de matrice testée | Matrice de modèles et capacités | Au moins deux modèles testés, échec explicite sinon | P3 |
| FV-IA-05 | Concepts SOMA / JEPA (vision historique) | 1 | Mentionnés dans le corpus (`_work/vision-long.md`) ; **aucun code, aucune spec** | Étudier la correspondance avant toute implémentation | Note de conception validée, ou abandon motivé ; jamais présenté comme fait | P4 |
| FV-IA-06 | Protocole msh / échange de poids et modèles | 3 | `MSH_SPEC.md` dans le corpus, non implémenté | Spécifier puis tester un échange de shards | Deux nœuds échangent un artefact vérifié par hash | P3 |

## E. Confidentialité et sécurité

| ID | Exigence | État | Preuve / réalité vérifiée | Travail restant | Critère d'acceptation | Priorité |
|---|---|---|---|---|---|---|
| FV-SEC-01 | Chiffrement post-quantique de bout en bout | 6 | ML-KEM-1024, ML-DSA-65, AES-256-GCM, BLAKE3 ; self-test et tests workspace | Audit externe (FV-SEC-08) | Message lisible seulement avec la clé privée du destinataire | P0 |
| FV-SEC-02 | Contenu invisible au relais, métadonnées assumées | 6 | Relay route `to`, ne lit pas le payload ; noms de fichiers chiffrés ; docs threat | Réduire la corrélation (pseudonymes de session) | Journal du relais sans clair, sans nom de fichier, sans clé | P0 |
| FV-SEC-03 | Anti-rejeu des messages | 6 | SEC.3 : rejeu refusé par le vrai relais (`REPLAY.SESSION_ALREADY_COMPLETED`, `STALE_TIMESTAMP`) | — | Rejeu immédiat et tardif refusés avec code explicite | P1 |
| FV-SEC-04 | Isolation des tâches et permissions minimales | 5 | systemd-run + wasmi ; isolation OS seulement avec `--isoler` | Défaut sûr (D2), vérification des accès fichiers | Tâche sandboxée ne peut pas lire `$HOME` | P0 |
| FV-SEC-05 | Gestion sécurisée des identités (rotation, révocation) | 3 | Identité locale chmod 600 ; pas de rotation | Concevoir rotation et révocation | Ancienne clé révoquée ne signe plus, nouvelle acceptée | P2 |
| FV-SEC-06 | Résistance Sybil / usurpation | 5 | SEC.4 : `from` forgé refusé ; preuve de clé au handshake (D2 bench) | Coût Sybil mesuré, quotas par identité | Un pair ne peut pas usurper `from` ; coût d'identités documenté | P1 |
| FV-SEC-07 | Aucun accès arbitraire aux fichiers des participants | 6 | SEC.8 refuse les secrets suivis ; `identity.json` 600 ; sandbox | Audit des chemins d'écriture restants | Aucun secret dans git, aucune lecture hors périmètre | P0 |
| FV-SEC-08 | Modèle de menace par service + audit externe | 5 | SEC.1 : `docs/THREAT-MODEL-SERVICES.md` couvre client, relais, gateway, daemon, serverless, petals, avec tests cités et risques résiduels ; aucun audit externe | Audit indépendant | Un tiers peut contredire les promesses avec le document | P1 |
| FV-SEC-09 | Aucune télémétrie non consentie | 6 | Aucun appel analytics ; `config.md` documente zéro télémétrie | Maintenir dans chaque nouveau service | Aucune requête sortante hors pairs/relais choisis | P0 |
| FV-SEC-10 | Détection d'abus et rate limiting | 3 | Refus d'allocation comptés (OPS.2) ; pas de limite par pair | Limites par identité et par pair | Un pair abusif est limité, journalisé, sans déni de service global | P2 |

## F. Expérience utilisateur

| ID | Exigence | État | Preuve / réalité vérifiée | Travail restant | Critère d'acceptation | Priorité |
|---|---|---|---|---|---|---|
| FV-UX-01 | Un seul binaire `polygone`, commandes cohérentes | 6 | `crates/client`, CLI complète, 408 tests | — | Les parcours documentés s'exécutent avec ce binaire | P0 |
| FV-UX-02 | Tableau de bord 4 onglets (Accueil, Favoris, Services, Paramètres) | 3 | Maquette historique ; TUI 2 commandes ; UI.4 non livré | Concevoir puis câbler sur l'état réel | Navigation aux flèches, aucun chiffre inventé | P2 |
| FV-UX-03 | Solde et consommation visibles | 7 | Bloqué par FV-ECO-01 ; aucun solde réel | Après crédits réels | Solde = somme de reçus vérifiés, consommation mesurée | P2 |
| FV-UX-04 | Pause bornée 10 min → 4 h | 6 | CLI.2 : intention horodatée, bail rendu, reprise refusée si expirée, 7 tests | — | `pause`/`reprendre` observés sur le vrai daemon | P1 |
| FV-UX-05 | Profils d'usage compréhensibles avec impact | 6 | CLI.3 : 3 profils, config relue par `polygoned`, 4 tests, session réelle | — | `polygone profil` écrit des seuils distincts et les explique | P1 |
| FV-UX-06 | Désinstallation propre et confirmée | 6 | CLI.5 : énumération, `--confirmer`, racines seulement, 4 tests | Désinstaller le service systemd (INST.2) | `HOME` inchangé hors racines après désinstallation | P1 |
| FV-UX-07 | Installation en une commande, sans configuration | 5 | `install.sh` fail-closed ; INST.9 (VM neuve) non fait | Test sur VM neuve | Installation → premier message sans terminal avancé | P0 |
| FV-UX-08 | Mises à jour signées + rollback | 3 | Aucun mécanisme de mise à jour | Concevoir signature et rollback | Downgrade testé, artefact vérifié | P3 |
| FV-UX-09 | Statistiques et visualisation du réseau | 5 | Pages web décrivant l'état réel ; UI.1/UI.2/UI.5 ont retiré les chiffres inventés | Brancher sur heartbeat persisté (GW.1) | Les chiffres affichés viennent d'une source mesurée | P2 |
| FV-UX-10 | Accessible aux non-techniciens (GUI de référence) | 3 | CLI/TUI seulement ; PWA est une interface, pas un nœud | Concevoir la GUI sur l'API stable | Un non-technicien installe, appaire, envoie, met en pause | P1 |
| FV-UX-11 | Multiplateforme Linux/macOS/Windows | 4 | Linux testé ; macOS partiel ; Windows `compile_error!` volontaire | macOS isolation, Windows non déclaré | Support déclaré seulement avec runner vert | P3 |
| FV-UX-12 | Expérience mobile / téléphone-nœud | 7 | **Hors périmètre** : PWA interface seulement, pas nœud (VISION §8) | Décision du fondateur avant tout travail | Décision écrite, puis conception si GO | P4 |

## G. Exploitation, livraison, observabilité

| ID | Exigence | État | Preuve / réalité vérifiée | Travail restant | Critère d'acceptation | Priorité |
|---|---|---|---|---|---|---|
| FV-OPS-01 | Diagnostics actionnables | 6 | CLI.6 : `KNOWN_REASONS` couvre les 8 constantes du daemon ; chaque code → cause + action, code technique conservé ; `every_reason_code_has_an_action` échoue si un motif manque, `unknown_denial_keeps_the_technical_code` | — | Un refus affiche cause + action, code technique conservé | P1 |
| FV-OPS-02 | Compteurs de refus observables | 6 | OPS.2 : 8 motifs sur vrai serveur, `status`/`doctor`, « non mesurés » sans daemon | — | Chaque refus est compté une fois et lisible | P1 |
| FV-OPS-03 | CI honnête, multi-OS | 5 | CI.1 : Windows hors job bloquant, canary non bloquant ; run distant non exécuté | Lancer la CI réelle (push non autorisé) | Matrice verte sur runners, sans job décoratif | P2 |
| FV-OPS-04 | Releases vérifiables (SBOM, empreintes) | 3 | Aucune release signée ni SBOM | Concevoir la chaîne de release | `sha256sum -c` + SBOM reproductibles | P3 |
| FV-OPS-05 | Observabilité du réseau (heartbeat, latence) | 6 | GW.1 : heartbeat ML-DSA-65, clé épinglée, `last_seen` atomique dans `nodes.json`, rechargé au démarrage. GW.2 : ping/pong signé, challenge à usage unique (30 s), `latency_ms` écrit seulement après pong, médiane des seuls nœuds mesurés, vrai socket TCP et `/api/v1/network/status` = 42 | — | `last_seen` persiste après redémarrage ; latence mesurée ou `None`, jamais `0` | P1 |
| FV-OPS-06 | Documentation qui ne ment pas | 6 | `check-docs` (chiffres, bandeaux, preuves), tests négatifs | Maintenir à chaque lot | Un chiffre faux fait échouer la commande | P0 |
| FV-OPS-07 | Sauvegarde restaurable avant opération risquée | 6 | P0.1 archive + `cmp` ; Git propre exigé par STATE | Automatiser la vérification | Restauration testée, hash comparé | P0 |
| FV-OPS-08 | SDK et intégrations externes | 3 | API locale non gelée, aucun client externe | Geler l'API, écrire un client d'exemple | Un programme externe envoie/reçoit sans lire le code interne | P3 |

## H. Corrections de trajectoire historiques (à ne pas rouvrir par erreur)

| ID | Exigence | État | Preuve / réalité vérifiée | Travail restant | Critère d'acceptation | Priorité |
|---|---|---|---|---|---|---|
| FV-HIST-01 | Pas de « mieux que Tor » ni d'anonymat absolu | 6 | VISION §4, threat models, README corrigé | Maintenir la formulation | Aucune promesse sans modèle de menace | P0 |
| FV-HIST-02 | Pas d'OS Polygone / interface neuronale / IA biomimétique | 7 | VISION §8 ; démon biomimétique archivé (mesures en dur) | Décision du fondateur pour tout retour | Aucun code ressuscité sans décision | P4 |
| FV-HIST-03 | Un monorepo, pas 15 dossiers | 6 | Atlas + `03-REALIGNED` ; originaux non déplacés | — | Une seule source de vérité, archives lisibles | P0 |
| FV-HIST-04 | Pas de fausse finition (« ✅ sans preuve ») | 6 | `check-docs`, STATE, ce registre, hook SEC.8 | Maintenir | Toute case cochée cite une commande exécutée | P0 |

---

## Lecture de l'avancement

`node scripts/vision-status.mjs` compte les lignes ci-dessus et refuse toute
ligne mal formée. Il donne :

- le nombre d'exigences **validées** (état 6) sur le total ;
- l'avancement **réaliste** = validées / (total − bloquées) ;
- les exigences **bloquées** (état 7) avec leur justification dans la colonne
  « Preuve / réalité vérifiée ».

Le pourcentage n'est **pas** un calendrier. Une exigence passe de 5 à 6
uniquement quand sa preuve est exécutée dans une session et citée ici.
