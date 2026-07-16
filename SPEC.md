# Polygone v2 — Rewrite Specification

## Context

The original Polygone (v1, 28 repos, ~3,800 lines of Rust across 9 crates) has a proven cryptographic foundation:
- ML-KEM-1024 (Kyber successor)
- Shamir 4-of-7 secret sharing
- AES-256-GCM + BLAKE3

What it lacks: a working, demoable, end-to-end system. The rewrite (v2) is not a redesign — it's a **reconstruction** of the same architecture with one goal: prove it works.

## Tagline

> "On voit rien. Et c'est comme ça que ça devrait être."

This is not marketing. It's a **technical property**: the relay sees no content, no metadata correlated to sender/receiver, no proof of communication.

## Architecture (v2)

```
User A                        User B
  │                              │
  ▼                              ▼
┌──────────────────────┐  ┌──────────────────────┐
│  Polygone Client      │  │  Polygone Client      │
│  • ML-KEM-1024 key    │  │  • ML-KEM-1024 key   │
│  • Shamir fragment   │  │  • Shamir fragment   │
│  • DHT peer discovery │  │  • DHT peer discovery│
└──────────┬───────────┘  └──────────┬───────────┘
           │                         │
           └─────────┬───────────────┘
                     │ P2P (libp2p / Kademlia DHT)
                     │ No server sees the content
                     ▼
           ┌─────────────────────┐
           │  Polygone Relay     │
           │  (stateless, blind) │
           │  "On voit rien."    │
           └─────────────────────┘
```

## What's Different from v1

| Aspect | v1 | v2 |
|---|---|---|
| Crypto | ✅ ML-KEM + Shamir (proven) | ✅ Same, but tested |
| P2P | libp2p (draft) | libp2p (stable, minimal) |
| Demo | Doesn't exist | One command: `cargo run --example send` |
| Relay | Module exists | Fully implemented, stateless, auditable |
| Daemon | None | `polygoned` (resource allocation) |
| Config | Dispersed across crates | Single `polygone.toml` |
| README | Promises | Proof of work |

## v2 Crate Structure

```
polygone/               — Workspace root (Cargo workspace)
├── daemon/             — Resource allocation daemon (this spec)
│   ├── system.rs       — sysinfo wrapper
│   ├── allocator.rs    — Allocation engine
│   ├── socket.rs       — Unix socket to node
│   └── main.rs         — Entry point
│
├── core/               — Crypto + protocol core
│   ├── src/crypto/    — ML-KEM-1024, AES-256-GCM, BLAKE3
│   ├── src/shamir/    — Shamir 4-of-7
│   ├── src/packet.rs   — Packet format
│   └── src/lib.rs
│
├── relay/             — The relay node (stateless)
│   └── src/main.rs
│
├── client/            — The user client (libp2p node)
│   ├── src/node.rs    — P2P node setup
│   ├── src/dispatch.rs — Message routing
│   └── src/cli.rs      — Simple TUI or CLI
│
├── examples/           — One-file demos
│   ├── 01_send.rs     — Alice → Bob, one hop
│   ├── 02_shamir.rs   — Demonstrate secret sharing
│   └── 03_relay.rs    — Relay sees nothing
│
└── tests/
    ├── integration.rs  — Full E2E test
    └── chaos.rs        — Relay audit test (does it really see nothing?)
```

## The Relay: What It Sees vs What It Doesn't

### What the relay sees
```
[connected] → [encrypted_packet_received] → [forwarded_to_peer] → [disconnected]
```

### What the relay NEVER sees
- Sender identity (packet is ciphertext)
- Receiver identity (relay doesn't resolve addresses)
- Content (end-to-end encrypted)
- Metadata that proves a conversation happened

### Audit test (chaos.rs)
```rust
// Relay logs EVERYTHING it processes.
// chaos.rs runs a relay, sends packets through it,
// then verifies relay logs contain ZERO plaintext content.
// If any content is found → test FAILS.
```

## The Daemon (polygoned)

Already implemented — see `daemon/SPEC.md`.

## How to Demo in <5 Minutes

```bash
# 1. Clone
git clone https://github.com/lvs0/Polygone-Network
cd Polygone-Network

# 2. Build
cargo build --release

# 3. Terminal 1 — start relay
cargo run --release -p polygone-relay

# 4. Terminal 2 — start node A
cargo run --release -p polygone-client -- --mode send --to bob --file secret.txt

# 5. Terminal 3 — start node B (receives)
cargo run --release -p polygone-client -- --mode recv

# Result: Bob receives secret.txt. Relay logs show only: [received] [forwarded].
```

## Non-Goals (What v2 is NOT)

- Not a VPN replacement (different model)
- Not a blockchain (no token, no ledger)
- Not "private by policy" — private by **construction**
- Not Signal/Matrix/Wire (those are servers that see metadata)

## Governance

- AGPL-3.0 — no proprietary forks
- No token, no ICO, no equity crowdfunding
- Structure: foundation or cooperative (TBD, not a blocker)
- Funded by: grants, donations, NLnet, Prototype Fund

## References

- ML-KEM: https://pq-crystals.org/kyber/ (NIST Level 5)
- libp2p: https://github.com/libp2p/rust-libp2p
- Shamir: same math as Secret Network / Threshold ECDSA

---

*Last updated: 2026-07-16 | v2 rewrite — build one thing, prove it works*