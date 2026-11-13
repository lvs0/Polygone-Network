# FOUNDATION-ROADMAP — de l'audit à Polygone Foundation 1.0

> Dérivée de [`FOUNDATION-AUDIT.md`](./FOUNDATION-AUDIT.md), datée du
> **2026-11-09**, HEAD `e2936a3` + correctifs de persistance du ledger.
> Une tâche n'est `VERIFIED` que si son critère d'acceptation est satisfait par
> une preuve exécutée. « Le code compile » n'est jamais une preuve suffisante.
>
> Vocabulaire de statut : `NOT_STARTED`, `INVESTIGATING`, `IN_PROGRESS`,
> `BLOCKED`, `IMPLEMENTED`, `TESTED`, `VERIFIED`.

---

## 0. Déjà fait pendant l'audit

| ID | Tâche | Statut | Preuve |
|---|---|---|---|
| F0-AUD-01 | Cartographie du workspace, des binaires et des tests | VERIFIED | `FOUNDATION-AUDIT.md` §2–4 ; mesures `find`/`wc` |
| F0-AUD-02 | Audit des flux critiques (identité, réseau, ledger, gateway, exécution) | VERIFIED | `FOUNDATION-AUDIT.md` §3–8 ; lecture du code |
| F0-DOC-01 | Documentation de fondation (13 livrables) | IMPLEMENTED | ce dépôt, `project/`, `docs/architecture/`, `docs/security/`, `docs/testing/`, `product/` |
| F0-FIX-01 | Persistance atomique du ledger + permissions 0600 | TESTED | `test_ledger_persist_is_atomic_and_private` |
| F0-FIX-02 | Quarantaine d'un ledger illisible (pas de perte silencieuse) | TESTED | `test_corrupt_ledger_is_quarantined_not_silently_erased` |
| F0-FIX-03 | Commentaire `alloc_granted.expires_at` aligné sur le code | VERIFIED | `daemon/src/socket.rs`, `Reply::AllocGranted` |

---

## 1. P0 — ce qui bloque une fondation crédible

| ID | Domaine | Objectif | État vérifié | Fichiers | Dépend de | Changement attendu | Test / preuve | Critère d'acceptation | Statut | Risques |
|---|---|---|---|---|---|---|---|---|---|---|
| F1-PROOF-01 | Preuve réseau | Valider deux machines physiques distinctes | Aucun test inter-hôtes ; procédure prête | `docs/P0-DEUX-MACHINES.md`, `scripts/p0-two-host.sh` | identité + relais authentifié (faits) | Exécuter la procédure sur deux hôtes réels, remplir le tableau de preuves | Journal horodaté + transfert comparé + reconnexion | Les 10 étapes de la procédure passent sur deux hôtes | NOT_STARTED (BLOCKED : second hôte indisponible) | NAT, pare-feu, horloges |
| F1-PROOF-02 | CI | Prouver le pipeline sur le HEAD réel | Structure vérifiée localement, runner jamais exécuté | `.github/workflows/ci.yml` | autorisation de push | Lancer le workflow et archiver le résultat | URL du run + statut + journaux | `test` et `smoke` verts sur Linux | NOT_STARTED (BLOCKED : aucun push autorisé) | cache, runners, temps |
| F1-SEC-01 | Secrets | Rejouer Gitleaks sur l'arbre final | Preuve antérieure existe ; non rejouée dans ce lot | `docs/proofs/gitleaks-2026-11-09.txt` | binaire `~/bin/gitleaks` | Exécuter `git` + `dir` sur l'arbre courant | Sortie datée, aucune valeur sensible dans le rapport | 0 fuite sur l'arbre courant | NOT_STARTED | faux positifs, volume |
| F1-RES-03 | Ressources | Décider et appliquer la limite CPU du palier | **Livré** : `allowed_cpu_cores()` = `floor(cpu_cores × max_cpu_percent / 100)` ; `free_cpu_cores` s'y limite ; `cpu_cores` reste le total physique mesuré | `daemon/src/socket.rs`, `DaemonState` | — | Plafonner le pool CPU par palier, sans changer le champ de mesure | `test_cpu_pool_is_capped_by_the_tier`, `test_cpu_above_the_tier_pool_is_denied` | Comportement unique, testé, documenté | **TESTED** | un palier bas réduit le nombre de cœurs partageables (1 cœur sur 3 physiques à 50 %) |
| F1-PROTO-01 | Protocole | Versionner et négocier les capacités | **Livré** : fenêtre `v1..=v2`, `hello` v1 annonçant `max_protocol_version: 2` et `capabilities`, réponse dans la version de la requête, client négociant `min(2, max daemon)` | `daemon/src/socket.rs`, `daemon/src/client.rs`, `crates/client/src/node.rs`, `crates/petals` | F1-RES-03 (fait) | Version de protocole + capacités, sans couper les clients v1 | Tests : v1 accepté et répondu en v1, v2 répondu en v2, v3 refusé sans fermer, vieux daemon négocié en v1, daemon v2 négocié en v2 | Deux versions se comportent comme spécifié | **TESTED** | une future v3 devra élargir la fenêtre, pas réinterpréter |

