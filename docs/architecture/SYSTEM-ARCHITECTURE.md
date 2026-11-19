# SYSTEM-ARCHITECTURE — architecture canonique du socle Polygone

> Date : **2026-11-09**. Décrit le code au HEAD `e2936a3` (+ correctif de
> persistance du ledger). Source de vérité : le code et les tests ; ce document
> les suit, il ne les précède pas.
>
> Documents liés : [`COMPONENT-MAP.md`](./COMPONENT-MAP.md),
> [`DATA-FLOWS.md`](./DATA-FLOWS.md),
> [`API-CONTRACTS.md`](./API-CONTRACTS.md),
> [`../security/THREAT-MODEL.md`](../security/THREAT-MODEL.md).

---

## 1. Ce que le système est réellement

Polygone est un ensemble de processus indépendants qui communiquent par deux
protocoles distincts :

1. **Un protocole local** (socket Unix, JSON par ligne) entre le daemon de
   ressources et les outils de l'utilisateur.
2. **Un protocole réseau** (TCP, NDJSON) entre un client et un relais, puis
   entre pairs via ce relais.

Il n'existe pas de serveur central obligatoire. Le relais est un routeur
aveugle : il voit les métadonnées nécessaires au routage, jamais le contenu.

Le produit actuel est une **CLI/TUI** (`polygone`) et une **gateway HTTP locale**
(`polygone-gateway`) qui sert une PWA. Il n'existe pas encore d'application
graphique native ni d'API de tâches.

---

## 2. Couches

```
┌─────────────────────────────────────────────────────────────┐
│  Présentation                                               │
│  polygone (CLI/TUI)          gateway HTTP + PWA statique    │
├─────────────────────────────────────────────────────────────┤
│  Services produit (partiels)                                │
│  msg · blocs · compute (RES) · Hide · pause · diagnostic     │
├─────────────────────────────────────────────────────────────┤
│  Moteur local                                               │
│  polygoned : mesure → politique → réservations → publication │
├─────────────────────────────────────────────────────────────┤
│  Transport                                                  │
│  relais TCP authentifié · mesh UDP local · socket Unix       │
├─────────────────────────────────────────────────────────────┤
│  Primitive                                                  │
│  ML-KEM-1024 · ML-DSA-65 · AES-256-GCM · BLAKE3 · Shamir 4/7 │
└─────────────────────────────────────────────────────────────┘
```

Chaque couche ne dépend que de la couche inférieure. `crates/core` n'a **aucune
dépendance réseau** ; c'est ce qui permet de tester la cryptographie sans
socket.

---

## 3. Composants et responsabilités

| Composant | Responsabilité unique | Ne fait pas |
|---|---|---|
| `polygoned` | mesurer la machine, décider une allocation, publier un état, tenir le ledger de réservations | ne déchiffre aucun message, ne route aucun pair, n'exécute aucune tâche |
| `polygone` | identité, chiffrement, messages, blocs, découverte, exécution, diagnostic | ne décide pas des ressources ; demande un bail |
| `polygone-relay` | router des enveloppes entre `node_id` connectés, authentifier la connexion | ne lit pas les charges utiles, ne persiste rien, ne stocke pas hors ligne |
| `polygone-gateway` | API HTTP locale, appairage, heartbeat, latence, fichiers statiques | ne parle pas au daemon, ne signe rien à la place d'un nœud |
| `polygone-petals` | décrire et planifier des backends d'inférence locaux | ne distribue pas encore de couches |
| `polygone-serverless` | découper/reconstituer, voter un quorum | n'est pas câblé à un dispatch réseau |

### Règle de propriété

Un état a **un seul écrivain**. Les invariants structurels sont testés :

- `run_tick` est le seul écrivain de `DaemonState`
  (`daemon/src/runtime.rs::test_tick_is_the_only_writer`) ;
- le ledger n'est muté que sous son `Mutex`, via `dispatch` et `LedgerGuard` ;
- `publish_shared` est l'unique porte de publication.

---

## 4. Topologie de déploiement

### 4.1 Une machine

```
utilisateur
   │
   ├─ polygone … ──socket Unix──> polygoned
   │      │
   │      └──TCP──> relais distant (ou local)
   │
   └─ navigateur ──HTTP loopback──> polygone-gateway ──> web/ (statique)
```

- Le socket daemon est dans `~/.polygone/daemon.sock`, permissions `0600`.
- La gateway écoute sur une adresse locale et sert l'API sous `/api/v1/`.
- Aucun composant n'ouvre de port public par défaut.

### 4.2 Deux machines

```
A: polygone ──TCP──> relais <──TCP── polygone :B
```

Le relais doit être joignable par les deux pairs. Il ne connaît que leurs
`node_id`, pas leurs identités civiles. C'est la topologie que la procédure
`docs/P0-DEUX-MACHINES.md` doit valider sur deux hôtes physiques — **non
exécutée à ce jour**.

---

## 5. États et transitions

### 5.1 Boucle du daemon

```
démarrage
  │  charge la config, le ledger, ouvre le socket
  ▼
tick (toutes les 5 s)
  │  capture système → politique → apply(nice) → publication
  ├─ échec de capture ──> erreur journalisée, état précédent conservé
  ├─ `shrinking` ──────> nouvelles allocations refusées
  ▼
arrêt gracieux (SIGINT/SIGTERM)
  │  retire le socket, libère les connexions
  ▼
arrêt
```

