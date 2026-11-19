# API-CONTRACTS — contrats d'interface existants

> Date : **2026-11-13**, HEAD `f6382e6` + reçu de résultat signé (`F1-TASK-03`).
> Ce document décrit **ce qui est
> implémenté**. Une interface absente est listée comme absente, pas comme
> « prévue ».
>
> Trois niveaux coexistent, sans être physiquement séparés :
> - **local** : socket Unix du daemon (`polygoned`) ;
> - **réseau** : relais TCP et enveloppes entre pairs ;
> - **produit** : CLI/TUI `polygone` et HTTP local de la gateway.
>
> Classification de stabilité : `stable` = format figé, tout changement casse un
> client ; `beta` = utilisé par le produit, peut évoluer avec préavis ;
> `interne` = non destiné à un client externe.

---

## 1. Socket daemon — protocole v1 (`stable`)

**Transport** : socket Unix, `~/.polygone/daemon.sock` (surcharge
`POLYGONE_DAEMON_SOCKET`), permissions `0600`. Une requête = une ligne JSON
terminée par `\n` ; une réponse = une ligne JSON.

**Enveloppe obligatoire** :

```json
{"v": 2, "type": "<type>", "id": "<optionnel>"}
```

- `v` absent ou hors de la fenêtre `1..=2` → `unsupported_version`.
- Le serveur répond avec **la version de la requête** : `v: 1` reçoit du v1,
  `v: 2` reçoit du v2. Un client v1 existant continue donc de lire le flux.
- `type` hors liste blanche → `unknown_type`.
- ligne > 64 Kio → `line_too_long` (la connexion est fermée).
- JSON invalide ou champ obligatoire manquant → `malformed_json`.

### 1.1 Requêtes

| `type` | Champs | Réponse |
|---|---|---|
| `status_request` | `id?` | `status_response` |
| `ping` | `id?` | `pong` |
| `alloc_request` | `id?`, `client?`, `cpu_cores: u32`, `ram_gb: f32`, `gpu_vram_gb?: f32`, `storage_gb?: f32` | `alloc_granted` ou `alloc_denied` |
| `release` | `id?`, `allocation_id: string` | `released` |
| `renew` | `id?`, `allocation_id: string` | `renewed` ou `alloc_denied` (`unknown_allocation`) |
| `refusals_request` | `id?` | `refusals_response` |

`storage_gb` absent = **0 octet**, jamais « illimité ».

### 1.2 Réponses

| `type` | Champs notables |
|---|---|
| `hello` | `daemon_version`, `platform`, `tier`, `protocol_version` (version de base, `1`), `max_protocol_version` (`2` chez ce daemon), `min_protocol_version`, `capabilities` — envoyé à la connexion, avant toute requête. Le `hello` reste lui-même en `v: 1` pour les anciens clients. |
| `status_response` | `tier`, `seq`, `max_cpu_percent`, `max_ram_percent`, `max_bandwidth_percent`, `max_gpu_percent`, `ram_bytes`, `ram_mb`, `bandwidth_mbps`, `cpu_cores`, `gpu_free_vram_mb`, `shrinking`, `uptime_secs`, `ram_capacity`, `storage_capacity` |
| `alloc_granted` | `allocation_id`, `cpu_cores`, `ram_gb`, `gpu_vram_gb?`, `storage_gb?`, `expires_at?`, `enforced: bool` |
| `alloc_denied` | `reason` |
| `renewed` | `allocation_id`, `expires_at` |
| `released` | `allocation_id` |
| `pong` | `id?` |
| `error` | `code`, `message` |
| `refusals_response` | `counts: {raison: nombre}` |

`Capacity` = `{measured, reservable, enforced}`. `enforced = 0` signifie
« licence, pas quota appliqué ». Un client ne doit jamais lire `reservable`
comme une garantie OS.

**CPU** : `cpu_cores` est le total **physique** mesuré. Le pool réservable vaut
`floor(cpu_cores × max_cpu_percent / 100)` ; `free_cpu_cores` en soustrait les
réservations. Une demande qui dépasse ce pool est refusée avec
`insufficient_cpu`, même si la machine a des cœurs libres : le palier est une
limite de partage, pas une mesure d'inactivité. Comme pour la RAM, `enforced`
reste `0` tant qu'aucun quota OS n'est posé.