---

## 2. P1 — nécessaire à une fondation utilisable

| ID | Domaine | Objectif | État vérifié | Fichiers | Dépend de | Changement attendu | Test / preuve | Critère d'acceptation | Statut | Risques |
|---|---|---|---|---|---|---|---|---|---|---|
| F1-TASK-01 | Exécution | Définir un modèle de tâche commun | **Livré** : `core::task` (id, propriétaire, type, entrée, ressources, échéance, états, résultat) ; file + API gateway | `crates/core/src/task.rs`, `crates/gateway/src/tasks.rs`, `routes.rs` | F1-PROTO-01 | Type de tâche (id, demandeur, entrées, ressources, échéance, état, résultat) | 15 tests de domaine + 10 tests gateway ; `VALIDATION-RESULTS.md` | Une tâche peut être créée, validée, suivie | **PARTIAL** | l'exécuteur n'est pas branché |
| F1-TASK-02 | Exécution | Cycle de vie complet et états d'échec | Transitions, tentatives, échéance et états terminaux implémentés et testés ; pas d'exécuteur ni de reprise après crash | `core/src/task.rs`, `exec.rs` | F1-TASK-01 | États explicites + transitions + reprise après crash | 15 tests de transition et d'échec ; reprise à faire | Aucune transition implicite | **PARTIAL** | l'exécuteur reste à brancher |
| F1-TASK-03 | Résultats | Vérifier l'intégrité et l'origine d'un résultat | `TaskResult` recalcule le hash BLAKE3, exige le nœud assigné et refuse un résultat futur ; reçu signé non relié de bout en bout | `core/src/task.rs`, `core/src/receipt.rs` | F1-TASK-02 | Lier résultat, tâche, nœud, signature, doublons | Tests : résultat altéré, producteur étranger et résultat futur refusés | Résultat accepté seulement si complet et signé | **PARTIAL** | aucun producteur réel |
| F1-API-01 | API produit | Contrats stables pour `identity`, `nodes`, `resources`, `network`, `tasks`, `usage`, `settings`, `events` | API tâches livrée (créer, lire, annuler) ; socket daemon et gateway existent ; autres domaines absents | `daemon/src/socket.rs`, `crates/gateway` | F1-TASK-01 | Contrats documentés + implémentation progressive | 10 tests de contrat pour les tâches | Un client externe peut lire/agir sans types internes | **PARTIAL** | figer trop tôt |
| F1-EVT-01 | Événements | Changements observables sans polling | Aucun bus ; le client interroge | `daemon`, `gateway` | F1-API-01 | Flux d'événements borné (état, ressource, tâche, erreur) | Test de souscription + ordre + perte | Une UI reçoit les changements sans boucle de polling | NOT_STARTED | backpressure, doublons |
| F1-OBS-01 | Observabilité | Rapport de diagnostic exportable sans secrets | `doctor`/`verite` affichent, pas d'export agrégé | `crates/client`, `daemon` | F1-API-01 | Rapport structuré ( versions, états, ledger, refus ) | Test : aucun secret dans le rapport | Un développeur diagnostique sans lire le code | NOT_STARTED | fuite de chemins/clés |
| F1-MET-01 | Métriques | Métriques issues des composants réels | Aucune métrique exportée | `daemon`, `gateway`, `relay` | F1-OBS-01 | Compteurs pairs, connexions, réservations, tâches, erreurs | Test de cohérence avec l'état | Les chiffres correspondent à l'état mesuré | NOT_STARTED | métriques décoratives |
| F1-NET-02 | Réseau | File bornée / reprise pour pair hors ligne | Fragments jetés si le pair est absent | `crates/relay/src/relay.rs` | F1-PROTO-01 | File bornée avec TTL, ou refus explicite documenté | Test de perte et de reprise | Comportement explicite et testé | NOT_STARTED | mémoire, abus |
| F1-SEC-02 | Identité | Politique d'inconnu configurable (TOFU vs strict) | **Livré** : `POLYGONE_TRUST_POLICY` (`tofu` par défaut, `strict`) ; strict refuse un pair non ancré avant livraison et n'apprend rien ; valeur inconnue = erreur au démarrage | `crates/client/src/net/mod.rs` | — | Mode strict optionnel, empreinte vérifiable | 3 tests : analyse des valeurs, inconnu refusé sans apprentissage, pair ancré accepté ; codes `AUTH.UNKNOWN_PEER` | Deux politiques testées | **TESTED** | UX d'appairage : en strict, l'empreinte doit être vérifiée hors-ligne avant le premier message |
| F1-SEC-03 | Données | Décider du chiffrement au repos | Identité/reçus en clair, 0600 | `crates/client/src/identity.rs` | — | Chiffrer au repos **ou** documenter la limite | Test de permission + documentation | Limite explicite, pas implicite | NOT_STARTED | perte de clé |
| F1-SVC-01 | Produit | Câbler ou retirer `serverless` | Reconstruit localement, ne contacte aucun nœud | `crates/serverless`, `crates/client/src/serverless.rs` | F1-TASK-02 | Soit dispatch réel, soit retrait de la promesse CLI | Test avec deux nœuds réels | Aucune affirmation non prouvée | NOT_STARTED | faux sentiment de distribution |
| F1-SVC-02 | Produit | Câbler ou retirer `petals::PipelinePlan` | Plan sans consommateur de production | `crates/petals/src/pipeline.rs` | F1-NET-02 | Pipeline réel **ou** statut explicitement expérimental | Test d'inférence locale + plan | Périmètre annoncé = périmètre prouvé | NOT_STARTED | dépendances lourdes |
| F1-SVC-03 | Produit | Réputation utilisée comme sélecteur | Score affiché, aucun sélecteur | `crates/client/src/reputation.rs` | F1-TASK-01 | Sélection déterministe fondée sur la réputation | Test : nœud mal noté évité | Sélection explicable et testée | NOT_STARTED | bulles de réputation |

