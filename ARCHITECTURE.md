# ARCHITECTURE.md — how it's built

> *For contributors. Read [`ECOSYSTEM.md`](./ECOSYSTEM.md) first to
> understand what we are building. This document is about how.*
>
> **Status 2026-11-14** : this document describes the **real** workspace
> (**8 members**, 4 product binaries + 1 lab opt-in, including the experimental
> `fabric` crate). The
> previous `src/` monolith (libp2p, HTTP
> relay, 4-tab TUI) was archived to `archive/2026-07-src/` — it is not built,
> not tested, not the product. The code is the source of truth; this document
> follows it.
>
> *Correction du 2026-11-03* : la version précédente de ce document annonçait
> « 4 crates, ~9 600 LOC, 109 tests » (état du 2026-08-07). C'était vrai à
> cette date. `gateway`, `petals` et `serverless` ont été intégrés le
> 2026-10-29 sans que ce fichier suive. Les chiffres ci-dessous ont été
> **mesurés** : `find -name '*.rs' | wc -l` pour les lignes, et
> `cargo test --workspace -j 2` (**554 tests, 0 échec**, mesure du 2026-11-15)
> pour les tests.

---

## 1. Workspace layout — 8 members, 4 product binaries + 1 lab

```
polygone/
├── Cargo.toml                    workspace, resolver 2, edition 2021
├── README.md                     manifesto + quickstart
├── ECOSYSTEM.md                  the mother file (single source of truth)
├── ARCHITECTURE.md               this file
├── PHILOSOPHY.md                 the 5 axioms (invariants, not marketing)
├── STAGING.md                    what is parked, and under which conditions
│
├── crates/
│   ├── core/                     no network deps — shared primitives
│   │   ├── crypto/               kem.rs · shamir.rs · symmetric.rs
│   │   ├── envelope.rs           Envelope / Fragment types + wire helpers
│   │   ├── sign.rs               ML-DSA-65 (FIPS 204) signer/verifier
│   │   ├── identity.rs           NodeId / SessionId
│   │   ├── time_sync/            NTP-like clock sync engine (CBOR)
│   │   ├── error.rs              PolygoneError / Result
│   │   └── lib.rs                re-exports
│   │
│   ├── client/                   the product — one binary: `polygone`
│   │   ├── main.rs               clap CLI + command dispatcher
│   │   ├── msg.rs                offline pipeline: KEM → KDF → AES → Shamir
│   │   ├── net/                  real network transport (plane 2)
│   │   ├── identity.rs           ~/.polygone/identity.json (chmod 600)
│   │   ├── blocks.rs             content-addressed store (BLAKE3), port 7643
│   │   ├── calc.rs               computation cache, keyed BLAKE3(version‖input)
│   │   ├── node.rs               daemon lease: serve, stop on lease loss
│   │   ├── serverless.rs         client side of split/reassemble
│   │   ├── exec.rs               RES execution (systemd-run + wasmi)
│   │   ├── hide.rs               SOCKS5 via relay (mono-saut)
│   │   ├── mesh.rs               LAN discovery (UDP broadcast, port 7642)
│   │   ├── reputation.rs         ghost-node trust ledger
│   │   ├── petals.rs             local AI (Ollama HTTP, no cloud)
│   │   ├── product.rs            packaging helpers for the single-command product
│   │   ├── duress.rs             kill-switch (Axiome 5, manuel)
│   │   ├── demo.rs               flagship E2E demo (in-process)
│   │   ├── self_test.rs          crypto self-test suite
│   │   ├── node_isolation.rs     tests du bail et de l'isolation
│   │   ├── testutil.rs           fixtures partagées
│   │   ├── tui.rs                vim-style command TUI (:envoyer / :quitter)
│   │   ├── drive.rs              PRÉSENT MAIS NON MONTÉ (zéro appelant)
│   │   └── brain.rs              PRÉSENT MAIS NON MONTÉ (zéro appelant)
│   │
│   ├── relay/                    the blind relay — one binary: `polygone-relay`
│   │   ├── main.rs               CLI (port, bind)
│   │   └── relay.rs              async TCP server, in-memory routing table
│   │
│   ├── gateway/                  HTTP loopback — one binary: `polygone-gateway`
│   │   ├── lib.rs                Gateway · NetworkStatus · PairRequest
│   │   ├── http.rs               socket TCP réel (testé dans tests/serve.rs)
│   │   ├── routes.rs             routes JSON + table de statut
│   │   ├── static_files.rs       web/ · PWA (POLYGONE_SITE_DIR)
│   │   └── status.rs             état réseau publié (table en mémoire)
│   │
│   ├── petals/                   inference backend planner — library only
│   │   ├── backends.rs           Ollama · vLLM · llama.cpp
│   │   ├── pipeline.rs           planner de pipeline (aucun consommateur hors tests)
│   │   ├── daemon.rs             client de démon petals
│   │   ├── models.rs             ModelRegistry
│   │   └── benchmarks.rs         BenchmarkConfig / BenchmarkResult
│   │
│   ├── serverless/               split/reassemble + quorum — library only
│   │   ├── request.rs            Shamir split, reassemble (tests négatifs)
│   │   └── quorum.rs             seuil de votes (jamais appelé par le CLI)
│   │
│   ├── fabric/                   lab opt-in — `polygone-fabric` (feature lab)
│   │   ├── model.rs              capacités, enveloppe, périmètres
│   │   ├── planner.rs            placement local/distant
│   │   ├── transport.rs          NDJSON TCP + nœuds mémoire
│   │   ├── engine.rs             vérification, reprise
│   │   ├── learning.rs           EWMA locale
│   │   └── store.rs              CAS BLAKE3 (explicitement pas Babel)
│   │
│   └── (daemon lives at repo root, see §7)
│
├── daemon/                       the system daemon — one binary: `polygoned`
│   ├── main.rs / lib.rs          loop + command socket
│   ├── system.rs                 SystemSnapshot::capture (/proc, nvidia-smi)
│   ├── resources/                linux.rs (real) · macos.rs (real) · windows.rs (stub)
│   ├── policy/glow_up.rs         the allocation policy engine
│   ├── allocator.rs              legacy allocator (see §7 — not used)
│   ├── bandwidth.rs / cpu.rs / gpu.rs   per-resource computations
│   └── socket.rs                 command socket (~/.polygone/daemon.sock)
│
├── scripts/                      install.sh · demo.sh
├── docs/                         cli.md · config.md · threat-*.md · STRATEGIE.md
├── site/ · index.html            landing / design-system page
└── .github/workflows/            ci.yml · release.yml
```

