# UI-API-MAPPING — chaque écran face à ses données réelles

> Date : **2026-11-09**, HEAD `e2936a3`. Règle absolue : **aucun indicateur
> simulé**. Un écran dont la donnée n'existe pas est marqué `BLOQUÉ`, avec la
> tâche qui le débloque.
>
> Sources de données disponibles : socket daemon (`status_response`,
> `refusals_response`), gateway (`/api/v1/*`), fichiers locaux (identité,
> `peers.json`, config TOML), CLI (`doctor`, `verite`, `ecosysteme`).
> L'API de tâches existe pour créer, lire et annuler une tâche ; elle ne
> l'exécute pas. Il n'existe **aucune** API d'historique, de consommation, de
> réglages ni d'événements.

---

## 1. Overview

| Élément | Donnée | Source | État | Permission |
|---|---|---|---|---|
| Nœud actif / inactif | `daemon_version`, `platform`, `uptime_secs` | daemon `status_response` | réel | lecture locale |
| Palier et plafonds | `tier`, `max_cpu_percent`, `max_ram_percent`, `max_bandwidth_percent`, `max_gpu_percent` | daemon | réel | lecture |
| RAM allouée | `ram_bytes`, `ram_mb` | daemon | réel | lecture |
| RAM mesurée / réservable / contrainte | `ram_capacity {measured, reservable, enforced}` | daemon | réel | lecture |
| Stockage mesuré / réservable | `storage_capacity` | daemon | réel (0 si non mesuré) | lecture |
| État de retrait | `shrinking` | daemon | réel | lecture |
| Pairs appairés | `nodes`, `active` | gateway `/api/v1/network/status` | réel | lecture |
| Latence médiane | `latency_ms_median` | gateway | réel ou `null` | lecture |
| Alertes de refus | `counts` | daemon `refusals_request` | réel | lecture |

**États d'écran** : chargement = aucune valeur affichée ; vide = « daemon
absent » / `null`, jamais `0` ; dégradé = daemon absent ou gateway arrêtée.
**Aucune** permission d'écriture sur cet écran.

---

## 2. Network

| Élément | Donnée | Source | État |
|---|---|---|---|
| Liste des nœuds appairés | `NodeInfo {id, services, device_name?, last_seen, latency_ms?}` | gateway `/api/v1/node/{id}` et `/network/status` | réel |
| Nœud actif | `is_active(last_seen, now)` fenêtre 300 s | gateway `status.rs` | réel |
| Latence par nœud | `latency_ms` | gateway, après pong | réel ou `null` |
| Pairs découverts sur le LAN | `node_id`, relais, RAM libre | CLI `voisins` (UDP) | réel mais non exposé par API |
| Graphe des connexions live | — | — | **BLOQUÉ** : le relais ne publie pas son routage |
| Débit par pair | — | — | **BLOQUÉ** : aucune mesure exposée |

Opérations disponibles : appairer un appareil (`POST /pair/request` puis
`/pair/accept`), afficher le QR (`/pair/qr/{token}`). Aucune révocation par API.

---

## 3. My Node

