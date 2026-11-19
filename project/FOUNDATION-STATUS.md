# FOUNDATION-STATUS — Polygone Foundation 1.0

> Date : **2026-11-13**. Branche `reconcile/2026-11`, HEAD `f6382e6` plus le
> correctif de persistance du ledger, la file de tâches, le plafond CPU, la
> politique de confiance, la négociation de protocole et le reçu de résultat
> signé décrits ci-dessous.
>
> Niveaux utilisés, sans exception :
> **PASS** = testé et vérifié ; **PARTIAL** = fonctionne partiellement ou
> couverture incomplète ; **FAIL** = échec confirmé ; **NOT VERIFIED** = aucune
> preuve suffisante. Un test local n'est jamais présenté comme multi-machine ;
> un mock n'est jamais présenté comme réel.

---

## 1. Tableau de validation (critères du §16 de la mission)

| Domaine | Critère obligatoire | État | Confiance | Preuve / limite |
|---|---|---|---|---|
| Architecture | Architecture canonique documentée et cohérente | **PARTIAL** | haute | `docs/architecture/` créé et adossé au code ; les documents historiques contradictoires sont signalés, pas réécrits |
| Installation | Installation reproductible | **PARTIAL** | moyenne | `install.sh` + `scripts/test-installer.sh` (fixture) ; pas de test sur VM neuve dans ce lot |
| Identité | Identité persistante et vérifiable | **PASS** | haute | `identity.rs` 0600, rotation/révocation, tests ; vérification cryptographique par signature |
| Réseau | Communication P2P authentifiée | **PARTIAL** | haute | handshake signé + messages signés + anti-rejeu sur relais réel, **loopback uniquement** |
| Multi-machine | Validation entre machines physiques distinctes | **NOT VERIFIED** | — | aucune exécution ; procédure prête (`docs/P0-DEUX-MACHINES.md`) |
| Résilience | Gestion des déconnexions et redémarrages | **PARTIAL** | moyenne | `crash-recovery.mjs` 9/9 (bail durable, SIGKILL), ledger atomique ; reconnexion inter-hôtes non testée |
| Ressources | Mesures réelles et actualisées | **PARTIAL** | haute | CPU/RAM/stockage/GPU mesurés au tick ; pool CPU réservable plafonné par `max_cpu_percent` (`F1-RES-03`) ; `user_active` = `Unknown` sur Linux/macOS ; pas d'application OS sans `--isoler` |
| Partage | Limites contrôlées par le propriétaire | **PARTIAL** | moyenne | paliers, marges, pause, retrait, `shrinking` ; pool CPU plafonné par `max_cpu_percent` (`F1-RES-03`, FND-04 corrigé) ; aucun quota CPU OS posé (`enforced = 0`) |
| Réservations | Allocation cohérente et persistante | **PASS** | haute | ledger : reserve/release/renew/expire, concurrence sous un mutex, redémarrage, **écriture atomique** (nouveau) |
| Exécution | Tâche distribuée réellement exécutée | **PARTIAL** | moyenne | exécuteur local attaché : `echo`/`blake3`, transitions complètes, résultat BLAKE3 vérifié, reprise après arrêt ; pas encore de placement multi-nœuds |
| Résultats | Résultats récupérables et vérifiables | **PASS** (local) | haute | résultat signé ML-DSA-65, hash BLAKE3 recalculé, `NodeId` dérivé de la clé et `task_id` liés ; reçu vérifié de bout en bout en local (`F1-TASK-03`) ; pas de placement multi-nœuds |
| Sécurité | Modèle de menace et protections documentés | **PARTIAL** | moyenne | `docs/security/THREAT-MODEL.md` + protections testées ; TOFU par défaut, mode strict disponible (`POLYGONE_TRUST_POLICY`, `F1-SEC-02`) ; aucun audit externe |
| API | Contrats stables et utilisables par un client | **PARTIAL** | haute | socket daemon v1/v2 négocié (réponse dans la version du pair), HTTP gateway v1, API tâches (créer, lire, annuler) et diagnostics agrégés ; usage/événements/réglages absents |
| Événements | États et changements observables | **FAIL** | haute | aucun bus ; seulement du polling (`status`, `refusals`, gateway) |
| Diagnostic | Journaux et métriques exploitables | **PARTIAL** | haute | `doctor`, `verite`, `ecosysteme`, compteurs de refus, et depuis le 2026-11-15 un rapport agrégé `GET /api/v1/diagnostics` (réseau, tâches, sections indisponibles motivées) ; pas d'historique ni de métriques exportées |
| Tests | Suites unitaires et intégration validées | **PASS** (local) | haute | 512 tests workspace, exit 0 ; portée locale explicitement limitée |
| CI | Pipeline reproductible | **NOT VERIFIED** | — | structure vérifiée par `check-ci-honesty.mjs` ; runner jamais exécuté |
| Dépendances | Audit effectué | **NOT VERIFIED** | — | aucun `cargo audit`/SBOM dans ce lot |
| Secrets | Vérification effectuée | **PASS** (local) | moyenne | Gitleaks 2026-11-13 : historique (281 commits) et copie de l'arbre hors `target/`/`.git/`, aucune fuite |
| Documentation | Documentation développeur et produit | **PASS** | haute | 13 livrables de fondation créés/actualisés, indexés quand ils vivent sous `docs/` |
| Préparation UI | Correspondance écran, API et données définie | **PARTIAL** | haute | `product/UI-API-MAPPING.md` ; les écrans sans API sont marqués bloqués |

