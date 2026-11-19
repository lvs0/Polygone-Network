# DATA-FLOWS — flux de données réels

> Date : **2026-11-09**, HEAD `e2936a3`. Chaque flux renvoie au fichier qui
> l'implémente. Les échecs sont décrits avec leur comportement observé, pas
> seulement le chemin heureux.

---

## 1. Création et persistance d'identité

```
premier lancement
  → génère ML-KEM-1024 + ML-DSA-65
  → node_id = 16 premiers octets hex de BLAKE3(clé de signature)
  → écrit ~/.polygone/identity.json (atomique, 0600)
  → pseudonyme local
```

- Fichier : `crates/client/src/identity.rs`.
- Vérification : `polygone test` (autotest crypto), `polygone id`.
- Rotation : `polygone clef-tourner` signe la nouvelle clé avec l'ancienne,
  révoque l'ancienne, conserve le `node_id` KEM. Une rotation de la clé de
  **signature** change le `node_id` de relais — conséquence documentée dans
  `docs/adr/0001-authentification-du-relais.md`.
- Échec : permissions impossibles à poser → refus, pas d'identité en clair
  dans un répertoire partagé.

---

## 2. Message hors ligne

```
clair
 → KEM encapsule (clé publique du destinataire)        → kem_ct + secret partagé
 → BLAKE3 domain-séparé                               → clé de session 32 o
 → AES-256-GCM (nonce aléatoire 96 bits)              → chiffré
 → Shamir 4-of-7                                      → 7 fragments
 → texte de fil : KEM_CT / SENDER_PK / FRAG…
```

Réception : ≥ 4 fragments → reconstruction → déchiffrement → clair.
Fichier : `crates/client/src/msg.rs`. Une mauvaise clé échoue au déchiffrement ;
aucun fragment n'est accepté hors bornes (indices 1..=7, C(7,4)=35).

---

## 3. Message via le relais (plan réseau)

```
A: polygone envoyer --via relais --a B
  1. handshake AUTH_INIT/CHALLENGE/AUTH_RESP (signé)
  2. KEM + 7 fragments → 1 enveloppe KEM + 7 enveloppes FRAG
  3. chaque enveloppe : {kind, from, to, session, seq, type, idx…, sig}
  4. relais vérifie from == identité authentifiée, route vers B
B: polygone ecouter
  5. vérifie la signature ML-DSA-65 sur les octets canoniques
  6. vérifie que l'enveloppe est adressée à B
  7. bufferise par session, reconstitue à ≥ 4 fragments
  8. déchiffre, écrit dans ~/.polygone/received/
```

- Fichiers : `crates/client/src/net/mod.rs`, `crates/relay/src/relay.rs`.
- Anti-rejeu : `ts` signé, fenêtre ±300 s, cache par session.
- Toile de confiance : premier contact appris (TOFU), clé différente pour un
  `from` connu = rejet.
- Échecs :
  - relais indisponible → erreur explicite, rien n'est envoyé ;
  - pair hors ligne → fragments jetés (pas de file) ;
  - signature absente/invalide → rejet fail-closed ;
  - fragment dupliqué ou adresse incorrecte → rejet.

---

## 4. Découverte LAN

```
polygone annoncer --relay <addr>
  → choisit les interfaces autorisées (exclut loopback, link-local, VPN, conteneurs)
  → UDP broadcast 7642 : {node_id, relay, free_ram_mb?}
polygone voisins --duree N
  → écoute N secondes, agrège les pairs
```

- Fichier : `crates/client/src/mesh.rs`.
- `POLYGONE_MESH_IFACE` ne peut que **restreindre** les interfaces ; une
  allowlist ne ressuscite jamais une interface exclue.
- Échec : aucune interface autorisée → aucune annonce, pas de broadcast depuis
  `0.0.0.0`.

---

## 5. Tick du daemon et publication

```
toutes les 5 s
  capture système (CPU, RAM, réseau, GPU, stockage, activité)
    → GlowUpEngine::tick calcule une allocation (paliers + marges)
    → apply(nice) si demandé (aucun plafond cgroup)
    → runtime_state construit DaemonState
    → publish_shared écrit l'état sous RwLock
```

- Fichiers : `daemon/src/main.rs`, `daemon/src/runtime.rs`,
  `daemon/src/policy/glow_up.rs`.
- `runtime_state` est **pur** : aucune E/S, aucun verrou, aucune horloge.
- Une mesure absente (stockage) vaut zéro ; la capacité n'est jamais inventée.
- `shrinking` : pendant la contraction, les nouvelles allocations sont refusées.

---

## 6. Bail d'allocation (ledger)

