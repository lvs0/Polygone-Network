# ⬡ Polygone

> *L'information n'existe pas. Elle traverse.*

**Post-quantum ephemeral transit network.**
ML-KEM-1024 · AES-256-GCM · Shamir 4-of-7 · BLAKE3.
No server sees the message. No observer can prove a message existed.

```
╔══════════════════════════════════════════════════════════════╗
║                                                              ║
║          ⬡  P O L Y G O N E                                ║
║                                                              ║
║   L'information n'existe pas.                                ║
║   Elle traverse.                                             ║
║                                                              ║
╚══════════════════════════════════════════════════════════════╝
```

**Status:** v1.0.0 · MIT License · No investors · No token · No telemetry
**Build:** ![CI](https://github.com/lvs0/Polygone-Network/actions/workflows/ci.yml/badge.svg)
**Source:** `https://github.com/lvs0/Polygone-Network`

---

## What it is

Polygone is **infrastructure sovereignty** — not a chat app, not a VPN, not a blockchain.

It is a P2P network where messages flow through nodes without residing in any node.
Every message is encrypted post-quantum (ML-KEM-1024), fragmented into 7 shares
(Shamir 4-of-7), and only 4 are needed to reconstruct. The relay knows nothing —
not the content, not the sender, not the receiver. Observers cannot prove a message
existed.

```
┌─────────────────────────────────────────────────────────────┐
│                        POLYGONE                             │
│                                                             │
│   Alice ──► [ML-KEM-1024] ──► AES-256-GCM ──► Fragment 7   │
│              Key exchange     Encryption       Shamir SSS  │
│                         │                                   │
│                    Relay ──► [Fragment 1] ──► Bob          │
│                    (sees nothing)         ──► [Fragment 2]  │
│                                            ──► [Fragment 3] │
│                                            ──► ...          │
│                                                             │
│   4 of 7 fragments → reconstruct the original message      │
│   Relay: zero knowledge. Observers: zero proof.           │
└─────────────────────────────────────────────────────────────┘
```

## What it is NOT

| Not this | Because |
|---|---|
| Blockchain | No consensus, no mining, no token |
| VPN | No centralized tunnel, no trusted provider |
| Signal | Not just encryption — it's transit infrastructure |
| "Crypto" in the speculative sense | No ICO, no investors, no token |

---

## Architecture

```
polygone (CLI)
├── crypto/         ML-KEM-1024, ML-DSA-87, AES-256-GCM, Shamir 4/7, BLAKE3
├── network/        P2pNode + PolygoneBehaviour (libp2p 0.53)
│                    Kad · GossipSub · Req-Resp · mDNS · Relay · AutoNAT · DCUtR · Ping
├── protocol/       TransitState FSM: Created → FragDistributed → PartialReception → Reassembling → Delivered → Expired
├── services/       Service trait (Phase · Health · Metrics)
├── tui/             ratatui v2: Dashboard · Favorites · Services · Composer · Settings
├── web/             serve() + serve_live() HTTP server · SSE events · static dashboard
├── ipc/             Unix socket client/daemon
├── computer/        Orchestrator: Plan + EventBus + SSE (the brain)
└── compute/         IdleDetector · compute lending · Petals protocol
```

---

## Quickstart

```bash
# Build (requires Rust 1.75+)
cargo build --release

# Interactive dashboard (TUI)
./target/release/polygone

# Web dashboard
./target/release/polygone serve --bind 127.0.0.1:8080
# → open http://127.0.0.1:8080

# Non-interactive status
./target/release/polygone status

# Run self-test suite
./target/release/polygone test

# Send a message (Alice → Bob)
./target/release/polygone send "Hello world" --generate
./target/release/polygone send "Hello world" --key <bob-public-key-hex>

# Receive a message (Bob)
./target/release/polygone receive fragments.bin --key <bob-secret-key-hex>

# IPC client
./target/release/polygone-ctl status
```

---

## Cryptographic Stack

| Layer | Algorithm | Standard |
|---|---|---|
| Key exchange | **ML-KEM-1024** (Kyber) | NIST FIPS 203 |
| Signatures | **ML-DSA-87** (Dilithium) | NIST FIPS 204 |
| Encryption | **AES-256-GCM** | NIST SP 800-38D |
| Hash / KDF | **BLAKE3** | - |
| Secret sharing | **Shamir 4-of-7** | - |

The relay **never sees plaintext**. It **never sees keys**. It only forwards
opaque encrypted fragments.

---

## Features

- **TUI Dashboard** — 5 tabs with sparklines, gauges, real-time metrics
- **Web Dashboard** — `/api/status`, `/api/peers`, topology visualization
- **Compute Lending** — IdleDetector finds idle GPU/CPU cycles
- **Petals Protocol** — Distributed LLM inference across peer shards
- **Drive** — Encrypted, sharded, distributed file storage
- **Hide** — SOCKS5 proxy through the mesh (no logs, no exit node tracking)
- **Ephemeral Messaging** — No server, no logs, TTL-based expiry
- **Post-Quantum Everything** — Quantum computers can't break ML-KEM-1024

---

## Documentation

- [`ARCHITECTURE.md`](./ARCHITECTURE.md) — Technical architecture, module layout, threading model
- [`ECOSYSTEM.md`](./ECOSYSTEM.md) — Service registry, data flows, contracts
- [`POLYGONE-SPEC-1.0.0.txt`](./POLYGONE-SPEC-1.0.0.txt) — Full specification
- [`POLYGONE-SPEC-AUDIT.md`](./POLYGONE-SPEC-AUDIT.md) — Security audit
- [`IMPROVEMENT_PLAN.md`](./IMPROVEMENT_PLAN.md) — Roadmap to v2.0

---

## License

MIT — Free as in freedom. No investors. No token. No telemetry.
Built by a 14-year-old who refuses to be tracked.