---

## 3. P2 — important, non bloquant

| ID | Domaine | Objectif | État vérifié | Fichiers | Dépend de | Changement attendu | Test / preuve | Critère d'acceptation | Statut | Risques |
|---|---|---|---|---|---|---|---|---|---|---|
| F2-DOC-02 | Documentation | Terminer la revue des modifications `a_examiner` | 31 classées, 4 réintégrées, décompte non recalculé | `project/DIRTY-REVIEW.md`, `docs/proofs/dirty-2026-11-09.tsv` | — | Statuer fichier par fichier, corriger le compte | Tableau à jour | Chaque entrée a un verdict | IN_PROGRESS | réintroduire du code obsolète |
| F2-CI-02 | CI | Audit de dépendances et SBOM | Non exécuté | `Cargo.lock`, `.github/workflows/ci.yml` | F1-PROOF-02 | Job `cargo audit` + SBOM | Rapport daté | 0 vulnérabilité critique non traitée | NOT_STARTED | faux positifs |
| F2-TEST-01 | Tests | Résilience systématique | Crash daemon et perte de bail déjà couverts | `scripts/crash-recovery.mjs`, `daemon` | — | Ajouter mesure invalide, réservation expirée, message incorrect | Suite dédiée | 9 scénarios couverts | PARTIAL | temps d'exécution |
| F2-TEST-02 | Tests | Charge contrôlée | Aucun test de charge | `scripts/`, `relay` | F1-PROOF-01 | Pairs/connexions/tâches sous limites surveillées | Rapport de mesure | Comportement sous saturation décrit | NOT_STARTED | épuisement de la machine |
| F2-SEC-04 | Sécurité | Limite par identité et anti-abus sur le socket daemon | Limite par identité existe côté relais (100 env/s) ; socket = 50 req/s/connexion | `daemon/src/socket.rs` | — | Vérifier qu'un client local ne peut pas affamer les autres | Test de concurrence | Un client abusif n'empêche pas les autres | PARTIAL | famine locale |
| F2-OPS-01 | Installation | Parcours install → identité → nœud → diagnostic → désinstallation | Installateur testé, désinstallation testée | `install.sh`, `scripts/test-installer.sh` | F1-PROOF-02 | Test sur VM neuve | Journal d'installation | Aucun résidu, aucun secret | PARTIAL | dépendances système |
| F2-PROD-01 | Produit | Maquette UI reliée uniquement à des API réelles | Mapping écran/API établi | `product/UI-API-MAPPING.md` | F1-API-01 | Construire les 3 écrans adossés à des données réelles | Test d'écran + données | Aucun indicateur simulé | NOT_STARTED | pression de livrer une coquille |

