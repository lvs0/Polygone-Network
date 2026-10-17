# STAGING.md — Services parked (pas dans MVP)

> *Archive des services initialement prévus dans `ECOSYSTEM.md` qui ne sont pas livrés.*
> *Source : Conseil des Sages 2026-06-29 — Musk (couper 90%) + Gödel (documenter pour la postérité).*
> *Snapshot historique du 2026-08-06 : brain livré (pilot D4 — petals local).*
> *Note de sécurité Compute vérifiée le 2026-10-16 ci-dessous; les autres statuts de ce tableau restent à réconcilier avec `ECOSYSTEM.md`.*

---

## Vue d'ensemble

| Service | Statut | Livré en 1.0.0-rc2 ? |
|---------|--------|----------------------|
| **msg**    | 🟢 Live | ✅ messages E2E via relay aveugle (4/7) |
| **drive**  | 🟢 Live | ✅ fichiers E2E via relay aveugle (4/7) |
| **brain**  | 🟢 Live | ✅ IA locale (`polygone petals` → Ollama, zéro cloud) |
| **mesh**   | 🟢 Live | ✅ découverte LAN (`voisins`/`annoncer`, Phase 4) |
| `compute`  | 🟡 Partiel (MVP) | Correctif grant appliqué dans l’arbre local; tests ciblés passent, validation complète encore ouverte — voir ci-dessous |
| `hide`     | 🟢 Live (Phase 1 MVP) | ✅ SOCKS5 proxy via relay aveugle + exit node (single-hop, doc honnête) |
| `petals`   | ⚪ Staging | ❌ (distribution distribuée — le local est livré sous `brain`) |
| `shell`    | ⚪ Staging | ❌ |

**Comptage historique au 2026-08-06 seulement; consulter `ECOSYSTEM.md` pour le registre courant.**

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
  (`~/.polygone/reputation.json`) et affiché dans `polygone compute` —
  on préfère les fantômes qui délivrent.
- **Honnête** : sandbox *système* + WASM wasmi (memory-safe), réputation
  locale (pas d'échange de réputation réseau — Phase 8+). Le corps JSON des
  requêtes Compute, y compris la tâche ou le contenu WASM encodé en base64,
  est signé mais pas chiffré E2E; le relay et les observateurs du trajet
  peuvent en lire le contenu car le transport TCP courant n’utilise pas TLS.
  N’y envoyer ni secret, ni donnée personnelle, ni code confidentiel.
- **Grant — défaut détecté puis corrigé localement le 2026-10-16** : le
  worker construit maintenant le payload final par branche, puis `grant_for()`
  le signe une seule fois. Les 10 tests `grant_` et la suite Client complète
  (105/105, incluant le faux relais qui renvoie un grant au mauvais destinataire)
  passent; rustfmt et `git diff --check` passent. Tilleul a relu indépendamment
  le test E2E, confirmé son résultat et n’a trouvé aucun défaut; le gate grants
  ciblé est clos. Le workspace (176 tests + 1 doc-test), Clippy strict, build
  release, smoke CLI, loopback, rustfmt et diff-check passent sur l’arbre
  non commité, avant le lot TOFU courant. Aucun smoke réseau entre environnements
  n’a été exécuté; le gate Modal de ce cycle n’est pas une preuve physique. Le
  corps Compute reste lisible par le relay et les observateurs TCP sans TLS.
- **Store de confiance TOFU — P1/P2 corrigés, gate Windows ouvert** : le
  snapshot de validation (24 chemins) SHA `33626ec120864167a107ceaa66164317e2d6cc9577cd792a671b74389c5bd0bd`
  rapporte 206/206 tests, Clippy strict, fmt, release, smoke CLI et forensic
  loopback PASS; Tilleul confirme Client 134/134 et ne trouve plus de défaut
  P1/P2. Le test `peers_store_first_launch_creates_missing_parent` existe sous
  `#[cfg(windows)]`, mais n’a pas été compilé/exécuté; MinGW échoue dans
  `pqcrypto-mlkem` sur `features.h`, donc le comportement Windows reste
  non démontré. Modal et tests inter-hôtes restent non exécutés/suspendus.
  Les mises à jour documentaires sont postérieures à ce digest. Aucune
  durabilité universelle face aux courses/crashs n’est revendiquée.
  Voir [`RELEASE-STATUS.md`](./RELEASE-STATUS.md).
**Phase 8+ restante.** Le parcours demande→grant, dont le rejet E2E d’un
destinataire incorrect, est couvert par les tests ciblés. Restent l’échange de
réputation réseau et l’adversary budget; la couche d’allocation existe déjà
dans `polygoned` (policy GlowUp).

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

### Phase 2+ (post-MVP, documentée, pas implémentée ici)
- Multi-hop (chaînage de nœuds, re-chiffrement par hop).
- DNS via le tunnel (éviter les fuites DNS).
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
**Pourquoi parked.** Compromet la compatibilité « zéro contact cloud ». Weights quantization standardization absente. Pas de demande marché définie.
**Ré-introduction Phase 8+.** Si demande explicite post-MVP.

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