### 1.3 Motifs de refus (`alloc_denied.reason`)

| Motif | Sens |
|---|---|
| `invalid_request` | demande absurde (NaN, ≤ 0, RAM arrondie à 0, cœurs 0) |
| `exceeds_measured` | dépasse la machine elle-même — définitif |
| `insufficient_headroom` | dépasse la marge du palier — temporaire |
| `insufficient_cpu` | pool de cœurs épuisé |
| `insufficient_vram` | VRAM libre insuffisante |
| `insufficient_storage` | stockage réservable insuffisant |
| `policy_shrinking` | retrait en cours, nouvelles allocations refusées |
| `unknown_allocation` | renouvellement d'un bail inconnu ou échu |

Chaque motif a une cause et une action dans `polygone doctor` / les messages du
client (`CLI.6`). Les compteurs sont exposés par `refusals_request`.

### 1.4 Codes d'erreur de décodage

`malformed_json`, `unsupported_version`, `unknown_type`, `line_too_long`,
`rate_limited`.

### 1.5 Limites et garanties

| Contrainte | Valeur |
|---|---|
| Taille de ligne | 64 Kio |
| Connexions simultanées | 16 |
| Requêtes par seconde et par connexion | 50 |
| Inactivité maximale | 60 s |
| Durée de bail par défaut | 900 s (`POLYGONE_LEASE_SECS`) |
| Persistance | `~/.polygone/ledger.json` (`POLYGONE_LEDGER_PATH`), atomique, 0600 |

### 1.6 Compatibilité

- Ajouter un champ **optionnel** est sûr ; le retirer ou changer son type ne
  l'est pas.
- Une réponse d'un daemon antérieur sans `ram_capacity`/`storage_capacity` reste
  décodable (`Option`).
- Un daemon antérieur qui reçoit `renew`/`refusals_request` répond
  `unknown_type` ; le client doit afficher « non mesuré », pas zéro.
- **Négociation** : le client lit `max_protocol_version` (repli :
  `protocol_version`) et retient `min(2, max daemon)`. Il envoie ensuite toutes
  ses requêtes avec cette version. Si les fenêtres ne se recouvrent pas, il
  échoue avec « aucune version de protocole commune » avant toute requête.
- `capabilities` énumère les fonctions optionnelles réellement présentes
  (`refusals`, `renew`, `storage_capacity`, `cpu_tier_cap`). Une capacité
  absente se traite comme indisponible, jamais comme acquise.
- Un client v1 reçoit un `hello` v1, envoie du v1 et reçoit des réponses v1 :
  la montée en v2 ne coupe pas les binaires déjà installés.

---

## 2. Gateway HTTP locale — `/api/v1` (`beta`)

**Transport** : HTTP sur socket TCP local. Pas de TLS. CORS `*`. Corps maximal
borné (`MAX_BODY_BYTES`). Réponses JSON, erreurs en texte ou JSON.

