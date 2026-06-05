# Polygone

> *L'information n'existe pas.*

Post-quantum ephemeral network. Rust 2021, single binary, arrow-key menu.
A complete privacy operating system layer on top of your machine.

```
╔══════════════════════════════════════╗
║        ⬡  P O L Y G O N E          ║
║   Post-quantum ephemeral network    ║
║     L'information n'existe pas.     ║
╚══════════════════════════════════════╝
```

## One command

```bash
polygone
```

That's it. Arrow-key menu. No YAML to write, no daemon to start, no
config file to edit. Every other entry point (`polygone status`,
`polygone test`, `polygone serve`) is a shortcut for the same dashboard.

## The ecosystem

Polygone is **not a CLI**. It is a layer of services running on your
machine, coordinated by a single daemon. The CLI is just one of the
windows into that layer.

| Component       | Role                                                        | Binary                |
| --------------- | ----------------------------------------------------------- | --------------------- |
| `polygone`      | Arrow-key dashboard, command dispatcher, IPC client         | `polygone`            |
| `polygone-computer` | Local orchestrator daemon — owns all services          | `polygone-computer`   |
| `polygone-server`   | Stateless zero-knowledge relay — bridges NATs          | `polygone-server`     |
| `polygone-ctl`      | Scriptable IPC client (status / list / start / stop)   | `polygone-ctl`        |
| TUI              | Terminal UI (ratatui, cyber/slate palette)                  | inside `polygone`     |
| Web UI           | Four HTML pages (index, node, drive, mesh) embedded         | `polygone serve`      |

## The services

Every service is an implementation of the `Service` trait. They can be
started, stopped, observed, restarted, and replaced independently.

| Service   | Purpose                                                           | Where it lives       |
| --------- | ----------------------------------------------------------------- | -------------------- |
| `compute` | Lend / borrow local compute for distributed inference             | `src/compute/`       |
| `drive`   | Encrypted, sharded, distributed file storage                      | `src/network/drive/` |
| `hide`    | SOCKS5 / HTTPS proxy through the mesh                             | `src/services/hide.rs` |
| `mesh`    | mDNS / BLE discovery, local peer orchestration                    | `src/network/mesh/`  |
| `brain`   | Local LLM interface (Notch SLM, Ollama, llama.cpp)                | `src/services/brain.rs` |
| `msg`     | Ephemeral E2E messaging — no server, no logs                      | `src/services/msg.rs` |
| `petals`  | Distributed LLM inference — peers hold shards, run in parallel    | `src/services/petals.rs` |
| `shell`   | Secure shell over the mesh, peer-to-peer                          | `src/services/shell.rs` |

The **Computer** daemon owns the lifecycle of all of them. The
**Server** is a passive relay that knows nothing about plaintext.

## Cryptography

| Layer            | Algorithm                  | Purpose                                  |
| ---------------- | -------------------------- | ---------------------------------------- |
| Key exchange     | ML-KEM-1024                | Post-quantum session key                 |
| Signatures       | ML-DSA-87                  | Identity + non-repudiation               |
| Symmetric        | AES-256-GCM                | Payload + integrity                      |
| Hash             | BLAKE3                     | Fragment IDs, KDF                        |
| Secret sharing   | Shamir 4-of-7               | Drive: split across 7 peers, recover from 4 |
| Hashing          | Argon2id                   | Local secret at rest                     |

The Server never sees plaintext. The Server never sees keys. The
Server only forwards opaque encrypted fragments between nodes that
cannot reach each other directly.

## Quickstart

```bash
# 1. build
cargo build --release

# 2. run the dashboard
./target/release/polygone

# 3. or boot the daemon
./target/release/polygone-computer

# 4. or expose the web UI
./target/release/polygone serve --bind 127.0.0.1:8080
# → open http://127.0.0.1:8080

# 5. or run a relay
POLYGONE_BIND=0.0.0.0:4001 ./target/release/polygone-server
```

## Tests

```bash
cargo test --lib
# 43 passed; 0 failed
```

## Documentation

- [`ECOSYSTEM.md`](./ECOSYSTEM.md) — the **mother file**. The service
  registry, use cases, the data flow between Computer / Server /
  services, and the contract every new service must respect.
- [`ARCHITECTURE.md`](./ARCHITECTURE.md) — the technical architecture.
  Crate layout, module boundaries, IPC protocol, lifecycle, threading
  model, build matrix.

## License

TBD. Until then: don't be evil.