---

## 2. Ce qui a été corrigé dans ce lot

| Correctif | Fichier | Preuve |
|---|---|---|
| Écriture atomique du ledger, permissions 0600, temporaire nettoyé | `daemon/src/socket.rs` | `test_ledger_persist_is_atomic_and_private` |
| Quarantaine d'un ledger illisible, aucune perte silencieuse | `daemon/src/socket.rs` | `test_corrupt_ledger_is_quarantined_not_silently_erased` |
| Commentaire `alloc_granted.expires_at` aligné sur le bail réel | `daemon/src/socket.rs` | lecture + tests `renew` |
| Chiffres d'architecture recalculés (35 577 lignes, 465 tests) | `ARCHITECTURE.md` | `scripts/check-docs.mjs` |

## 2 bis. Incrément F1-TASK-01 — file de tâches (2026-11-09)

| Élément | Fichier | Preuve |
|---|---|---|
| Modèle de tâche : spec validée, états explicites, transitions, résultat BLAKE3 | `crates/core/src/task.rs` | 15 tests : cycle complet, transition illégale, état terminal, résultat altéré/producteur étranger/futur, bornes, sérialisation hex |
| Magasin : publication, annulation, expiration, pagination, persistance atomique `0600`, quarantaine | `crates/gateway/src/tasks.rs` | 8 tests, dont redémarrage, expiration balayée et échec de persistance sans accusé |
| API HTTP : `POST/GET /api/v1/tasks`, `GET /{id}`, `POST /{id}/cancel` | `crates/gateway/src/routes.rs` | 2 tests : création/liste/lecture/annulation et refus d'un `state` glissé dans le corps |
| Test latent corrigé | `crates/gateway/src/routes.rs` | `latency_populated_after_pong` comparait `last_seen` au timestamp client ; borne `>= stamp` |

## 3. Ce qui a été ajouté (documentation)

`project/FOUNDATION-AUDIT.md`, `project/FOUNDATION-ROADMAP.md`,
`project/FOUNDATION-STATUS.md`, `docs/architecture/SYSTEM-ARCHITECTURE.md`,
`docs/architecture/COMPONENT-MAP.md`, `docs/architecture/DATA-FLOWS.md`,
`docs/architecture/API-CONTRACTS.md`, `docs/security/THREAT-MODEL.md`,
`docs/testing/TEST-STRATEGY.md`, `docs/testing/VALIDATION-RESULTS.md`,
`product/PRODUCT-FOUNDATION.md`, `product/UI-API-MAPPING.md`,
`product/PRODUCT-READINESS.md`.

## 4. Ce qui est réellement testé

