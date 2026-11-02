# PLAN_SOCKET — tâche 1 : le daemon devient serveur du socket de décision

> Plan seul. Aucun code écrit. Décision actée : **Option B** — `polygoned` écoute sur
> `~/.polygone/daemon.sock` ; `Petals` s'y adaptera dans une tâche ultérieure.
> Contexte : `docs/DAEMON_STATE.md` §3.1 (le socket n'a aucun consommateur, et le client
> Petals vise un autre chemin avec un autre protocole).

---

## 1. Protocole

**Transport** : socket Unix, flux (`SOCK_STREAM`), `~/.polygone/daemon.sock`. **Format** : JSON, **une requête par ligne**, UTF-8, terminateur `\n`, 64 Kio maximum par ligne.
**Bornes** : `PROTOCOL_VERSION = 1` ; champ `v` **obligatoire** dans tout message ; `id` optionnel, recopié tel quel dans la réponse.
**Enveloppe** : `{"v":1,"id":"a1b2","type":"…", …champs…}` — champs à plat, pas d'imbrication.

### Messages client → daemon

| `type` | Champs | Effet attendu |
|---|---|---|
| `status_request` | `id` | Réponse `status_response` |
| `alloc_request` | `id`, `client`, `cpu_cores`, `ram_gb`, `gpu_vram_gb` (null si aucun) | `alloc_granted` ou `alloc_denied` |
| `release` | `id`, `allocation_id` | `released` |
| `subscribe` | `id` | Reçoit ensuite les `tick_update` ; réponse `subscribed` |
| `unsubscribe` | `id` | Réponse `unsubscribed` |
| `ping` | `id` | `pong` |

### Messages daemon → client

| `type` | Contenu |
|---|---|
| `hello` | Envoyé à la connexion : `v`, `daemon_version`, `platform`, `tier`, `protocol_version` |
| `status_response` | `tier`, limites effectives, `ram_bytes`/`ram_mb`, `bandwidth_mbps`, `shrinking`, `cpu_cores`, `gpu` (résumé), `uptime_secs` |
| `alloc_granted` | `allocation_id`, `cpu_cores`, `ram_gb`, `gpu_vram_gb`, `expires_at` |
| `alloc_denied` | `reason` (ex. `insufficient_headroom`, `unsupported`) |
| `released` | `allocation_id` |
| `subscribed` / `unsubscribed` | Accusés |
| `tick_update` | Diffusé à chaque tick aux abonnés : `ram_mb`, `bandwidth_mbps`, `tier`, `shrinking`, `ts` |
| `pong` | Accusé de `ping` |
| `error` | `code` + `message` : `malformed_json`, `unknown_type`, `unsupported_version`, `line_too_long`, `rate_limited`, `internal` |

**Compatibilité.** `docs/SPEC.md` décrit un flux **unidirectionnel** (`cmd` : `set_alloc`,
`shrink`, `grow`) que personne ne lit. Ce plan le remplace par un flux **bidirectionnel** ;
pour ne pas casser un lecteur conforme à `SPEC.md`, `tick_update` **réutilise les noms de
champs historiques** (`ram_mb`, `bandwidth_mbps`, `ts`) et n'ajoute que l'en-tête `v`/`type`.

**Erreurs** : une erreur **ne ferme pas** la connexion (sauf `line_too_long` et
`rate_limited`). Le client peut se corriger et continuer.

---

## 2. Changements côté daemon

### Fichiers

| Fichier | Nature |
|---|---|
| `daemon/src/socket.rs` | **Principal.** Devient le module serveur (bind, accept, dispatch, diffusion) ; garde `socket_path()` et active enfin `ensure_dir()` |
| `daemon/src/main.rs` | Démarre le serveur, publie l'allocation à chaque tick, arrête proprement |
| `daemon/src/policy/glow_up.rs` | **Inchangé** (exposer les limites pour `status` si besoin) |
| `daemon/Cargo.toml` | **Inchangé** — `std` suffit, aucune dépendance nouvelle |
| `daemon/SPEC.md` | Mettre à jour « Interface to Polygone Node » (protocole v1) |
| `ARCHITECTURE.md` | §7 et §11 : retirer « no consumer » (et corriger le renvoi erroné à « D5 », qui traite de la topologie du relais) |
| `DECISIONS.md` | Nouvelle entrée (prochain numéro libre : **D10**) : « le daemon devient serveur du socket de décision » |
| `docs/DAEMON_STATE.md` | Corriger §3.1 après implémentation |

### Nouvelles fonctions (dans `socket.rs`)

- `cleanup_stale_socket(path)` — supprime un fichier de socket orphelin (cf. §4).
- `serve(path, state)` — bind + boucle `accept`, un thread par connexion.
- `handle_client(stream, state)` — boucle lecture-ligne → `dispatch` → réponse.
- `dispatch(request, state) -> Reply` — **cœur pur**, sans entrée/sortie : c'est lui qui est
  testé unitairement.
