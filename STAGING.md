# STAGING.md — Services parked (pas dans MVP)

> *Archive des services initialement prévus dans `ECOSYSTEM.md` qui ne sont pas livrés.*
> *Source : Conseil des Sages 2026-06-29 — Musk (couper 90%) + Gödel (documenter pour la postérité).*
> *Mis à jour 2026-08-06 : brain livré (pilot D4 — petals local).*
> *Mis à jour 2026-10-30 : serverless (crate + CLI) et planner petals entrent en « partiel ».*

---

## Vue d'ensemble

| Service | Statut | Livré ? |
|---------|--------|----------------------|
| **msg**    | 🟢 Live | ✅ messages E2E via relay aveugle (4/7) |
| **drive**  | 🟢 Live | ✅ fichiers E2E via relay aveugle (4/7) |
| **brain**  | 🟢 Live | ✅ IA locale (`polygone petals` → Ollama, zéro cloud) |
| **mesh**   | 🟢 Live | ✅ découverte LAN (`voisins`/`annoncer`, Phase 4) |
| `compute`  | 🟢 Live (MVP) | ✅ visibilité + prêt signé + shell systemd + WASM wasmi + réputation locale ; reçus d'exécution portables/réputation réseau restants |
| `hide`     | 🟢 Live (Phase 1 MVP) | ✅ SOCKS5 proxy via relay aveugle + exit node (single-hop) · 🟡 planification de circuit multi-hop, transport non |
| `serverless` | 🟡 Partiel | ❌ crate + CLI (Shamir transit 4/7, quorum ≥ 4 nœuds) — dispatch relay non branché |
| `petals`   | 🟡 Partiel | ❌ planner de pipeline livré (affectation des couches, débit estimé) — distribution des poids absente ; le local est livré sous `brain` |
| `shell`    | ⚪ Staging | ❌ |

**5 livrés** (`msg`, `drive`, `brain`, `mesh`, `compute`) **+ `hide` Phase 1 · 2 partiels** (`serverless`, `petals`) **· 1 archivé** (`shell`).
Un service n'est marqué 🟢 que s'il s'exécute ; un planner ou un crate sans transport reste 🟡.

---

## Service : `compute` (Polygone Compute)

**Concept.** Lend/borrow compute pour distributed work. Idle detection → allocation → exécution sandboxée.
**Statut 1.0.0-rc2 : 🟢 PROTOCOLE + EXÉCUTION LIVE (shell sandbox + WASM).**
- Visibilité : `polygone compute` — RAM libre locale + nœuds fantômes du LAN.
- Prêt : `polygone compute --emprunter <node> --via <relay>` → le fantôme
  (`ecouter --compute`) répond par un grant via le relay aveugle.
- **Exécution shell** : `polygone compute --executer "<cmd>" --emprunter <node>`
  → la tâche tourne DANS la sandbox systemd du fantôme (MemoryMax 256 Mo,
  NoNewPrivileges, ProtectSystem=strict, PrivateTmp, PrivateNetwork,
  CPUQuota 50 %) et la sortie revient via le relay.
- **Exécution WASM** : `polygone compute --wasm <fichier.wasm> --emprunter <node>`
  → le module tourne dans le **sandbox wasmi** du fantôme (pure Rust,
  memory-safe, vérifiable) — la Phase 8 du plan, livrée.
- **Réputation** : chaque résultat (grant/échec) est enregistré par nœud
  (`~/.polygone/reputation.json`) et affiché dans `polygone compute`. Les refus
  signés et les codes de sortie distants non nuls sont bien comptés comme échecs.
- **Honnête** : les grants sont signés ML-DSA-65 et vérifiés par le demandeur ;
  cela prouve l'origine de la réponse, pas la justesse du résultat. Le shell
  tourne dans une sandbox systemd (même UID utilisateur), WASM dans wasmi.
  Les requêtes compute ne sont pas chiffrées E2E et le relay voit leurs métadonnées.
**Phase 8+ restante.** Reçu signé de résultat d'exécution distinct du grant,
échange de réputation réseau, sélection automatique des pairs et adversary
budget. La couche d'allocation existe déjà dans `polygoned` (policy GlowUp).

---

## Service : `hide` (Polygone Hide)

**Concept.** SOCKS5 + HTTPS proxy à travers le mesh. Multi-hop routing.
**Statut 1.0.0-rc2 : 🟢 LIVE (Phase 1 MVP).**
- Sous-commande `polygone hide` : listener SOCKS5 sur `127.0.0.1:9050`.
- Négociation SOCKS5 (RFC 1928) : no-auth, CONNECT.
- Encapsulation de la demande `CONNECT host:port` dans le pipeline
  existant (`net.rs`, fragments NDJSON, ML-KEM/AES, signature ML-DSA).
- Côté exit node : `polygone ecouter --hide` — reçoit les demandes,
  établit la connexion TCP réelle, renvoie le flux.
- Streaming bidirectionnel (client ↔ exit node, par relais).
- Test d'intégration réel : `scripts/hide-smoke.sh` — 2 nœuds locaux
  (exit node + client), `curl --socks5` à travers le proxy → réponse reçue.
- Doc : `HIDE-SPEC.md` + section `THREAT_MODEL.md` + tradeoffs vs Tor
  (README « livré », ECOSYSTEM.md statut 🟢).