**Reality check — measured 2026-11-05 :**

| Crate        | Fichiers | LOC (rust) | Binaries             | Tests |
| ------------ | -------- | ---------- | -------------------- | ----- |
| `core` | 15 | 3655 | — (lib) | 77 |
| `client` | 33 | 16842 | `polygone` | 217 |
| `relay` | 4 | 1675 | `polygone-relay` | 28 |
| `gateway` | 13 | 5785 | `polygone-gateway` | 96 |
| `petals` | 12 | 3104 | — (lib) | 19 |
| `serverless` | 3 | 776 | — (lib) | 17 |
| `fabric` | 8 | 3026 | `polygone-fabric` | 32 |
| `daemon` | 17 | 11224 | `polygoned` | 133 |
| doc-tests | — | — | — | comptés dans leur crate |
| **Total** | **105** | **46087** | **5 binaires** | **619** |

> Base de mesure initiale du **2026-11-05** après `GW.2` (latence ping/pong), `SEC.1`
> (modèle de menace par service) et `CLI.6` (refus traduits en actions) :
> 88 fichiers Rust, 35597 lignes et 465 tests, dont le contrôle de couverture de `docs/INDEX.md`.
> *Mise à jour du 2026-11-09* : la réconciliation canonique (`docs/decisions/ADR-0001`)
> ajoute quatre fichiers Rust issus de `realign/v2` et du correctif P0
> (`relay/auth_tests.rs`, `client/net/s1_tests.rs`, `core/sign/node_id_tests.rs`,
> `client/tests/`) ; les compteurs de tests restent à recomputer par `make test`.
> Re-mesuré le **2026-11-09** sur `reconcile/2026-11` : **90 fichiers Rust et 37 392 lignes**, après l'ajout du modèle de tâche (`crates/core/src/task.rs`), du magasin et de l'API produit (`crates/gateway/src/tasks.rs`), de leurs 25 tests et des correctifs précédents. Une exécution workspace complète donne **490 tests, 0 échec**.
> Re-mesuré le **2026-11-13** après `F1-RES-03` (plafond CPU par palier) et
> `F1-SEC-02` (politique de confiance stricte) : **90 fichiers, 37 929 lignes,
> 499 tests, 0 échec**. Puis `F1-PROTO-01` (fenêtre v1/v2 et capacités) :
> **38 439 lignes, 507 tests, 0 échec**. Puis `F1-TASK-02` (exécuteur local
> `echo`/`blake3` et reprise), puis `F1-TASK-03` (reçu ML-DSA-65 lié à la
> tâche, au producteur et à la clé) : **38 876 lignes, 512 tests, 0 échec**.
> **2026-11-14** : ajout du prototype `crates/fabric` (8 fichiers, 3 026 lignes,
> 32 tests, puis P0 E2E/sandbox). Puis le 2026-11-15 la vérification L1 (recalcul
> local) et les mesures de quota CPU/mémoire. Total mesuré : **98 fichiers, 42 573 lignes, 5 binaires (dont 1
> lab opt-in), 554 tests,
> 0 échec**. Les 512 tests antérieurs restent verts ; les 32 nouveaux couvrent
> le placement, le TCP loopback, la vérification et le stockage expérimental.
> **2026-11-15 (session 10)** : `crates/core/src/reuse.rs` (Device Reuse,
> 8 tests) et `crates/gateway/src/diagnostics.rs` (rapport agrégé, 2 tests)
> portent la mesure à **100 fichiers, 43 350 lignes, 565 tests, 0 échec**.
> **2026-11-15 (audit Akash, intégration)** : `crates/core/src/evidence.rs`
> (preuve canonique `polygone.evidence.v1`, 17 tests, **non câblée**) porte la
> mesure à **101 fichiers, 44 000 lignes, 582 tests, 0 échec**.
> **2026-11-19 (production, mandat opérateur)** : câblage de la preuve L1
> (`net::persist_l1_evidence`, ADR-0003, 4 tests round-trip/disjonction/refus/sujet)
> et preuve deux hôtes (`docs/proofs/P0-DEUX-HOTES-2026-11-19.md`) :
> **101 fichiers, 44 332 lignes, 586 tests, 0 échec**.
> **2026-11-19 (soir, réconciliation)** : re-mesuré après le correctif macOS/BSD
> `8352c5c` (socket accepté `O_NONBLOCK`) et la fusion `github/main` (ADR-0004,
> **aucun fichier Rust modifié** — 24 ajouts documentaires/scripts de septembre) :
> **101 fichiers, 44 332 lignes, 586 tests, 0 échec** ; clippy workspace RC=0,
> `check-ci-honesty` OK, gitleaks 313 commits sans fuite.
> **2026-11-19 (nuit, prototype `prototype-complet`)** : domaines locaux
> F1-API-01 livrés (`local.rs`, `relay.rs`, `events.rs`, 6 routes, contrats
> §2.4), capacité `ledger` additive du daemon, bus d'événements borné
> (F1-EVT-01), hooks d'événements de tâches, 3 écrans web. Mesure :
> **105 fichiers, 45 853 lignes, 612 tests, 0 échec** (+4 fichiers gateway,
> +1 521 lignes ; +25 tests gateway dont 14 tests de contrat `local_api.rs`,
> +1 test ledger daemon). Clippy workspace `-D warnings` RC=0. Smoke vivant
> daemon + gateway : tâches signées, événements, 503 daemon absent, refus relayés.
> **2026-11-20 (`mission/nodes-revocation`)** : domaine nodes local livré
> (`GET /api/v1/nodes`, `POST /api/v1/nodes/{id}/revoke`, contrats §2.5,
> événement `node_revoked`), écran Réseau/Appareils (`mesh.html`/`mesh.js`),
> coque PWA `polygone-v4`. Mesure : **105 fichiers, 46 087 lignes,
> 619 tests, 0 échec** (+234 lignes gateway, +7 tests nodes dont heartbeat
> bloqué après révocation). Clippy workspace `-D warnings` RC=0, fmt RC=0.
> Le 4 novembre, `CLI.1` ajoute l'isolation OS **lue** du cgroup au diagnostic,
> `CLI.2` une pause bornée dont l'intention survit au processus mais pas au reboot,
> `CLI.5` une désinstallation énumérée qui refuse de sortir de ses racines, et
> `DOC.3` un bandeau `Instantané daté` sur les 29 documents historiques, dont
> l'absence fait échouer `scripts/check-docs.mjs` ; `CI.1` une matrice Windows
> honnête ; `OPS.2` des compteurs de refus par motif, lus par `polygoned status`
> et `doctor`, avec « non mesurés » plutôt que zéro quand le daemon est absent ;
> `SEC.8` un test qui refuse un secret suivi par git, avec sa preuve négative ;
> `CLI.3` trois profils d'usage (`sobre|equilibre|genereux`) qui écrivent une
> config réellement relue par `polygoned`, sans jamais annoncer de limite OS ;
> `GW.1` un heartbeat ML-DSA-65 vérifié contre la clé épinglée à l'appairage,
> avec `last_seen` écrit atomiquement et rechargé au démarrage ; `GW.2` une
> latence mesurée par ping/pong signé, challenge à usage unique et médiane qui
> n'utilise que des nœuds réellement mesurés ; `SEC.1` un modèle de menace par
> service (`docs/THREAT-MODEL-SERVICES.md`) qui cite les tests existants ;
> `CLI.6` une table exhaustive des motifs de refus avec cause et action,
> le code technique toujours affiché ; `FV-SEC-10` une limite par identité
> (100 env/s) partagée entre reconnexions, qui n'affecte jamais les autres pairs ;
> `FV-SEC-05` une rotation de clé de signature qui signe la nouvelle clé avec
> l'ancienne, révoque l'ancienne et survit au redémarrage.
> Le contrôleur `scripts/check-docs.mjs` recalcule ces valeurs et échoue si elles
> divergent — c'est lui qui a trouvé les six écarts précédents.

