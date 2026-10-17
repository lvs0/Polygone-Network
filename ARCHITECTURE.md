# ARCHITECTURE.md — how it's built

> *For contributors. Read [`ECOSYSTEM.md`](./ECOSYSTEM.md) first to
> understand what we are building. This document is about how.*
>
> **Status 2026-10-16** : this document describes the current Cargo workspace
> (6 packages: 4 executable targets and 2 libraries). The previous `src/` monolith (libp2p, HTTP relay, 4-tab TUI)
> was archived to `archive/2026-07-src/` — it is not built, not tested, not
> the product. The code is the source of truth; this document follows it.

---

## 1. Workspace layout — 6 Cargo packages

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
│   ├── gateway/                  HTTP API + static site + pairing binary
│   ├── petals/                   local AI inference library and backends
│   └── relay/                    the blind relay — one binary: `polygone-relay`
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

**Snapshot historique (2026-08-07) — compteurs non actuels :**

| Crate      | LOC (rust) | Binaries              | Tests |
| ---------- | ---------- | --------------------- | ----- |
| core       | ~2 200     | — (lib)               | 34    |
| client     | ~3 600     | `polygone`            | 47    |
| relay      | ~340       | `polygone-relay`      | 7     |
| daemon     | ~3 500     | `polygoned`           | 21    |
| **Total**  | **~9 600** | 4 binaries            | **109** |

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
`Pair --qr` currently constructs an internal `polygone-pair` JSON payload
with this alias and the KEM public key, not the ML-DSA key or a proof binding
both. No QR is rendered and no consumer of this JSON was found. The payload
also double-hex-encodes the 16-character network `node_id` into 32 characters;
this is a confirmed internal-output defect, not evidence that a live user
pairing flow is broken. The alias is for routing, not a trust anchor.
The two complete public keys total 3,520 raw bytes, exceeding the 2,953-byte
maximum of a standard Version 40-L QR before any metadata; a single-QR v2
bundle must not be promised. Two compact QR payloads or a multi-QR transfer
remain proposals, not decisions. Display the full fingerprint separately for
out-of-band comparison; a hash inside the same QR is not independent proof.
Validate final capacity with the selected encoder and error-correction level.
Registration is **acknowledged**: a duplicate `HELLO` for a live node_id
is refused *and the claimant is told* — no last-writer-wins, no silent
slot theft. The client waits for the ack before sending a single byte.

### Signatures — "c'est bien Alice"

Every message is signed by the sender's ML-DSA-65 key. The signature
covers the canonical bytes `session ‖ from ‖ to ‖ kem_ct ‖ ciphertext`
(Shamir deterministically rebuilds the same ciphertext from any 4-of-7
fragments, so the receiver verifies before decrypting). The receiver
fails closed on: missing signature, unverifiable signature, envelope not
addressed to it, duplicate fragment index. `known_peers` (`from` → expected
ML-DSA pk) anchors TOFU. Tilleul reviewed the corrected P1/P2 rollback and
orphan-backup paths on the 24-path validation snapshot SHA-256
`33626ec120864167a107ceaa66164317e2d6cc9577cd792a671b74389c5bd0bd`; no new
defect was found. Polygone reports workspace 206/206, strict Clippy, fmt,
release build, CLI smoke and forensic loopback passing; Tilleul independently
confirmed Client 134/134, fmt and diff-check. The Windows-only
`peers_store_first_launch_creates_missing_parent` test is present but was not
compiled or executed; MinGW failed in `pqcrypto-mlkem` on `features.h`. Keep the
Windows portability gate open and make no universal crash-durability claim.

### File names are out-of-band

The relay never sees a file name: `name_ct` is the name encrypted with
the session key, which only the recipient can derive. The relay sees
opaque bytes.

### Send

`envoyer --via <relay:port> --a <dest_node_id> <message>` :

1. `msg::send_bytes` → KEM_CT + 7 fragments (as in §2).
2. 1 KEM envelope + 7 fragment envelopes written to the relay.
3. The relay forwards each to `to` if connected; otherwise drops.

For `envoyer --fichier`, Drive splits the file into 12 KiB plaintext blocks.
Each block carries an encrypted transfer id, sequence, total and filename,
and is sent in its own signed KEM session (one KEM envelope + 7 Shamir
fragments). The listener authenticates/decrypts each session, buffers blocks
by sender and transfer id, restores sequence order, and writes the destination
file only when the complete transfer is present. Duplicate, missing, or
inconsistent blocks cannot produce a partial output file. Pending network
file data is capped at 256 MiB and stale incomplete transfers expire after
the session TTL.

