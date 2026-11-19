# UI-API-MAPPING — chaque écran face à ses données réelles

> Date : **2026-11-13**, HEAD `f6382e6`. Règle absolue : **aucun indicateur
> simulé**. Un écran dont la donnée n'existe pas est marqué `BLOQUÉ`, avec la
> tâche qui le débloque.
>
> **Mise à jour 2026-11-19 (prototype `prototype-complet`)** : les domaines
> `identity`, `resources`, `settings` (lecture), `usage` (ledger actif) et
> `events` sont LIVRÉS et testés — contrats `API-CONTRACTS.md` §2.4. Les
> écrans Console (étendu), Compute et Activité sont construits dessus (§9).
> Restent bloqués : historique de consommation (non enregistré), graphe de
> routage/débit (mesures inter-hôtes), écritures de config (F1-SEC-02).
>
> Sources de données disponibles : socket daemon (`status_response`,
> `refusals_response`, `ledger_response`), gateway (`/api/v1/*` dont
> `identity`, `daemon`, `resources`, `settings`, `usage`, `events`), fichiers
> locaux (identité, `peers.json`, config TOML), CLI (`doctor`, `verite`,
> `ecosysteme`). L'API de tâches existe pour créer, lire et annuler une tâche ;
> avec un fichier de tâches, elle l'exécute localement et renvoie un résultat
> signé.

---

## 1. Overview