Méthode : `find <crate> -name '*.rs' | wc -l` pour les lignes (fichiers de test
comptés dans leur crate), `cargo test --workspace -j 2` pour les tests — **619 tests, 0 échec**
> (recompté le 2026-11-20 après le domaine nodes local, 25 suites).
Les tests sont comptés par crate de rattachement, pas par fichier.

No `unsafe` except libc calls in the daemon. Zero libp2p. The crypto is
real: `pqcrypto-mlkem` (FIPS 203) and `pqcrypto-mldsa` (FIPS 204) with
their exact sizes checked by tests.

---

## 2. The offline pipeline (msg.rs) — no network required

What `polygone envoyer <texte>` does, end to end:

```
plaintext
   │
   ▼
KEM encapsulate  (recipient's ML-KEM-1024 public key)   → kem_ct + ss
   │
   ▼
KDF BLAKE3       (domain-separated: "polygone session key v1")  → 32B key
   │
   ▼
AES-256-GCM      (random 96-bit nonce)                   → ciphertext
   │
   ▼
Shamir 4-of-7    (split ciphertext into 7 shares)        → 7 fragments
   │
   ▼
wire text        "KEM_CT:<hex>:/SENDER_PK:<hex>:/FRAG:<b64>,<b64>,..."
```

`polygone recevoir <fichier>` reverses it: ≥4 fragments → Shamir
reconstruction → AES-GCM decrypt → plaintext. The sender's public key
travels in the wire text so the recipient knows *who to reply to*.