| Méthode | Chemin | Corps | Réponse 200 | Erreurs |
|---|---|---|---|---|
| GET | `/api/v1/network/status` | — | `{nodes, active, latency_ms_median?, uptime_seconds}` | 500 |
| GET | `/api/v1/diagnostics` | — | rapport agrégé (voir §2.2) | — |
| GET | `/api/v1/services` | — | `{services: [{name, state, since}]}` | — |
| POST | `/api/v1/pair/request` | `{device_name, kem_public_key_hex, sign_public_key_hex}` | `{token, expires_in_seconds: 300}` | 400 (champ/clé invalide), 429 (trop d'appairages) |
| POST | `/api/v1/pair/accept` | `{token}` | `{ok: true}` | 400 (token malformé), 404 (inconnu/expiré/déjà utilisé) |
| GET | `/api/v1/pair/qr/{token}` | — | `{deep_link: "polygone://pair/<token>"}` | 404 |
| POST | `/api/v1/nodes/heartbeat` | `{node_id, timestamp, signature_hex}` | `{ok, node_id, last_seen}` | 400 (horodatage), 403 (signature), 404 (nœud inconnu) |
| POST | `/api/v1/nodes/ping` | `{node_id, timestamp, signature_hex}` | `{ok, challenge, expires_in_seconds: 30}` | 400, 403, 404 |
| POST | `/api/v1/nodes/pong` | `{node_id, timestamp, challenge, rtt_ms, signature_hex}` | `{ok, latency_ms}` | 400 (borne), 403, 404, 409 (sans ping / expiré / rejoué) |
| GET | `/api/v1/node/{id}` | — | `NodeInfo` | 404 |
| POST | `/api/v1/tasks` | `{owner, kind, input_hex?, cpu_cores?, ram_bytes?, storage_bytes?, gpu_vram_bytes?, deadline_in_seconds?, max_attempts?, allow_network?, allow_filesystem_write?, max_output_bytes?}` | 201 `{task, executor_attached}` | 400 (`invalid_task`), 429 (`too_many_tasks`), 500 (`task_storage_failure`) |
| GET | `/api/v1/tasks?state=&limit=&offset=` | — | `{tasks, total, limit, offset, executor_attached}` | 400 (`invalid_task`) |
| GET | `/api/v1/tasks/{id}` | — | `{task, executor_attached}` | 404 (`task_not_found`) |
| POST | `/api/v1/tasks/{id}/cancel` | `{}` (corps ignoré) | `{task, executor_attached}` | 404 (`task_not_found`), 409 (`invalid_transition`) |
| GET | `/api/*` (inconnu) | — | — | 404 JSON, jamais un fichier statique |

**Authentification** : aucune pour la lecture locale. Les mutations de nœud
exigent une signature ML-DSA-65 vérifiée contre la clé **épinglée à
l'appairage** ; le corps ne peut pas choisir sa propre clé. Un jeton
d'appairage est à usage unique, TTL 300 s, 256 bits.

**Modèle de menace de cette surface** : loopback uniquement, pas de TLS, CORS
permissif. Un processus local peut lire le trafic. Ne pas exposer cette
gateway sur une interface publique en l'état.

### 2.1 Tâches — cycle de vie (`beta`)

États : `created → validated → published → assigned → accepted → running →
succeeded`, plus les terminaux `failed`, `cancelled`, `expired`. Un état
terminal ne change plus. `expired` exige que l'échéance soit atteinte.

Ce que l'API HTTP peut faire :

1. `POST /api/v1/tasks` valide la spécification puis effectue
   `created → validated → published` dans la même requête. Le corps est refusé
   s'il contient un champ inconnu (`state`, `result`, `id`…) : un client ne
   fabrique pas un état.
2. `GET` lit l'état, `POST …/cancel` annule une tâche non terminale.
3. Une tâche publiée dont l'échéance est passée devient `expired` à la première
   lecture ; le balayage est persisté.

**Deux modes, dits par `executor_attached`.**

- `false` : magasin inerte (tests, gateway sans fichier de tâches). La tâche
  reste `published` ; l'API ne peut jamais la marquer `succeeded`.
- `true` : un exécuteur local est attaché (gateway avec fichier de tâches).
  Il assigne la tâche, exécute `echo`/`blake3`, signe le résultat en ML-DSA-65,
  vérifie le hash BLAKE3, le `NodeId` du producteur et la signature, puis
  persiste `succeeded` ou `failed`. `POST` renvoie donc l'état final, et
  `cancel` répond 409 sur une tâche déjà terminale.

Dans les deux modes, aucune route HTTP ne fabrique un résultat : elle lit celui
que l'exécuteur a produit, ou constate qu'il n'y en a pas. La reprise après
arrêt brutal est faite au démarrage de la gateway (`TaskStore::recover`).

Un résultat a cette forme :

```json
{
  "task_id": "task_…",
  "output": "68656c6c6f",
  "output_hash": "…",
  "producer": "<NodeId dérivé de la clé>",
  "producer_public_key_hex": "…",
  "produced_at": 1800000000,
  "signature_hex": "…"
}
```

La signature ML-DSA-65 couvre le payload canonique
`polygone-task-result-v1\n<task_id>\n<producer>\n<output_hash>\n<produced_at>`.
Le vérificateur exige : `task_id` égal à la tâche, hash recalculé, `producer`
égal au `NodeId` dérivé de `producer_public_key_hex`, et signature valide. Un
résultat non signé, altéré, destiné à une autre tâche ou signé par une autre
clé est refusé ; la tâche reste `running` puis passe à `failed` si l'échec
vient de l'exécuteur.

Contraintes de la première famille (`echo`, `blake3`) : déterministes, sans
réseau ni écriture disque ; une demande de permission correspondante est
refusée. Entrée ≤ 64 Kio, sortie ≤ 1 Mio, échéance ≤ 7 jours, 1 à 8 tentatives.

Persistance : `$POLYGONE_GATEWAY_TASKS` ou `~/.polygone/tasks.json` ; écriture
temporaire + `sync_all` + `rename` + fsync du répertoire, permissions `0600`.
Un fichier illisible est déplacé en `<nom>.corrupt-<horodatage>-<pid>`, jamais
supprimé. Un échec d'écriture annule la création : aucune tâche non durable
n'est reconnue.

La clé de signature de l'exécuteur vit dans `gateway-signing.json`, à côté du
fichier de tâches, avec les mêmes garanties atomiques et `0600`. Elle est
rechargée au démarrage : une tâche reprise est signée par le même `NodeId`. Si
la clé a changé, la reprise refuse explicitement de signer sous l'ancienne
identité et marque la tâche `failed` avec le motif.

### 2.2 Diagnostics — `/api/v1/diagnostics` (`beta`)

Rapport agrégé **en lecture seule**, destiné à une UI de supervision. Il ne
remplace ni `polygoned status` ni `polygone doctor`.

| Champ | Contenu | Garantie |
|---|---|---|
| `v` | `polygone-diagnostics-v1` | un client vérifie la forme, pas la version du binaire |
| `generated_at`, `gateway.uptime_seconds` | secondes Unix, uptime mesuré | — |
| `gateway.nodes_persisted`, `tasks_persisted` | vrai si un fichier est configuré | la mémoire seule ne se présente pas comme durable |
| `network.nodes`, `active` | nœuds connus, nœuds vus ≤ 300 s | `active` n'est pas `nodes` |
| `network.latency_measured`, `latency_unmeasured` | nœuds avec et sans RTT | une latence absente n'est pas un zéro |
| `network.latency_ms_median` | médiane des seules mesures, ou `null` | `null` = aucune mesure |
| `tasks.total`, `by_state` | comptage, **les 10 états présents** | un état à zéro est explicite, pas omis |
| `tasks.executor_attached` | vrai si exécution locale réelle | — |
| `daemon.available` | `false` avec motif | la gateway n'interroge pas le socket du daemon |
| `refusals.available` | `false` avec motif | aucun compteur inventé |
| `limitations` | portée explicitement locale | — |

**Aucune clé** (`signing_key_hex`) n'apparaît dans ce rapport : il agrège des
compteurs, pas des identités. Un test le vérifie explicitement.

La console `web/node.html` consomme cette route (panneau « Diagnostic
agrégé ») ; si elle répond 404 (gateway antérieure) ou pas du tout, le panneau
affiche « — » et le dit, sans zéro de remplissage.

Les compteurs de refus et les réserves restent dans le daemon
(`refusals_request` du protocole socket). Les dupliquer ici donnerait un second
chiffre à maintenir, que la gateway ne peut pas vérifier.

### 2.3 Device Reuse — proposition de rôles (`beta`, CLI seulement)

`polygone doctor` ajoute une section « Rôles proposés (Device Reuse) », calculée
par `polygone_core::reuse::suggest_roles` à partir **des mesures du daemon**
(`cpu_cores`, `ram_capacity.measured`, `storage_capacity.measured`,
`gpu_free_vram_mb`). Aucun rôle n'est activé, réservé ni annoncé au réseau.

| Rôle | Seuil mesuré | Refus nommé |
|---|---|---|
| `relais` | ≥ 1 cœur, ≥ 256 Mio de RAM | « RAM mesurée insuffisante » |
| `supervision` | ≥ 128 Mio de RAM | idem |
| `stockage` | ≥ 20 Gio libres | « stockage libre non mesuré » si absent |
| `calcul léger` | ≥ 2 cœurs, ≥ 2 Gio de RAM | batterie < 20 %, ≥ 80 °C, activité utilisateur, cœurs ou RAM insuffisants |

Une contrainte non mesurée (batterie, température, activité) est écrite
« contrainte non évaluée » : elle ne devient jamais un feu vert. Aucun champ de
ce modèle n'est exposé par HTTP, et il n'existe pas de rôle distribué :
l'appartenance au réseau n'est pas modifiée par cette proposition.

---

## 3. Relais TCP — handshake et trames (`stable` pour le handshake v1)

**Handshake** (documenté en détail dans
[`../PROTOCOL-RELAY-AUTH.md`](../PROTOCOL-RELAY-AUTH.md)) :

```
client → AUTH_INIT <node_id> <kem_pk> <nonce>…
relais → CHALLENGE <nonce>
client → AUTH_RESP <signature ML-DSA-65>
relais → AUTH_OK | AUTH_DENIED <raison>
```

`node_id` est dérivé de la clé de signature (`BLAKE3`, 128 bits). Le slot d'un
`node_id` vivant n'est pas volable. Le mode `--allow-insecure-hello` existe pour
la compatibilité et doit rester explicite.

**Trames** : NDJSON, une enveloppe par ligne, 64 Kio maximum. Le relais lit
`kind`, `from`, `to`, `session` pour router ; toute trame dont `from` diffère de
l'identité authentifiée est jetée. Limites : 200 enveloppes/s par connexion,
100/s par identité, 1 024 connexions, handshake 10 s, écriture 10 s.

**Persistance** : aucune. Un pair hors ligne perd les fragments qui lui étaient
destinés.

---

## 4. Mesh UDP (`beta`, non versionné)

Datagramme JSON sur le port 7642 : `node_id`, adresse du relais, RAM libre
optionnelle. Aucune authentification, aucune version de protocole. Écoute et
annonce uniquement sur les interfaces autorisées ; `POLYGONE_MESH_IFACE` ne peut
que restreindre. À considérer comme de la découverte best-effort, pas comme une
source de confiance.

---

## 5. CLI `polygone` (`beta`)

Surface réellement exposée (extrait ; voir `polygone --help`) :

`ecosysteme`, `doctor`, `profil`, `pause`, `reprendre`, `desinstaller`, `tui`,
`demo`, `envoyer`, `recevoir`, `ecouter`, `voisins`, `annoncer`, `compute`,
`node`, `serverless`, `clef`, `clef-tourner`, `id`, `test`, `verite`,
`premier-soir`, `carte`, `petals`, `duress`, `hide`.

Les commandes de diagnostic (`doctor`, `verite`, `ecosysteme`) affichent
« non mesuré » plutôt qu'un zéro inventé. Les commandes destructrices exigent
`--confirmer`.

---

## 6. Contrats absents (ne pas les supposer)

| Domaine attendu par une UI | État |
|---|---|
| `identity` (créer, restaurer, révoquer via API) | absent ; seulement CLI |
| `nodes` (liste enrichie, permissions par nœud) | partiel via gateway (nœuds appairés) |
| `resources` (capacités détaillées par ressource) | partiel via `status_response` |
| `network` (pairs, connexions, découverte) | partiel via gateway et CLI |
| `tasks` (créer, suivre, annuler, résultat) | **partiel** : création, lecture, annulation et résultat signé en local via gateway ; aucun placement multi-nœuds ni historique |
| `storage` (quotas, réservations, blocs) | partiel ; pas d'API |
| `usage` (historique, consommation) | **absent** |
| `settings` (lire/écrire la config via API) | absent ; fichier TOML |
| `events` (flux de changements) | **absent** ; polling uniquement |
| `diagnostics` (supervision agrégée) | **partiel** : rapport local `/api/v1/diagnostics` ; pas de flux, pas de métriques historiques |
| `device reuse` (rôles par appareil) | **partiel** : proposition CLI fondée sur les mesures du daemon ; aucun rôle activé, aucune API |

Toute UI qui afficherait ces domaines aujourd'hui devrait inventer les données.
C'est interdit par la règle du projet.

---

## 7. Règles d'évolution

1. Un champ existant ne change jamais de sens silencieusement.
2. Un champ obligatoire ne devient jamais optionnel sans version.
3. Une nouvelle capacité est annoncée par un champ de capacité, pas déduite de
   la version du binaire.
4. Une rupture de protocole exige une nouvelle version et une période de
   compatibilité.
5. Les erreurs sont nommées par un code stable ; le message est destiné à
   l'humain et peut changer.