- `encode(reply)` / `decode(line)` — sérialisation et validation (version, taille, type).
- `broadcast(tick)` — envoie `tick_update` aux clients abonnés, sans jamais bloquer.
- `shutdown()` — ferme le listener et supprime le fichier socket.

### Placement dans `main.rs`

1. Après `platform.init()` et le chargement de la config : appeler `ensure_dir()`
   (**aujourd'hui jamais appelé** — sans lui, `~/.polygone` peut ne pas exister et le bind
   échoue pour une raison trompeuse).
2. `cleanup_stale_socket()`, puis lancement de `serve()` dans un **thread dédié**, sur le
   modèle exact de `run_status_http` (déjà lancé ainsi, ligne ~119) : la boucle du daemon
   est synchrone, on n'introduit donc **pas** de runtime asynchrone.
3. État partagé : réutiliser le motif `Arc<Mutex<DaemonStatus>>` déjà construit pour
   `--expose`, en y ajoutant l'allocation courante.
4. Dans la boucle, **après** `engine.tick()` et `engine.apply()` : remplacer
   `notify_allocation` / `notify_shrink` par `broadcast(tick_update)`.
5. À la sortie : dernier `broadcast` de décroissance, `shutdown()`, puis sortie normale.

---

## 3. Tests

`tempfile` est déjà en `dev-dependencies` : chaque test utilise un socket temporaire, jamais
`~/.polygone`.

1. **Connexion + `status_response`** — démarrer `serve()` sur un chemin temporaire, connecter
   un client, envoyer `{"v":1,"id":"t1","type":"status_request"}`, lire une ligne, vérifier
   qu'elle est un JSON valide avec `v == 1`, `type == "status_response"`, `id == "t1"` et des
   champs `tier` / `ram_mb` / `bandwidth_mbps` présents. Vérifier aussi la réception du
   `hello` initial.
2. **Message mal formé rejeté proprement** — quatre cas couverts par un test paramétré :
   - JSON invalide (`{"v":1,"type":`) → `error:malformed_json` ;
   - type inconnu → `error:unknown_type` ;
   - version inconnue (`v:2`) → `error:unsupported_version` ;
   - ligne > 64 Kio → `error:line_too_long` **puis fermeture**.
   Dans les trois premiers cas, la connexion doit **rester ouverte** (envoyer un
   `status_request` ensuite et recevoir une réponse valide).
3. *(recommandé)* **Diffusion** — deux clients abonnés reçoivent le même `tick_update`.
4. *(recommandé)* **`dispatch()` en pur** — logique métier testée sans socket : plus rapide,
   plus stable.
5. *(recommandé)* **Nettoyage** — un socket orphelin est supprimé ; un socket vivant fait
   échouer le démarrage avec un message explicite.

---

## 4. Risques

**Deux clients (ou plus).** Le serveur est multi-clients par construction : un thread par
connexion, et les réponses sont corrélées par `id`, donc pas d'ambiguïté. Le vrai danger est
la **diffusion** : la boucle principale ne doit jamais bloquer sur un client qui ne lit pas.
Contre-mesure : écriture avec délai d'expiration et mise en quarantaine (client illisible =
déconnecté), plus un plafond de connexions simultanées (proposé : 16).

**Socket déjà présent (crash précédent).** `bind` échouerait avec « address already in
use ». Politique au démarrage : si un fichier existe, tenter une connexion de test —
si personne ne répond, le fichier est orphelin → **le supprimer** et continuer ; si un pair
répond, une autre instance est vivante → **refuser de démarrer** avec un message clair,
sans tuer l'instance en place. Le fichier est aussi supprimé à l'arrêt normal.
Deux limites assumées et à vérifier à l'implémentation : (a) aujourd'hui `ensure_dir()`
n'est jamais appelé, donc un échec de bind peut avoir cette cause et non un socket orphelin ;
(b) sur certains systèmes, un socket sans lecteur ne se laisse pas sonder immédiatement —
la connexion de test devra avoir un délai court et un échec par défaut prudent.

**1000 requêtes par seconde.** Risque de saturation CPU et de famine de la boucle
d'allocation. Contre-mesures : plafond de requêtes par seconde et par connexion (proposé :
50), plafond de 64 Kio par ligne, plafond de connexions, et réponse `error:rate_limited`
suivie d'une fermeture. Règle de conception décisive : **`status_request` ne doit jamais
déclencher une capture système** — il lit un état déjà en mémoire. Sans cette règle,
1000 requêtes/s deviendraient 1000 lectures de `/proc` par seconde. Enfin, `tick_update` est
**diffusé**, jamais demandé : un client curieux ne peut pas accélérer le rythme du daemon.

---

## Hors périmètre (tâches suivantes)

- Adapter le client `Petals` (`crates/petals/src/daemon.rs`) au chemin et au protocole v1.
- Authentification : non prévue ici — le socket est protégé par les permissions de
  `~/.polygone` (accès même utilisateur uniquement).