Library callers with in-memory data can use `DriveFile::prepare_send_iter`; it
validates the 256 MiB limit before yielding anything and then produces one
`SendOutput` at a time, in stable block order. For a file path,
`DriveFile::send_iter` takes the initial file size as its contract, validates
the 256 MiB limit before iteration, and reads/encrypts one block per step. It
fails on premature EOF, a byte beyond the initial length, or a read/encryption
error; it also rechecks open-file length and modification time. An error ends
iteration, and the caller must abort any transfer even if earlier outputs were
already yielded. This is not an immutable filesystem snapshot: an in-place
same-size rewrite that is not reflected by the filesystem timestamp may not be
detected. For either iterator, the memory benefit applies only if the caller
processes and drops each output before requesting the next; collecting the
iterator retains batch-like memory. `DriveFile::send` and
`DriveFile::prepare_send` remain available for compatibility, but both collect
all encrypted outputs in memory. At the 256 MiB cap, the static peak estimate
for a batch caller is approximately 2.25–2.30 GiB; this risk remains open until
those callers migrate to streaming or are separately bounded. This is not an
RSS measurement. The active network CLI uses its own bounded block-by-block
path and is unchanged and separate from the library APIs.

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
2. **WASM** : `wasmi` + WASI, fuel metering (`100_000_000` units), strict
   compiler limits and stdout/stderr caps. Fuel is the primary bound against
   non-terminating loops; the wall-clock check runs after the synchronous
   call returns, so it is not an asynchronous interrupt.

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

Validation-set SHA-256 (24 paths) `33626ec120864167a107ceaa66164317e2d6cc9577cd792a671b74389c5bd0bd`
was reported with 206/206 workspace tests, strict Clippy, fmt, release build,
CLI smoke and forensic loopback passing; Tilleul independently confirmed Client
134/134, fmt, diff-check and no new P1/P2 defect. The Windows-only first-bootstrap
test is present but was not compiled or executed, so native Windows behavior
remains unverified. This digest predates the documentation reconciliation and
does not represent the current full worktree. No Modal exchange between environments
or physical-host test has run; see `RELEASE-STATUS.md`. Highlights: 35/35 Shamir
combinations (C(7,4)), ML-DSA-65 exact sizes, wrong-key KEM failure, full
network pipeline round-trips, relay routing/drop/ignore tests.

---

## 10. Build

```bash
cargo build --release            # all 4 binaries
    cargo test --workspace           # historical pre-TOFU baseline: 176 + 1; see RELEASE-STATUS.md for latest reported pass
cargo clippy --all --all-targets -- -D warnings
```

Release profile: `opt-level=3`, `lto="thin"`, `codegen-units=1`,
`strip=true`.

---

## 11. Known gaps (tracked, honest)

