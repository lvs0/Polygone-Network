# HIDE-SPEC.md — Polygone Hide (service ⚪ → 🟢)

> *Ré-introduction du service `hide` depuis STAGING.md, à la demande de Lévy
> (2026-08-09). Le design respecte les 5 axiomes de `PHILOSOPHY.md` et les
> conditions de ré-introduction de STAGING.md : doc honnête vs Tor, pas de
> promesse au-delà de ce que le code tient.*

---

## 1. Concept

**Polygone Hide** = un proxy **SOCKS5** local qui route le trafic TCP à
travers le réseau Polygone (relay aveugle + exit nodes) pour masquer
l'origine physique des connexions.

```
Application (curl, navigateur…)
   │  SOCKS5 (127.0.0.1:9050)
   ▼
polygone hide ──────────────► relay aveugle ──────────────► exit node
   │  CONNECT host:port           │  route (métadonnées)        │  CONNECT réel
   │  flux chiffré ML-KEM+AES     │  ne voit pas le contenu     │  ▼
   └──────────────────────────────┴─────────────────────────────┴──► destination
```

**Le « plus » vs Tor (argument honnête)** :
- Pas de directory authority centralisée : le mesh + relay forment le réseau.
- Le relay ne voit jamais le contenu : flux chiffré ML-KEM-1024 + AES-256-GCM
  (même pipeline que `msg`/`drive`, testé).
- L'exit node voit la destination réelle (comme un exit Tor) — **documenté**,
  pas caché.

## 2. Portée

### Phase 1 (cette mission) — MVP single-hop
- [ ] Sous-commande `polygone hide` : listener SOCKS5 sur `127.0.0.1:9050`
      (port déjà projeté dans `docs/config.md`).
- [ ] Négociation SOCKS5 (RFC 1928) : no-auth, CONNECT.
- [ ] Encapsulation de la demande `CONNECT host:port` dans le pipeline
      existant (`net.rs`, fragments NDJSON, ML-KEM/AES, signature ML-DSA).
- [ ] Côté exit node : `polygone ecouter --hide` — reçoit les demandes,
      établit la connexion TCP réelle, renvoie le flux.
- [ ] Streaming bidirectionnel (client ↔ exit node, par relais).
- [ ] Test d'intégration réel : un exit node local, `curl --socks5` à
      travers le proxy → réponse reçue.
- [ ] Doc : cette SPEC + section `THREAT_MODEL.md` + tradeoffs vs Tor
      (README « en cours » → « livré », ECOSYSTEM.md statut 🟢).

### Phase 2+ (post-MVP, documentée, pas implémentée ici)
- Multi-hop (chaînage de nœuds, re-chiffrement par hop).
- DNS via le tunnel (éviter les fuites DNS).
- Rotation d'exit nodes + réputation.
- Fingerprinting resistance (padding, tailles fixes).

## 3. Architecture technique

### Flux par session
```
client (hide)                              relay                        exit node (ecouter --hide)
   │  SOCKS5 CONNECT host:port                  │                              │
   │  → session_id = random hex                 │                              │
   │  → enveloppe KEM (host:port chiffré,       │                              │
   │     signée ML-DSA, ts anti-replay)         │                              │
   ├────────── 7 fragments NDJSON ─────────────►│  route sur `to`              │
   │                                            ├────────── fragments ────────►│  ≥4/7 → vérif sig →
   │                                            │                              │  déchiffre host:port →
   │                                            │                              │  TcpStream::connect
   │  ◄── fragments (réponse/refus) ────────────┤  ◄──────── fragments ────────┤  enveloppe réponse
   │  ≥4/7 → reconstruit → déchiffre            │                              │  (chiffrée + signée)
   ▼                                            ▼                              ▼
   octets du flux → SOCKS5 reply → app          relais aveugle des octets      données de la destination
```

### Réutilisation
- `crates/core` : ML-KEM, AES-GCM, BLAKE3, ML-DSA (sign), Shamir — rien à
  inventer.
- `crates/client/src/net.rs` : transport NDJSON existant — étendre le
  protocole avec un type de fragment `stream` (ou un en-tête de session
  dédié) plutôt que de dupliquer le transport.
- `crates/client/src/msg.rs` : modèle de reconstruction ≥4/7 → le réutiliser
  pour le streaming (avec fenêtre de session, pas un message unique).

### Honnêteté des limites (à documenter dans THREAT_MODEL.md)
- Le relay voit `from`/`to`/session/tailles (déjà documenté D5).
- L'exit node voit la destination (comme Tor exit) — documenté.
- Un seul hop : relay + exit node peuvent corréler (pas de multi-hop au MVP).
- Pas de padding au MVP : le fingerprinting par tailles/timings reste
  possible — **dit noir sur blanc**, pas de promesse Tor-level.
- Pas d'audit externe (condition STAGING.md : Trail of Bits ou équivalent)
  — **restera en attente**, mentionné dans le README.

## 4. Critères de succès (vérifiables)

1. `cargo test --workspace` vert (tests existants + nouveaux tests hide).
2. `clippy --all --all-targets -- -D warnings` exit 0.
3. Smoke réel : 2 nœuds locaux (exit node + client), `curl --socks5
   127.0.0.1:9050 http://example.com` → HTTP 200 (ou échec réseau documenté
   si le réseau local bloque).
4. Le relay (dans le test) ne reçoit aucun octet de contenu en clair —
   audit « on voit rien » étendu à Hide.
5. Axiome 4 respecté : `wc -l crates/client/src/*.rs | tail -1` ≤ 5000.

## 5. Contraintes de style (CLAUDE.md)
- Ne pas créer de fichiers inutiles ; les docs nouvelles uniquement si
  nécessaires (HIDE-SPEC.md, sections THREAT_MODEL).
- Fichiers < 500 lignes si possible ; validation aux frontières système.
- Zéro secret, zéro télémétrie, zéro compte.

---

*AGPL-3.0 · Posture honesty-first · « Le silence est le produit. »*