| Élément | Donnée | Source | État | Permission |
|---|---|---|---|---|
| Nœud actif / inactif | `daemon_version`, `platform`, `uptime_secs` | gateway `/api/v1/daemon` + `/api/v1/resources` (relais du daemon) | réel | lecture locale |
| Palier et plafonds | `tier`, `max_cpu_percent`, `max_ram_percent`, `max_bandwidth_percent`, `max_gpu_percent` | gateway `/api/v1/resources` | réel | lecture |
| RAM allouée | `ram_bytes`, `ram_mb` | gateway `/api/v1/resources` | réel | lecture |
| RAM mesurée / réservable / contrainte | `ram_capacity {measured, reservable, enforced}` | gateway `/api/v1/resources` | réel | lecture |
| Stockage mesuré / réservable | `storage_capacity` | gateway `/api/v1/resources` | réel (omis si non mesuré) | lecture |
| État de retrait | `shrinking` | gateway `/api/v1/resources` | réel | lecture |
| Pairs appairés | `nodes`, `active` | gateway `/api/v1/network/status` | réel | lecture |
| Latence médiane | `latency_ms_median` | gateway | réel ou `null` | lecture |
| Alertes de refus | `refusals` | gateway `/api/v1/usage` (ledger + refus) | réel | lecture |

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
| Identité | `pseudo`, `node_id`, clés publiques KEM/DSA, rotation, révocations | gateway `GET /api/v1/identity` (lecture du fichier local, champs publics seulement) | réel — **livré 2026-11-19** |
| Rotation de clé | nouvelle signature, ancienne révoquée | CLI `clef-tourner` | réel (pas d'API d'écriture) |
| Profil de partage | `tier`, quatre seuils | gateway `GET /api/v1/settings` (lecture seule) | réel — **livré 2026-11-19** |
| Modifier le profil | `sobre|equilibre|genereux` | CLI `profil` | réel (pas d'API, F1-SEC-02) |
| Pause | intention bornée, bail rendu | CLI `pause`/`reprendre` | réel (pas d'API) |
| Port de blocs | bail, port 7643 | CLI `node servir/status` | réel |
| Isolation OS | `memory.max`, `cpu.max` du scope | CLI `doctor` | réel sous Linux avec `--isoler` |
| Stockage partagé | `storage_capacity` | gateway `/api/v1/resources` | réel ; omis si non mesuré |
| Réservations actives | `grants[]` (bail, cœurs, RAM, échéance) | gateway `GET /api/v1/usage` | réel — **livré 2026-11-19** |

**BLOQUÉ** : historique des changements de profil (non enregistré) ;
historique de consommation (le ledger évince les baux clos, F1-USG à ouvrir).

---

## 4. Compute — **PARTIEL** : file et exécution locale, placement absent

| Élément | Donnée | Source | État |
|---|---|---|---|
| Tâches publiées / annulées / expirées | `tasks[]`, `total` | `GET /api/v1/tasks` | réel : `created`…`published`, `cancelled`, `expired` |
| Soumettre une tâche | `task`, `executor_attached` | `POST /api/v1/tasks` | réel : validation + publication ; exécution signée si `executor_attached: true` |
| Suivre une tâche | `task` | `GET /api/v1/tasks/{id}` | réel |
| Annuler une tâche | `task` | `POST /api/v1/tasks/{id}/cancel` | réel, refusé si terminal |
| Tâches terminées / échouées | `state`, `result` | `GET /api/v1/tasks/{id}` | réel en local : l'exécuteur assigne, exécute et persiste l'état final |
| Résultat et vérification | `task_id`, `output`, `output_hash`, `producer`, `producer_public_key_hex`, `signature_hex` | `TaskResult` renvoyé par l'API | réel : signature ML-DSA-65 et `NodeId` dérivé de la clé |
| Nœuds disponibles pour un calcul | VRAM libre, cœurs | daemon `status` | partiel : capacité locale, pas de sélection |
| Capacités IA | backends Ollama | CLI `petals models` | local uniquement |

Un écran « file de tâches » est **construit** (`web/compute.html`, §9) sur ces
quatre routes. Il affiche `executor_attached` : `false` signifie qu'une tâche
reste `published`, `true` qu'elle est exécutée et signée localement. L'écran
« résultat » affiche le reçu signé (producteur, empreinte BLAKE3, signature) ;
la vérification cryptographique complète reste à la CLI. La progression est
couverte par les événements de cycle de vie (`task_created`, `task_resumed`,
`task_expired`, `task_cancelled` via `/api/v1/events`, F1-EVT-01 **livré**) :
l'exécution locale étant synchrone dans la requête, l'événement porte l'état
final réel — pas de fausse barre de progression.

---

## 5. Activity — **LIVRÉ 2026-11-19** (hors historique de consommation)

| Élément | Donnée | Source | État |
|---|---|---|---|
| Événements (connexion, tâche, ressources, daemon) | `events[] {id, at, kind, data}`, curseur, gap | gateway `GET /api/v1/events?since=&limit=&kind=` | réel — **livré** (F1-EVT-01), anneau 512, perte signalée |
| Historique de consommation | — | — | **BLOQUÉ** : non enregistré (F1-USG à ouvrir) |
| Journal de diagnostic | sortie texte de `doctor`/`verite` | CLI | partiel, non structuré |
| Compteurs de refus | `refusals` | gateway `GET /api/v1/usage` | réel — **livré** |
| Réservations actives | `grants[]`, agrégats réservés | gateway `GET /api/v1/usage` | réel — **livré** |
| Redémarrages | `uptime_secs` + événements `daemon_up`/`daemon_absent`/`gateway_up` | gateway | réel |
| Erreurs | journaux locaux | fichiers | non exposé |

L'écran `web/activity.html` (§9) affiche le flux à curseur (polling 3 s,
bandeau de gap explicite) et le ledger actif. Le journal de diagnostic
structuré et les erreurs de services restent à exposer (F1-OBS-01, F1-MET-01).

---

## 6. Settings

| Élément | Donnée | Source | État |
|---|---|---|---|
| Identité | clés publiques, `node_id`, pseudo, rotation | gateway `GET /api/v1/identity` | réel — **livré 2026-11-19** |
| Révocation / rotation | ancienne clé marquée révoquée | CLI `clef-tourner` | réel (pas d'API d'écriture) |
| Profil et seuils | config TOML | gateway `GET /api/v1/settings` (lecture seule, `source: file\|defaults`) | réel — **livré 2026-11-19** |
| Limites réseau | `max_bandwidth_percent` | gateway `/api/v1/resources` | réel (non appliqué par l'OS) |
| Confidentialité / menace | documentation | `docs/security/THREAT-MODEL.md` | statique |
| Destructions | `duress --confirmer` | CLI | réel, irréversible |
| Désinstallation | `desinstaller --confirmer` | CLI | réel |
| Préférences UI | — | — | **BLOQUÉ** : aucune API de préférences |

La lecture de la configuration est livrée ; **aucune écriture HTTP** n'existe
pour la configuration (F1-SEC-02). La console affiche les réglages en lecture
seule et renvoie vers la CLI pour modifier — c'est explicite à l'écran, pas
un bouton mort.

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

---

## 9. Écrans livrés (prototype 2026-11-19)

| Écran | Fichier | Routes consommées | Règles respectées |
|---|---|---|---|
| Console (Overview + Mon nœud) | `web/node.html` + `dashboard.js` + `node-local.js` | `/network/status`, `/services`, `/diagnostics`, `/identity`, `/daemon`, `/resources`, `/settings` | null ≠ 0 ; `mesurée/réservable/appliquée` distinguées ; erreur = code + hint ; identité publique seulement ; réglages lecture seule |
| Compute | `web/compute.html` + `compute.js` | `POST/GET /tasks`, `GET /tasks/{id}`, `POST /tasks/{id}/cancel` | `executor_attached` affiché ; résultat signé montré (producteur, blake3, signature) sans prétendre vérifier la signature dans le navigateur ; annulation seulement sur état non terminal |
| Activité | `web/activity.html` + `activity.js` | `/events?since=&limit=`, `/usage` | curseur monotone ; bandeau de gap explicite ; ledger actif seulement (pas d'historique inventé) ; daemon absent = message, pas zéro |
| Drive / Mesh | `web/drive.html`, `web/mesh.html` | existants | inchangés (nav étendue) |

Coque PWA : `sw.js` version `polygone-v3`, nouveaux fichiers précachés ; les
réponses API ne sont jamais mises en cache. Démarrage :
`polygone-gateway --site-dir ./web` (loopback uniquement).
