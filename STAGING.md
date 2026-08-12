# STAGING.md — Services parked (pas dans MVP)

> *Archive des services initialement prévus dans `ECOSYSTEM.md` qui ne sont pas livrés.*
> *Source : Conseil des Sages 2026-06-29 — Musk (couper 90%) + Gödel (documenter pour la postérité).*
> *Mis à jour 2026-08-06 : brain livré (pilot D4 — petals local).*

---

## Vue d'ensemble

| Service | Statut | Livré en 1.0.0-rc2 ? |
|---------|--------|----------------------|
| **msg**    | 🟢 Live | ✅ messages E2E via relay aveugle (4/7) |
| **drive**  | 🟢 Live | ✅ fichiers E2E via relay aveugle (4/7) |
| **brain**  | 🟢 Live | ✅ IA locale (`polygone petals` → Ollama, zéro cloud) |
| **mesh**   | 🟢 Live | ✅ découverte LAN (`voisins`/`annoncer`, Phase 4) |
| `compute`  | 🟢 Live (MVP) | ✅ visibilité + prêt + exécution sandboxée — WASM/reputation en Phase 8 |
| `hide`     | ⚪ Staging | ❌ |
| `petals`   | ⚪ Staging | ❌ (distribution *distribuée* — le local est livré sous `brain`) |
| `shell`    | ⚪ Staging | ❌ |

**3 livrés + 1 protocole live · 3 archivés** (la coupe 75% tient, +2 ramenés en isolation).

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
  locale (pas de reçus signés ML-DSA ni d'échange réseau — Phase 8+).
  Requêtes non chiffrées E2E (métadonnées).
**Phase 8+ restante.** Reçus vérifiables (grants signés ML-DSA) + échange
de réputation réseau + adversary budget. La couche d'allocation existe déjà
dans `polygoned` (policy GlowUp).

---

## Service : `hide` (Polygone Hide)

**Concept.** SOCKS5 + HTTPS proxy à travers le mesh. Multi-hop routing.
**Pourquoi parked.** Tor existe. Polygone-hide doit prouver un *plus*. Pas d'audit Tor-level disponible. Multihop introduit du délai non maîtrisé.
**Ré-introduction Phase 8+.** 1 audit externe minimum (Trail of Bits) + traffic fingerprinting resistance + doc honnête vs Tor tradeoffs.

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
