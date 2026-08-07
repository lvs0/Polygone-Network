# ARCHITECTURE.md — how it's built

> *For contributors. Read [`ECOSYSTEM.md`](./ECOSYSTEM.md) first to
> understand what we are building. This document is about how.*
>
> **Status 2026-08-07** : this document describes the **real** v2 workspace
> (4 crates). The previous `src/` monolith (libp2p, HTTP relay, 4-tab TUI)
> was archived to `archive/2026-07-src/` — it is not built, not tested, not
> the product. The code is the source of truth; this document follows it.

---

## 1. Workspace layout — 4 crates

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
│   │   ├── net.rs                real network transport (plane 2)
│   │   ├── identity.rs           ~/.polygone/identity.json (chmod 600)
│   │   ├── exec.rs               RES execution (systemd-run + wasmi)
│   │   ├── mesh.rs               LAN discovery (UDP broadcast, port 7642)
│   │   ├── reputation.rs         ghost-node trust ledger
│   │   ├── petals.rs             local AI (Ollama HTTP, no cloud)
│   │   ├── duress.rs             kill-switch (Axiome 5)
│   │   ├── demo.rs               flagship E2E demo (in-process)
│   │   ├── self_test.rs          crypto self-test suite
│   │   └── tui.rs                vim-style command TUI (:envoyer / :quitter)
│   │
│   ├── relay/                    the blind relay — one binary: `polygone-relay`
│   │   ├── main.rs               CLI (port, bind)
│   │   └── relay.rs              async TCP server, in-memory routing table
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

**Reality check (2026-08-07) :**

| Crate      | LOC (rust) | Binaries              | Tests |
| ---------- | ---------- | --------------------- | ----- |
| core       | ~2 200     | — (lib)               | 34    |
| client     | ~3 600     | `polygone`            | 30    |
| relay      | ~340       | `polygone-relay`      | 4     |
| daemon     | ~3 500     | `polygoned`           | 21    |
| **Total**  | **~9 600** | 4 binaries            | **89** |

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
`apply`. It measures CPU, RAM, GPU, bandwidth and computes an
allocation; `apply()` re-nices the daemon and writes `memory.max` into
its own cgroup. A command socket (`~/.polygone/daemon.sock`) accepts
`set_alloc / shrink / grow / status`.

> **Honest status :** no process currently reads the daemon socket, and
> `user_active()` returns `false` on Linux — the "throttle on user
> activity" is not wired. Bandwidth/GPU numbers are *reported, not
> enforced*. `allocator.rs` (the "Wozniak" allocator) is legacy code not
> used by the loop — the policy engine is `GlowUpEngine`.

---

## 8. Error handling

`PolygoneError` in `crates/core/src/error.rs` — `Crypto`, `Network`,
`Storage`, `InvalidArgument`, `NotFound`, `Internal`, `Io`, `Serde`.
The `?` operator is used where `From` exists; conversions at boundaries
are written by hand.

---

## 9. Testing

`cargo test --workspace` — 89 tests (client 30, core 34, relay 4,
daemon 21). Highlights: 35/35 Shamir combinations (C(7,4)), ML-DSA-65
exact sizes, wrong-key KEM failure, full network pipeline round-trips,
relay routing/drop/ignore tests.

---

## 10. Build

```bash
cargo build --release            # all 4 binaries
cargo test --workspace           # 89 tests
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

---

## 12. What is not in this document

- The exact `Cargo.toml` features — read it.
- The daemon's resource math — see `daemon/` source.
- The time_sync CBOR protocol — see `crates/core/src/time_sync/`.

If something is missing here, it is because the code is the source of
truth, not the docs. **Always trust the tests.**