```
client → alloc_request {cpu_cores, ram_gb, gpu_vram_gb?, storage_gb?}
daemon :
  1. valide la demande (NaN, ≤ 0, RAM arrondie à 0 → invalid_request)
  2. balaie les baux échus
  3. refuse si `shrinking`
  4. refuse si demande > total machine          → exceeds_measured
  5. refuse si demande > marge restante        → insufficient_headroom
  6. refuse si cœurs > pool restant            → insufficient_cpu
  7. refuse si VRAM > VRAM libre               → insufficient_vram
  8. refuse si stockage > stockage réservable  → insufficient_storage
  9. réserve sous un identifiant unique, persiste (atomique)
 10. répond alloc_granted {allocation_id, expires_at, enforced}
```

Libération : `release` idempotent. Renouvellement : `renew`, refusé si le bail
est inconnu ou échu. Déconnexion : `LedgerGuard::drop` libère tous les baux de
la connexion. Redémarrage : `load_from` recharge les baux, `adopt_legacy_grants`
donne une échéance aux baux historiques, `sweep_expired` libère les échus.

- Fichier : `daemon/src/socket.rs`.
- Persistance : temporaire + `sync_all` + `rename` + fsync du répertoire ;
  `0600`. Un fichier illisible est mis en quarantaine, jamais écrasé en silence.

---

## 7. Exécution d'un calcul (compute)

```
A: polygone compute --executer "<commande>" --via relais
  → enveloppe de demande de grant vers le nœud B
B: ecouter --compute
  → grant_for() vérifie to/session/from
  → exécute (systemd-run --user ou wasmi)
  → renvoie un résultat signé
A: vérifie la signature, affiche la sortie
```

- Fichiers : `crates/client/src/net/mod.rs`, `exec.rs`.
- Isolation : `MemoryMax`, `CPUQuota`, `NoNewPrivileges`, `ProtectSystem=strict`,
  `ProtectHome=yes`, `PrivateTmp`, `PrivateNetwork` ; WASM avec carburant borné.
- Limite : le shell partage l'UID ; ce n'est pas un bac à sable
  cryptographique. Deux exécutions concurrentes au maximum.
- Échec : timeout → unité transitoire arrêtée ; code de sortie non nul →
  résultat signé d'échec, refusé par l'appelant qui attendait un succès.

---

## 8. Blocs adressés par contenu

```
put(bytes) → hash = BLAKE3(bytes) → écrit le fichier
get(hash)  → lit si présent
serve      → TCP 7643 : répond à GET <hash>
fetch(hash, peer) → récupère et vérifie le hash
```

- Fichier : `crates/client/src/blocks.rs`.
- Taille maximale : 1 Mio par bloc ; 16 connexions.
- Un nœud n'ouvre le port qu'après obtention d'un bail du daemon et s'arrête si
  le bail est perdu ou si la politique passe à `shrinking`.
- Non prouvé : transfert entre deux hôtes physiques (`BLOCKS.9`).

---

## 9. Gateway : appairage, heartbeat, latence

```
appairage
  → POST /api/v1/pair/request {device_name, kem_pk, sign_pk}
  → 200 {token, expires_in_seconds: 300}
  → POST /api/v1/pair/accept {token}  (usage unique)
  → nœud ajouté avec sa clé de signature épinglée

vie
  → POST /api/v1/nodes/heartbeat {node_id, timestamp, signature_hex}
     signature vérifiée contre la clé épinglée, fenêtre ±300 s
     → last_seen mis à jour, nodes.json écrit atomiquement

latence
  → POST /api/v1/nodes/ping  → challenge à usage unique, TTL 30 s
  → POST /api/v1/nodes/pong  → rtt_ms borné, un seul usage
     → latency_ms écrit seulement après un pong valide
```

- Fichiers : `crates/gateway/src/routes.rs`, `lib.rs`, `status.rs`.
- Un heartbeat inconnu ou mal signé ne touche ni la table ni le fichier.
- `latency_ms_median` vaut `null` tant qu'aucune mesure n'existe — jamais `0`.

---

## 10. Hide (SOCKS5 via relais)

```
client SOCKS5 → polygone hide
  → demande de sortie vers le nœud exit (destinataire chiffré)
  → grant chiffré accordé
  → tunnel de chunks chiffrés via le relais
  → l'exit ouvre la connexion réelle
```

- Fichier : `crates/client/src/hide.rs`.
- Limite : **mono-saut**, pas d'anonymat garanti, pas de multi-hop prouvé.

---

## 11. Duress et désinstallation

- `duress --confirmer` : détruit identité, reçus et réputation ; irréversible.
- `desinstaller` : énumère, exige `--confirmer`, ne supprime que sous ses
  racines, refuse `/` et `$HOME`, ne laisse aucun résidu.
- Fichiers : `duress.rs`, `uninstall.rs`.

---

## 12. Flux absents (à ne pas supposer)

- Aucun flux de tâche complet (publication → sélection → vérification →
  archivage).
- Aucun flux d'événements vers une UI.
- Aucun flux d'historique de consommation.
- Aucun flux de métriques.
- Aucun flux de mise à jour signée.
- Aucun flux multi-hop Hide.