**Security posture :** KEM IND-CCA2 (ML-KEM-1024) + AES-256-GCM. The
session key is `ZeroizeOnDrop`. Wrong-key decrypt is tested to fail.

---

## 3. The network pipeline (net.rs) — plane 2, real transport

TCP, newline-delimited JSON (NDJSON), through the blind relay.

### Wire contract

```json
{"kind":"fragment","from":"<node_id>","to":"<node_id>","session":"<hex>",
 "seq":0,"type":"kem"|"frag","idx":0,"threshold":4,"total":7,
 "payload":[...],
 "sig":"<ML-DSA-65 signature, KEM envelope only>",
 "signer":"<sender ML-DSA pk hex, KEM envelope only>",
 "name_ct":"<file name encrypted with the session key, KEM envelope of a file only>"}
```

### Handshake

```
client → relay : HELLO <node_id>\n
relay   → peer  : HELLO_OK\n      (registration accepted — the slot is yours)
relay   → peer  : HELLO_DENIED\n  (another connection already owns this node_id)
```

`node_id` = first 16 hex chars of the **KEM public key** (stable,
derived, persistent). The relay routes fragments to connected node_ids.
Registration is **acknowledged**: a duplicate `HELLO` for a live node_id
is refused *and the claimant is told* — no last-writer-wins, no silent
slot theft. The client waits for the ack before sending a single byte.