Invariant : le daemon ne publie jamais un état qu'il n'a pas mesuré. Une mesure
absente vaut zéro et refuse l'allocation correspondante (fail-closed).

### 5.2 Bail d'allocation

```
libre ──alloc──> réservé (expires_at) ──renew──> réservé (nouvelle échéance)
                     │
                     ├──release──> libre
                     ├──expiration──> libre (sweep)
                     └──déconnexion──> libre (LedgerGuard::drop)
```

Un bail expiré n'est jamais ressuscité par `renew`. Un redémarrage recharge les
baux ; un bail hérité sans échéance reçoit une grâce d'un bail.

### 5.3 Connexion au relais

```
TCP ouvert
  → AUTH_INIT (node_id, preuves)
  → CHALLENGE
  → AUTH_RESP (signature)
  → AUTH_OK ──> routage
  ✗ toute erreur ──> AUTH_DENIED, fermeture
```

Une connexion ne peut pas émettre sous l'identité d'un autre `node_id` : le
relais compare `from` à l'identité authentifiée et jette la trame sinon.

### 5.4 Cycle de tâche

**État actuel** : partiel. Une demande de calcul (`compute`) obtient un grant
d'un nœud, exécute du shell ou du WASM, et renvoie un résultat. Il n'existe pas
encore de machine à états complète (création → publication → sélection →
réservation → exécution → vérification → archivage). Voir
`project/FOUNDATION-ROADMAP.md` `F1-TASK-01/02/03`.

---

## 6. Persistance

| Donnée | Format | Emplacement | Écriture | Perte |
|---|---|---|---|---|
| identité | JSON | `~/.polygone/identity.json` | atomique, 0600 | non |
| pairs connus | JSON v2 | `~/.polygone/peers.json` | atomique, 0600, migration v1 | non |
| réservations | JSON | `~/.polygone/ledger.json` | **atomique**, 0600, quarantaine si illisible | non |
| nœuds gateway | JSON | chemin de la gateway (`nodes.json`) | `tmp` + `rename`, verrou | non |
| blocs | fichiers | `~/.polygone/blocks/` | adressés par BLAKE3 | non |
| config daemon | TOML | `~/.config/polygone/daemon.toml` | atomique | non |
| intention de pause | JSON | `~/.polygone/pause.json` | atomique | survit au processus, pas au reboot par conception |
| routage relais | mémoire | — | — | oui, par conception |

Aucune donnée critique n'est écrasée sans trace : un fichier illisible de ledger
est mis en quarantaine avant que le daemon ne reparte d'un état vierge.

---

## 7. Frontières de confiance et isolation

| Frontière | Mécanisme | Limite assumée |
|---|---|---|
| utilisateur ↔ daemon | permissions du socket Unix (`0600`) | tout processus du même utilisateur peut parler au daemon |
| client ↔ relais | handshake signé, `node_id` dérivé de la clé de signature | le relais voit le graphe et les IP |
| pair ↔ pair | ML-DSA-65, TOFU ancré | premier contact accepté puis appris |
| navigateur ↔ gateway | loopback, jeton d'appairage, heartbeat signé | pas de TLS ; un processus local peut lire le trafic loopback |
| tâche ↔ hôte | `systemd-run --user` ou WASM | même UID pour le shell : pas de frontière cryptographique |
| disque ↔ attaquant | permissions 0600, `duress` | pas de chiffrement au repos |

---

## 8. Points de défaillance

| Défaillance | Effet | Comportement prévu |
|---|---|---|
| daemon absent | plus de bail, plus d'état de ressources | le client refuse d'ouvrir le port de blocs ; `doctor` dit « non mesuré » |
| daemon tué | baux non libérés en mémoire | baux persistés + expiration ; redémarrage les recharge |
| ledger corrompu | réservations illisibles | quarantaine + ledger vierge + journal d'erreur |
| relais indisponible | messages non routés | erreur explicite ; les fragments d'un pair hors ligne sont jetés |
| pair déconnecté | session incomplète | le destinataire ne reconstitue pas ; le relais ne bufferise pas |
| gateway arrêtée | PWA et API indisponibles | aucune donnée simulée ; la PWA affiche « — » |
| horloge dérivée | rejet anti-rejeu | fenêtre ±300 s ; refus explicite |
| disque plein | écriture impossible | erreur remontée ; le ledger n'est pas tronqué (écriture atomique) |

---

## 9. Invariants

1. Aucune ressource n'est annoncée sans mesure ; une mesure absente vaut zéro.
2. Une allocation accordée est une **licence**, sauf si `enforced` est vrai —
   ce qui n'arrive aujourd'hui que si une contrainte OS est réellement mesurée.
3. Le total publié n'est jamais une constante : il vient du snapshot.
4. Un bail expiré ne peut pas être renouvelé.
5. Un pair ne peut pas usurper le `from` d'un autre au relais.
6. Le relais ne voit ni le contenu ni le nom des fichiers.
7. Le daemon n'écrit jamais de plafond mémoire de cgroup.
8. Un fichier d'état critique est écrit atomiquement ou n'est pas écrit.

---

## 10. Ce que cette architecture ne prétend pas

- Elle ne garantit pas l'anonymat : le relais voit les métadonnées.
- Elle ne garantit pas l'isolation forte des tâches : le shell partage l'UID.
- Elle ne garantit pas la disponibilité d'un message si le pair est hors ligne.
- Elle ne fournit pas encore de calcul distribué complet ni d'API produit.
- Elle n'a pas été validée entre deux machines physiques.