### Phase 2+ (post-MVP, partiellement planifiée)
- Multi-hop (chaînage de nœuds, re-chiffrement par hop) : **type de circuit
  livré** (1 à 3 sauts, sans doublon), **transport non** — le flux reste
  single-hop.
- DNS via le tunnel : la cible SOCKS5 est conservée sous forme de **nom**, pas
  résolue localement — la résolution appartient à l'exit node, ce qui évite la
  fuite DNS côté client. Le comportement de bout en bout reste à vérifier sur
  un exit node réel.
- Rotation d'exit nodes + réputation.
- Fingerprinting resistance (padding, tailles fixes).

---

## Service : `mesh` (Polygone Mesh local)

**Concept.** mDNS + BLE + Wi-Fi Direct discovery. Cluster LAN sans internet.
**Pourquoi parked.** La couche `p2p` actuelle (libp2p) couvre déjà mDNS + Kademlia. BLE/Wi-Fi Direct = autre protocole, orthogonal. Pas de valeur ajoutée démontrable.
**Ré-introduction.** Si un cas concret émerge (école sans internet, zone disaster).

---

## Service : `brain` (Polygone Brain)

**Concept.** Local LLM gateway (Notch / Ollama / llama.cpp / Petals).
**Statut 1.0.0-rc2 : 🟢 LIVRÉ.** `polygone petals status/models/ask` +
`polygone` TUI `:ia <question>` — IA locale via Ollama (défaut
127.0.0.1:11434), client HTTP minimal sans dépendance, zéro cloud.
**Ré-introduction de la *distribution*.** Le sharding distribué reste sous
`petals` (staging) — le local est suffisant pour l'isolation.

---

## Service : `petals` (Distributed LLM)

**Concept.** Peer shard model + parallel inference (BitTorrent-style).
**Statut : 🟡 PARTIEL — le planner existe, la distribution non.**
- **Livré** : `crates/petals/src/pipeline.rs` — à partir d'une liste de pairs
  (`PeerCapacity` : nœuds, couches disponibles, RAM, débit) et d'un modèle,
  calcule un `PipelinePlan` : affectation de plages de couches **contiguës**
  couvrant le modèle exactement, sans nœud en double, ordonnée par capacité
  décroissante. Refus explicite (`NoPeers`, `NotEnoughLayers`, `DuplicateNode`)
  plutôt qu'un plan silencieusement faux.
- **Absent** : transfert des poids entre pairs, protocole de forward/backward
  entre étapes, tolérance à la panne d'un pair en cours de pipeline.
- **Pourquoi ce n'est pas "livré"** : un plan n'exécute rien. Le déclarer vert
  serait exactement le genre de promesse que `STAGING.md` existe pour éviter.
- **Ré-introduction complète.** Phase 8+, si la distribution des poids devient
  un besoin réel.

---

## Service : `serverless` (Polygone Serverless)

**Concept.** Exécuter une fonction sur les nœuds du réseau, avec un résultat
vérifiable.
**Statut : 🟡 PARTIEL — le crate et la CLI existent, le transport non.**
- **Livré** (`crates/serverless`, `docs/SERVERLESS-SPEC.md`) :
  - split **Shamir 4-of-7 réel** de la charge utile (`sharks`, via
    `polygone-core`) — pour le transit, pas pour le calcul ;
  - reconstitution à 4 parts + vérification du hash BLAKE3 de la charge utile ;
  - **quorum** : `threshold` nœuds **distincts** doivent renvoyer le même hash
    de sortie ; un nœud ne vote qu'une fois ;
  - verdicts explicites `Agreed` / `Disagreement` / `Insufficient` ;
  - vérification des reçus ML-DSA-65 (`Receipt`) ; une sortie sans reçu valide
    est écartée.
- **Absent** : envoi des parts sur le relay, réception des sorties, exécution
  WASM côté nœud (le transport `compute` existe séparément et n'est pas câblé
  ici).
- **Honnête** : Shamir ne s'applique pas au calcul — `f(a) + f(b) ≠ f(a+b)`.
  Le calcul est **répliqué puis arbitré**, pas reconstruit à partir de parts.
  La conséquence est un coût de 7 exécutions pour un résultat ; c'est le prix
  de la vérifiabilité sans preuve cryptographique.

---

## Service : `shell` (Polygone Shell)

**Concept.** SSH-like remote shell via the mesh.
**Pourquoi parked.** SSH existe. Remote shell = augmentation surface d'attaque énorme. Mitnick peut pivoter. Pas de besoin urgent.
**Ré-introduction.** ⚠️ Doit être tabouillé par design (one-time token, no persistence, no auth replay). Probablement jamais.

---

## Conventions

- `Phase 8+` = explicit postponed. Ré-évalué en revue annuelle (pas en revue mensuelle).
- `Phase ∞` = pas une feature Polygone ; elle appartient à un autre produit.

## Pourquoi cette coupe est honnête

`PHILOSOPHY.md` Axiome 4 : « Le silence est le produit, pas le silence marketing. »

L'inverse du silence marketing = promettre 8 services pour en livrer 2. Coupe Musk = promettre 2 services et en livrer 2. C'est ce que fait v2.0.0-rc1.

---

*Hérite de : `ECOSYSTEM.md` (original 8 services), Conseil des Sages 2026-06-29 (Musk + Gödel).*
*AGPL-3.0 · Pas de monétisation.*