| Élément | Donnée | Source | État |
|---|---|---|---|
| Identité | `id` (node_id), clé publique KEM | CLI `id`, `clef` ; `identity.json` | réel |
| Rotation de clé | nouvelle signature, ancienne révoquée | CLI `clef-tourner` | réel |
| Profil de partage | `tier`, quatre seuils | config TOML + daemon | réel |
| Modifier le profil | `sobre|equilibre|genereux` | CLI `profil` | réel (pas d'API) |
| Pause | intention bornée, bail rendu | CLI `pause`/`reprendre` | réel (pas d'API) |
| Port de blocs | bail, port 7643 | CLI `node servir/status` | réel |
| Isolation OS | `memory.max`, `cpu.max` du scope | CLI `doctor` | réel sous Linux avec `--isoler` |
| Stockage partagé | `storage_capacity` | daemon | réel ; réservations non listées individuellement |

**BLOQUÉ** : liste détaillée des réservations actives (le ledger n'expose que
des agrégats) ; historique des changements de profil.

---

## 4. Compute — **PARTIEL** : file de tâches réelle, exécution absente

| Élément | Donnée | Source | État |
|---|---|---|---|
| Tâches publiées / annulées / expirées | `tasks[]`, `total` | `GET /api/v1/tasks` | réel : `created`…`published`, `cancelled`, `expired` |
| Soumettre une tâche | `task`, `executor_attached: false` | `POST /api/v1/tasks` | réel : validation + publication, **pas** d'exécution |
| Suivre une tâche | `task` | `GET /api/v1/tasks/{id}` | réel |
| Annuler une tâche | `task` | `POST /api/v1/tasks/{id}/cancel` | réel, refusé si terminal |
| Tâches assignées / en cours / terminées | — | — | **BLOQUÉ** : aucun exécuteur (`F1-TASK-02`) |
| Résultat et vérification | modèle `TaskResult` + hash BLAKE3 | — | **BLOQUÉ** : aucun producteur réel (`F1-TASK-03`) |
| Nœuds disponibles pour un calcul | VRAM libre, cœurs | daemon `status` | partiel : capacité locale, pas de sélection |
| Capacités IA | backends Ollama | CLI `petals models` | local uniquement |

Un écran « file de tâches » peut être construit sur ces quatre routes, à
condition d'afficher `executor_attached: false` et de ne jamais présenter une
tâche `published` comme en cours. Les écrans « progression » et « résultat »
restent bloqués par `F1-TASK-02` et `F1-TASK-03`.

---

## 5. Activity — **BLOQUÉ dans son ensemble**

| Élément | Donnée | Source | État |
|---|---|---|---|
| Événements (connexion, tâche, erreur) | — | — | **BLOQUÉ** (`F1-EVT-01`) |
| Historique de consommation | — | — | **BLOQUÉ** |
| Journal de diagnostic | sortie texte de `doctor`/`verite` | CLI | partiel, non structuré |
| Compteurs de refus | `counts` | daemon `refusals_request` | réel |
| Redémarrages | `uptime_secs` | daemon | indirect seulement |
| Erreurs | journaux locaux | fichiers | non exposé |

Seul le bloc « refus par motif » est affichable aujourd'hui. Le reste exige
`F1-OBS-01`, `F1-MET-01` et `F1-EVT-01`.

---

## 6. Settings

| Élément | Donnée | Source | État |
|---|---|---|---|
| Identité | clés publiques, `node_id` | `identity.json` | réel |
| Révocation / rotation | ancienne clé marquée révoquée | CLI `clef-tourner` | réel (pas d'API) |
| Profil et seuils | config TOML | fichier + CLI | réel |
| Limites réseau | `max_bandwidth_percent` | daemon | réel (non appliqué par l'OS) |
| Confidentialité / menace | documentation | `docs/security/THREAT-MODEL.md` | statique |
| Destructions | `duress --confirmer` | CLI | réel, irréversible |
| Désinstallation | `desinstaller --confirmer` | CLI | réel |
| Préférences UI | — | — | **BLOQUÉ** : aucune API de settings |

Aucune opération d'écriture via HTTP n'existe pour la configuration. Une UI
devrait passer par le fichier ou la CLI, ce qui n'est pas acceptable pour un
utilisateur non technique : c'est la tâche `F1-API-01` (domaine `settings`).

---

## 7. États transverses

| État | Comportement exigé |
|---|---|
| Chargement | squelette sans chiffre ; jamais `0` par défaut |
| Vide | « aucune mesure », « daemon absent », « aucun pair » |
| Dégradé | bandeau explicite + dernière valeur datée si disponible |
| Erreur | code + cause + action, jamais un écran blanc |
| Non autorisé | explication de la permission manquante |
| Déconnecté | les valeurs cessent d'être présentées comme courantes |

---

## 8. Règles pour toute future UI

1. Ne jamais appeler une structure interne du daemon : passer par un contrat
   documenté dans `docs/architecture/API-CONTRACTS.md`.
2. Ne jamais transformer `null` en `0`.
3. Ne jamais afficher « en ligne » sans `last_seen` dans la fenêtre.
4. Ne jamais afficher une tâche ou une consommation qui n'a pas de source.
5. Toute nouvelle donnée exige d'abord son contrat et son test côté moteur.
6. Une permission refusée doit être distinguée d'une erreur technique.