### Signatures — "c'est bien Alice"

Every message is signed by the sender's ML-DSA-65 key. The signature
covers the canonical bytes `session ‖ from ‖ to ‖ kem_ct ‖ ciphertext`
(Shamir deterministically rebuilds the same ciphertext from any 4-of-7
fragments, so the receiver verifies before decrypting). The receiver
fails closed on: missing signature, unverifiable signature, envelope not
addressed to it, duplicate fragment index. A `known_peers` map (`from` →
expected ML-DSA pk) turns first-contact trust (TOFU) into a real trust
anchor: a peer whose signing key is known cannot be impersonated.

### File names are out-of-band

The relay never sees a file name: `name_ct` is the name encrypted with
the session key, which only the recipient can derive. The relay sees
opaque bytes.

### Send

`envoyer --via <relay:port> --a <dest_node_id> <message>` :

1. `msg::send_bytes` → KEM_CT + 7 fragments (as in §2).
2. 1 KEM envelope + 7 fragment envelopes written to the relay.
3. The relay forwards each to `to` if connected; otherwise drops.

### Receive

`ecouter <relay:port>` :

1. `HELLO <node_id>` to the relay, then read lines.
2. Buffer envelopes by `session`; when ≥ `threshold` fragments arrive →
   Shamir reconstruct → decrypt.
3. Delivers the message / writes the file to `~/.polygone/received/`.

### Mesh (Phase 4)

`annoncer` / `voisins` / `ecouter --a` : UDP broadcast on port 7642.
Each announcement carries node_id + relay address + free RAM, so peers
can discover a relay without configuration.

---

## 4. The relay (crates/relay) — blind, not omniscient

An async TCP server with an in-memory routing table — **16 shards** of
`HashMap<node_id, write_half>`, so forwarding is parallel, not
serialized behind one lock. It:

- reads **only** `kind`, `from`, `to`, `session` to route (payloads pass
  verbatim);