| Gap | Where | Status |
| --- | ----- | ------ |
| ~~ML-DSA-65 not signed/verified on the network path~~ | net.rs / sign.rs | ✅ **FIXED (Phase 1)** — every message is signed, verified, fail-closed |
| Relay sees routing metadata and Compute JSON bodies in clear | relay.rs + net.rs | 🟡 Message/file plaintext and names are E2E-encrypted; Compute request/grant JSON is signed but not encrypted, so the relay and on-path observers of the TCP path (no TLS) can read task/WASM payload and response |
| ~~Relay: no line limit, no rate limit, from spoofing~~ | relay.rs | ✅ **FIXED (Phase 1)** — 64 KiB cap, 200 env/s, `from` must equal HELLO, sharded table, **duplicate HELLO refusé + ack HELLO_OK/HELLO_DENIED** (anti-squatting déterministe, pas de vol silencieux de slot) |
| Relay: HELLO not cryptographically authenticated | relay.rs | 🟡 Assumed — authenticity lives at the receiver (ML-DSA + peers.json); a signed HELLO is a design option |
| ~~`known_peers` map never populated~~ | net.rs | ✅ **FIXED (Phase 4, contre-attaque)** — `~/.polygone/peers.json` permet d’apprendre un pin et de rejeter une clé différente pour un `from` connu, à condition que chargement et sauvegarde réussissent; voir le gap fail-open ci-dessous. |
| TOFU store portability | net.rs | 🔴 **GATE OPEN** — Tilleul found no remaining P1/P2 defect in the corrected `replace` rollback or orphan-backup handling on SHA `33626ec…`. The Windows-only `peers_store_first_launch_creates_missing_parent` test is present but not compiled/executed; MinGW fails in `pqcrypto-mlkem` on `features.h`. Require native Windows execution before claiming Windows portability. No universal crash-durability claim. |
| ~~combinations4 non borné (idx u8 non validé)~~ | net.rs | ✅ **FIXED (Phase 4, contre-attaque)** — idx borné 1..=7, ≤7 fragments, C(7,4)=35 max |
| ~~sessions sans plafond ni TTL~~ | net.rs | ✅ **FIXED (Phase 4, contre-attaque)** — MAX_SESSIONS=1024 fail-closed, TTL d’inactivité 300 s |
| Session KEM pré-authentification incomplète | net.rs | 🟡 Un KEM syntaxiquement valide peut réserver un slot avant vérification de signature. Chaque fragment distinct renouvelle le TTL; un timestamp futur accepté peut laisser les fragments recevables pendant ~600 s après le KEM, puis le slot reste jusqu’à 300 s d’inactivité et la purge périodique peut ajouter jusqu’à 30 s (environ 15 min 30 s au total). Aucun délai absolu non renouvelable; voir `RELEASE-STATUS.md`. |
| ~~grant forgé accepté~~ | net.rs | ✅ **FIXED (Phase 4, contre-attaque)** — to/session/from vérifiés avant d'accepter un grant |
| ~~replay possible~~ | net.rs | ✅ **FIXED (Phase 4, contre-attaque)** — `ts` signé (canonical v2), fenêtre ±300 s, cache anti-rejeu |
| ~~confusion de session~~ | net.rs | ✅ **FIXED (Phase 4, contre-attaque)** — clé `from|session`, second KEM d'identité différente rejeté, fragments liés au `from` du KEM |
| ~~RES shell sandbox reads `$HOME` (same UID)~~ | exec.rs | ✅ **FIXED (Phase 2)** — `ProtectHome=yes`, `InaccessiblePaths=~/.polygone` (identité illisible), `PrivateDevices`, `RestrictAddressFamilies`, `SystemCallFilter=@system-service`, max 2 exécutions concurrentes |
| ~~WASM timeout after `start()` (sync wasmi)~~ | exec.rs | ✅ **MITIGATED (Phase 2)** — fuel metering (`Config::consume_fuel` + `set_fuel(100_000_000)`) traps an infinite loop; the wall-clock limit is still checked only after the synchronous call |
| ~~run_with_timeout laisse des unités transitoires orphelines~~ | exec.rs | ✅ **FIXED (Phase 2)** — unité nommée `polygone-res-<pid>-<nanos>`, `systemctl --user stop` au timeout |
| ~~Duress ne couvre que identity + received~~ | duress.rs | ✅ **FIXED (Phase 2)** — `reputation.json` détruit aussi |
| Zeroize : `#[zeroize(skip)]` ML-KEM + ML-DSA SecretKey sans zeroize | core | 🟡 Documenté honnêtement (pqcrypto n'expose pas de bytes mutables) ; l'effacement réel passe par `duress` |
| Daemon socket has no consumer | daemon/socket.rs | Decision pending (D5) |
| `time_sync` engine with no consumers | core/time_sync | Decision: wire or archive |
| Compute grant request/response validation | net.rs | ✅ Local fix builds final payload before one signature; client suite 105/105 and workspace 176 + 1 pass; independent review found no defect. E2E body confidentiality and physical inter-host validation remain open |
| At-rest: identity.json + received/ in clear | identity.rs / net.rs | Decision: encrypt-at-rest or document (Phase 2 — documenté, duress = effacement) |
| TOFU window : le premier contact d'un `from` inconnu est accepté puis appris | net.rs | 🟡 TOFU reste automatique au premier contact. Le CLI n’offre pas de procédure supportée pour vérifier l’empreinte canonique du bundle KEM+ML-DSA; le guide ne compare que ML-DSA et ne suffit pas à autoriser du contenu sensible. |
| WindowsPlatform non défini — build Windows = `compile_error!` explicite | daemon/resources | ✅ Honnête (2026-08-08) — jamais un lien cassé qui prétendrait marcher ; `windows.rs` = placeholder du futur portage ; promesse produit = Linux/macOS |

---

## 12. What is not in this document

- The exact `Cargo.toml` features — read it.
- The daemon's resource math — see `daemon/` source.
- The time_sync CBOR protocol — see `crates/core/src/time_sync/`.

If something is missing here, it is because the code is the source of
truth, not the docs. **Always trust the tests.**