- 512 tests Rust workspace sur Linux, 0 échec (dont 20 pour le modèle et l'exécution des tâches, 13 pour le magasin de tâches, 2 pour le plafond CPU, 3 pour la politique de confiance et 4 pour la négociation de protocole).
- Formatage et Clippy en erreur sur tout le workspace (voir
  `docs/testing/VALIDATION-RESULTS.md` pour les codes de sortie exacts).
- Parcours réseau local : relais + deux pairs, handshake signé, refus d'identité
  non prouvée (`scripts/p0-two-process.sh`).
- Migration `peers.json` v1 → v2 et compatibilité inter-versions (preuves datées
  existantes, rejouables).
- Ledger : réservation/libération/renouvellement/expiration, redémarrage,
  atomicité et quarantaine.
- Gateway : appairage, heartbeat signé, ping/pong, socket réel.

## 5. Ce qui n'est pas terminé

- Aucun test entre deux machines physiques.
- Aucune exécution de la CI GitHub Actions sur cet arbre.
- Aucun audit de dépendances ni SBOM.
- API de tâches : l'exécuteur local produit un résultat signé pour `echo`/`blake3` et le vérifie de bout en bout ; il n'existe toujours ni placement multi-nœuds ni historique de tâches.
- Pas d'historique / consommation / réglages / événements.
- `serverless`, `petals::PipelinePlan` et la réputation ne sont pas câblés au produit.
- Politique CPU appliquée au pool réservable, mais aucun quota CPU OS n'est posé (`enforced = 0`) : c'est une limite de partage, pas une contrainte noyau.
- Le mode strict existe mais n'est pas le défaut ; l'appairage hors-ligne reste à outiller.
- Aucun métrique exportée ni rapport de diagnostic agrégé.
- Revue des modifications historiques `a_examiner` incomplète.

## 6. Ce qui bloque

1. **Second hôte physique** — bloque `F1-PROOF-01` et toute revendication multi-machine.
2. **Autorisation de push** — bloque l'exécution de la CI réelle.
3. **Placement multi-nœuds** — l'exécuteur local signe et vérifie ses résultats, mais aucun ordonnanceur distribué ne choisit un producteur distant.

## 7. Risques restants

| Risque | Niveau | Réduction |
|---|---|---|
| Métadonnées visibles par le relais | élevé (assumé) | documenté et mesuré ; pas de promesse d'anonymat |
| Exécution shell même UID | élevé | `systemd-run` + protections ; ce n'est pas une frontière cryptographique |
| TOFU au premier contact | moyen | empreinte affichée ; mode strict disponible mais non défaut |
| Ledger local saturable en écritures | moyen | limite de débit par connexion ; pas de quota global d'écritures |
| Dépendances non auditées | moyen | `Cargo.lock` ; audit à exécuter |
| Horloges dérivées | moyen | fenêtre ±300 s ; à surveiller en multi-hôte |

## 8. Prochain travail prioritaire

1. **`F1-API-01`** — stabiliser les contrats des domaines encore absents (identity, resources, settings, events).
2. **`F1-OBS-01`** — exporter un rapport de diagnostic agrégé, sans secret.
3. **`F1-PROOF-01`** — dès qu'un second hôte est disponible, exécuter la procédure
   et remplir ses preuves.
4. **`F1-PROOF-02`** — exécuter la CI après autorisation de push.

---

## 8 bis. Incrément 2026-11-15 — supervision agrégée et Device Reuse

| Élément | Fichier | Preuve |
|---|---|---|
| `GET /api/v1/diagnostics` : réseau mesuré, 10 états de tâche, sections indisponibles motivées, aucune clé | `crates/gateway/src/diagnostics.rs`, `routes.rs` | 2 tests : agrégation mesurée sans clé, et absence de latence/refus inventés |
| Comptage par état, tous états présents | `crates/gateway/src/tasks.rs` | `counts_by_state` ; test `diagnostics_never_invents_a_latency_or_a_refusal_count` (10 entrées) |
| Proposition de rôles fondée sur des mesures | `crates/core/src/reuse.rs` | 8 tests (petit appareil, stockage non mesuré, batterie, température, activité, ordre stable) |
| Rôles affichés par `polygone doctor` | `crates/client/src/main.rs` | `doctor_proposes_device_reuse_roles_from_measured_capacity` |
| Suite workspace | — | **565 réussis, 0 échec** |

Aucun rôle n'est activé : ce lot ne change ni l'appartenance au réseau ni les
réservations. Le rapport de diagnostic ne remplace pas `polygoned status`.

## 9. Verdict global

**Polygone Foundation 1.0 n'est pas atteinte.** Le socle est cohérent, testé
localement et documenté : identité, messages, relais authentifié, ledger de
réservations, mesure des ressources, gateway locale. Mais quatre critères
obligatoires restent `NOT VERIFIED` (multi-machine, CI, dépendances, secrets
finaux) et trois capacités structurantes sont absentes ou partielles
(événements, API produit de tâches, exécution distribuée complète).

La fondation actuelle permet de construire une application **de lecture et de
contrôle local** (état du nœud, ressources, pairs connus, identité, pause,
diagnostic) et une **file de tâches locale** (créer, lire, annuler, exécuter
`echo`/`blake3`, vérifier un résultat signé). Elle ne permet pas encore une
application de **calcul distribué** : le placement multi-nœuds et les
événements manquent, même si le résultat local est désormais signé et vérifié.