- **drops any envelope whose `from` differs from the HELLO identity**
  (a connection cannot emit under another node's name);
- caps lines at **64 KiB** and rate-limits each connection (200 env/s);
- holds **no** state to disk — restart = full amnesia;
- forgets a peer the moment it disconnects;
- drops fragments for offline peers instead of buffering them.

> **Honest limitation (documented, not hidden) :** the relay sees the
> envelopes' metadata — `from`, `to`, `session`, sizes — because it must
> route on them. It *cannot* read the encrypted payloads, and since
> Phase 1 it no longer sees file names (encrypted out-of-band). The
> threat model is documented in `docs/threat-commodity.md` and
> `docs/threat-high-value.md`. Authenticity of the sender is enforced at
> the receiver (ML-DSA signature + trust anchor), not at the relay.

---

## 5. RES — the execution layer (exec.rs)

Two sandboxes for ghost-node compute:

1. **Shell** : `systemd-run --user` with `MemoryMax`, `CPUQuota`,
   `NoNewPrivileges`, `ProtectSystem=strict`, `PrivateTmp`,
   `PrivateNetwork`. Honest scope: *isolation against accidents, NOT a
   cryptographic sandbox* (same UID, `$HOME` readable).
2. **WASM** : `wasmi` + WASI, stdout captured, wall-clock timeout.
   Known limitation: wasmi is synchronous — a non-terminating module
   blocks the caller until timeout; an infinite loop without yield is
   detected only after `start()` returns.

The grant protocol is locked by tests (`grant_for()` pure + routing).

---

## 6. Identity (identity.rs) — `~/.polygone/identity.json`

First run generates and persists (chmod 600) :

- ML-KEM-1024 public + secret keys (hex)
- ML-DSA-65 public + secret keys (hex)
- a pseudonym

`polygone clef` prints the KEM public key — what you share to receive
messages. `polygone duress --confirmer` destroys identity + received
files (Axiome 5).

---

## 7. The daemon (polygoned)

A 5-second loop: `SystemSnapshot::capture` → `GlowUpEngine::tick` →
`apply` → publication of the live IPC state. On Unix, `polygoned` starts
`~/.polygone/daemon.sock` (unless `--dry-run`) and serves protocol v1
requests: `status`, `ping`, `alloc_request` and `release`. The socket is
owner-only (`0600`) and is removed after a graceful shutdown.

> **Honest status — 2026-11-02:** `apply()` only changes the daemon's
> `nice` priority. The client now exposes `polygone node servir/status`: it
> obtains a lease before opening the block-store port, and stops on daemon loss
> or a `shrinking` policy signal. That signal persists through the full RAM
> contraction; new leases are refused with `policy_shrinking` during that phase.
> The transfer ceiling is self-imposed. On Linux, `polygone node servir
> --isoler` can relaunch the consumer into a dedicated user-systemd cgroup v2
> scope and verifies `memory.max` + `cpu.max`; the scope is outside the desktop
> session cgroup. Without that explicit flag, no CPU/RAM OS quota is imposed.
> macOS has no equivalent wired yet. `user_active()` remains `Unknown` on
> Linux/macOS, not a verified idle state.
> Bandwidth/GPU values are measured/reported, not enforced.
> `allocator.rs` (the "Wozniak" allocator) is legacy code not used by the
> active `GlowUpEngine` policy.

---

## 8. Error handling

`PolygoneError` in `crates/core/src/error.rs` — `Crypto`, `Network`,
`Storage`, `InvalidArgument`, `NotFound`, `Internal`, `Io`, `Serde`.
The `?` operator is used where `From` exists; conversions at boundaries
are written by hand.

---

## 9. Testing

`cargo test --workspace` is the source of truth; do not rely on stale hard-coded
counts. The workspace had **431 passing tests** on 2026-11-05. Highlights: 35/35 Shamir combinations (C(7,4)), ML-DSA-65
exact sizes, wrong-key KEM failure, full network pipeline round-trips,
relay routing/drop/ignore tests.

---

## 10. Build

```bash
cargo build --release            # 4 binaires produit (fabric : --features lab)
cargo test --workspace           # 554 tests
cargo clippy --all --all-targets -- -D warnings
```

Release profile: `opt-level=3`, `lto="thin"`, `codegen-units=1`,
`strip=true`.

---

## 11. Known gaps (tracked, honest)

| Gap | Where | Status |
| --- | ----- | ------ |
| ~~ML-DSA-65 not signed/verified on the network path~~ | net.rs / sign.rs | ✅ **FIXED (Phase 1)** — every message is signed, verified, fail-closed |
| ~~Relay metadata in clear (from/to/tailles/name)~~ | relay.rs + net.rs | 🟡 Assumed + documented; **file name now out-of-band** (encrypted) |
| ~~Relay: no line limit, no rate limit, from spoofing~~ | relay.rs | ✅ **FIXED (Phase 1)** — 64 KiB cap, 200 env/s, `from` must equal HELLO, sharded table, **duplicate HELLO refusé + ack HELLO_OK/HELLO_DENIED** (anti-squatting déterministe, pas de vol silencieux de slot) |
| Relay: HELLO not cryptographically authenticated | relay.rs | 🟡 Assumed — authenticity lives at the receiver (ML-DSA + peers.json); a signed HELLO is a design option |
| ~~`known_peers` map never populated~~ | net.rs | ✅ **FIXED (Phase 4, contre-attaque)** — `~/.polygone/peers.json` chargé une fois, **TOFU appris puis ancré** (premier contact vérifié appris ; clé différente pour un `from` connu = rejet) |
| ~~combinations4 non borné (idx u8 non validé)~~ | net.rs | ✅ **FIXED (Phase 4, contre-attaque)** — idx borné 1..=7, ≤7 fragments, C(7,4)=35 max |
| ~~sessions sans plafond ni TTL~~ | net.rs | ✅ **FIXED (Phase 4, contre-attaque)** — MAX_SESSIONS=1024 fail-closed, purge TTL 300 s |
| ~~grant forgé accepté~~ | net.rs | ✅ **FIXED (Phase 4, contre-attaque)** — to/session/from vérifiés avant d'accepter un grant |
| ~~replay possible~~ | net.rs | ✅ **FIXED (Phase 4, contre-attaque)** — `ts` signé (canonical v2), fenêtre ±300 s, cache anti-rejeu |
| ~~confusion de session~~ | net.rs | ✅ **FIXED (Phase 4, contre-attaque)** — clé `from|session`, second KEM d'identité différente rejeté, fragments liés au `from` du KEM |
| ~~RES shell sandbox reads `$HOME` (same UID)~~ | exec.rs | ✅ **FIXED (Phase 2)** — `ProtectHome=yes`, `InaccessiblePaths=~/.polygone` (identité illisible), `PrivateDevices`, `RestrictAddressFamilies`, `SystemCallFilter=@system-service`, max 2 exécutions concurrentes |
| ~~WASM timeout after `start()` (sync wasmi)~~ | exec.rs | ✅ **FIXED (Phase 2)** — fuel metering wasmi (`Config::consume_fuel` + `set_fuel(1e10)`) : une boucle infinie trappe, le nœud ne gèle plus |
| ~~run_with_timeout laisse des unités transitoires orphelines~~ | exec.rs | ✅ **FIXED (Phase 2)** — unité nommée `polygone-res-<pid>-<nanos>`, `systemctl --user stop` au timeout |
| ~~Duress ne couvre que identity + received~~ | duress.rs | ✅ **FIXED (Phase 2)** — `reputation.json` détruit aussi |
| Zeroize : `#[zeroize(skip)]` ML-KEM + ML-DSA SecretKey sans zeroize | core | 🟡 Documenté honnêtement (pqcrypto n'expose pas de bytes mutables) ; l'effacement réel passe par `duress` |
| Daemon socket has no consumer | daemon/socket.rs | Decision pending (D5) |
| `time_sync` engine with no consumers | core/time_sync | Decision: wire or archive |
| At-rest: identity.json + received/ in clear | identity.rs / net.rs | Decision: encrypt-at-rest or document (Phase 2 — documenté, duress = effacement) |
| TOFU window : le premier contact d'un `from` inconnu est accepté puis appris | net.rs | 🟡 Assumé + documenté — l'empreinte du pair est affichée à l'écoute pour vérification hors-bande |
| WindowsPlatform non défini — build Windows = `compile_error!` explicite | daemon/resources | ✅ Honnête (2026-08-08) — jamais un lien cassé qui prétendrait marcher ; `windows.rs` = placeholder du futur portage ; promesse produit = Linux/macOS |

---

## 12. What is not in this document

- The exact `Cargo.toml` features — read it.
- The daemon's resource math — see `daemon/` source.
- The time_sync CBOR protocol — see `crates/core/src/time_sync/`.

If something is missing here, it is because the code is the source of
truth, not the docs. **Always trust the tests.**