---

## 4. P3 — extensions ultérieures (hors Foundation 1.0)

| ID | Domaine | Objectif | Dépend de | Statut |
|---|---|---|---|---|
| F3-ECO-01 | Économie | Réciprocité non transférable, reçus de contribution | F1-TASK-03 | NOT_STARTED (décision `DECISION-POLY.md` : pas de cryptomonnaie) |
| F3-IA-01 | IA | Inférence multi-nœuds réelle | F1-SVC-02, F1-NET-02 | NOT_STARTED |
| F3-HIDE-01 | Confidentialité | Hide multi-hop | F1-PROOF-01 | NOT_STARTED |
| F3-PLAT-01 | Plateformes | Portage Windows avec isolation réelle | F1-PROOF-02 | NOT_STARTED (canari CI non bloquant) |
| F3-GOV-01 | Gouvernance | Gouvernance décentralisée | F1-API-01 | NOT_STARTED |

---

## 5. Ordre d'exécution imposé par les dépendances

```
F1-PROOF-01 (deux hôtes) ─┐
F1-PROOF-02 (CI réelle)  ─┼─> F1-PROTO-01 ─> F1-TASK-01 ─> F1-TASK-02 ─> F1-TASK-03
F1-RES-03 (politique CPU)─┘                                      │
                                                                 ├─> F1-API-01 ─> F1-EVT-01
                                                                 │        └────> F1-OBS-01 ─> F1-MET-01
                                                                 └─> F1-SVC-01 / F1-SVC-03
```

**Prochaine tâche technique précise** : `F1-TASK-02` (brancher un exécuteur
réel sur la file), parce qu'elle est réalisable sur une seule machine et que
le protocole est désormais versionné et négocié. `F1-TASK-01` est livrée pour
la file ; après l'exécuteur viendra la vérification de bout en bout
(`F1-TASK-03`). `F1-PROOF-01` et `F1-PROOF-02` restent prioritaires mais
bloquées par l'environnement (second hôte, autorisation de push).

---

## 6. Ce qui n'entre pas dans cette roadmap

Marketplace, paiement, token économique, gouvernance complexe, catalogue de
modèles, intégrations multiples, automatisation marketing. L'architecture doit
rester extensible vers eux ; ils ne doivent pas être construits maintenant.
