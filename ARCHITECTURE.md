# ARCHITECTURE.md — how it's built

> *For contributors. Read [`ECOSYSTEM.md`](./ECOSYSTEM.md) first to
> understand what we are building. This document is about how.*

---

## 1. Crate layout

```
polygone/
├── Cargo.toml                    edition 2021, rustc 1.75
├── README.md                     manifesto + quickstart
├── ECOSYSTEM.md                  the mother file
├── ARCHITECTURE.md               this file
│
├── src/
│   ├── lib.rs                    re-exports, 9 module declarations
│   ├── main.rs                   CLI dispatcher (clap)
│   │
│   ├── crypto/                   pure functions, no I/O
│   │   ├── kem.rs                ML-KEM-1024 (post-quantum KEM)
│   │   ├── shamir.rs             4-of-7 secret sharing
│   │   ├── sign.rs               ML-DSA-87 signatures
│   │   ├── symmetric.rs          AES-256-GCM
│   │   ├── error.rs              PolygoneError enum
│   │   └── karma.rs              account balance (poly)
│   │
│   ├── network/                  libp2p swarm + custom protocols
│   │   ├── p2p.rs                libp2p behaviour (884 lines, 5+ tests)
│   │   ├── drive.rs              distributed storage (Shamir + AES)
│   │   ├── mesh.rs               local discovery
│   │   ├── discovery.rs          mDNS / BLE
│   │   ├── topology.rs           peer table, RTT, capability ads
│   │   └── node.rs               local node identity (ML-DSA-87)
│   │
│   ├── protocol/                 application layer
│   │   └── session.rs            KEM → sign → encrypt handshake
│   │
│   ├── compute/                  lend/borrow compute service
│   │   └── daemon.rs             PoW, heartbeat, accounting
│   │
│   ├── services/                 the Service trait
│   │   └── mod.rs                trait + Phase + Health + Metric
│   │
│   ├── computer/                 the local orchestrator
│   │   └── mod.rs                Computer daemon, watchdog loop
│   │
│   ├── server/                   the stateless relay
│   │   └── mod.rs                in-memory fragment store + HTTP
│   │
│   ├── ipc/                      line-JSON over Unix socket
│   │   └── mod.rs                Request/Response, dispatch
│   │
│   ├── tui/                      ratatui dashboard
│   │   ├── app.rs                4 tabs, key bindings, state
│   │   └── views.rs              Dashboard, Favoris, Services, Settings
│   │
│   ├── web/                      minimalist HTTP server
│   │   ├── mod.rs                tokio TcpListener, 4 routes + static
│   │   └── assets.rs             compile-time embed of web/*
│   │
│   ├── config/                   (planned) TOML config
│   ├── cli/                      (planned) shared CLI helpers
│   │
│   └── bin/
│       ├── polygone.rs              CLI dashboard
│       ├── polygone-computer.rs     orchestrator daemon
│       ├── polygone-server.rs       relay
│       └── polygone-ctl.rs          scriptable IPC client
│
├── web/                          1990 lines, 4 HTML pages
│   ├── index.html                landing
│   ├── node.html                 node dashboard
│   ├── drive.html                drive interface
│   ├── mesh.html                 mesh visualization
│   └── style.css, app.js, *.css, *.js
│
├── docs/                         (planned) install scripts, more
├── docker/                       (planned) Dockerfile
└── target/                       build artifacts
```

---

## 2. The 9 modules and their public API

| Module        | Public exports                                            | LOC    |
| ------------- | --------------------------------------------------------- | ------ |
| `crypto`      | `kem`, `shamir`, `sign`, `symmetric`, `error`, `karma`   | ~1200  |
| `network`     | `p2p`, `drive`, `mesh`, `discovery`, `topology`, `node`   | ~1400  |
| `protocol`    | `session`                                                 | ~200   |
| `compute`     | `daemon`                                                  | ~340   |
| `services`    | `Service`, `Phase`, `Health`, `Metric`, `ServiceInfo`     | ~250   |
| `computer`    | `Computer`                                                | ~220   |
| `server`      | `serve`, `RelayStore`, `RelayStats`                       | ~280   |
| `ipc`         | `Request`, `Response`, `Op`, `bind`, `handle`, `call`     | ~250   |
| `tui`         | `App`, `View`, `run_dashboard`                            | ~700   |
| `web`         | `serve`, `WebConfig`, `NodeState`                         | ~280   |
| `bin`         | 4 binaries                                                | ~300   |

Total Rust: **~5500 lines** in `src/`.

---

## 3. Data flow — what happens when you press Enter

```
       ┌──────────────────────────────────────────────────────┐
       │                       USER                            │
       └───────────────────────┬──────────────────────────────┘
                               │ arrow keys + Enter
                               ▼
                ┌──────────────────────────┐
                │   polygone (TUI)         │
                │   ratatui + crossterm    │
                │   4 tabs, 250ms redraw   │
                └────────────┬─────────────┘
                             │ connect /tmp/polygone-computer.sock
                             │ write JSON line
                             ▼
                ┌──────────────────────────┐
                │  polygone-computer       │
                │  Computer::dispatch      │
                │  ─────────────────       │
                │  start_all / stop_all    │
                │  / per-service start_one │
                └────────────┬─────────────┘
                             │
                ┌────────────┴──────────────┐
                │                           │
                ▼                           ▼
       ┌──────────────────┐       ┌──────────────────┐
       │  drive service   │       │  hide service    │
       │  (Arc<dyn Svc>)  │       │  (Arc<dyn Svc>)  │
       └────────┬─────────┘       └────────┬─────────┘
                │ libp2p swarm              │ SOCKS5 listener
                ▼                           ▼
       ┌──────────────────┐       ┌──────────────────┐
       │  peer: bob       │       │  firefox:9050    │
       │  direct or via   │       │  → internet      │
       │  polygone-server │       │  anonymized      │
       └──────────────────┘       └──────────────────┘
```

---

## 4. Threading model

The project uses **tokio multi-thread** for binaries that need it
(`polygone-server`, `polygone-computer`, `polygone`), and
`current_thread` for tiny clients (`polygone-ctl`).

| Binary                 | Runtime    | Why                                            |
| ---------------------- | ---------- | ---------------------------------------------- |
| `polygone`             | multi      | TUI needs a separate task for stdin/stdout     |
| `polygone-computer`    | multi      | one task per service + watchdog + IPC accept   |
| `polygone-server`      | multi      | one task per TCP connection                     |
| `polygone-ctl`         | current    | one request, one response                      |

The Computer spawns:
- 1 task for the 1 Hz watchdog loop
- 1 task per running service (each is a long-lived `tokio::spawn`)
- 1 task for the IPC Unix listener
- 1 task for the status-file writer

Total: **2 + N** tasks where N is the number of services.

---

## 5. Error handling

A single error type: `PolygoneError` (in `src/crypto/error.rs`).

```rust
pub enum PolygoneError {
    Crypto(String),
    Network(String),
    Storage(String),
    InvalidArgument(String),
    NotFound(String),
    Internal(String),
    Io(std::io::Error),
    Serde(serde_json::Error),
}
```

There is **no** `From<std::net::TcpListener>` etc. — conversions are
written by hand at the boundary. The `?` operator is only used for
`std::io::Error` and `serde_json::Error` (both have `From`).

If you find yourself adding `From<X> for PolygoneError`, ask first:
*do we really want every `X` to silently become a `PolygoneError`?*

---

## 6. The Service trait — full definition

```rust
#[async_trait]
pub trait Service: Send + Sync {
    fn info(&self) -> ServiceInfo;
    async fn start(&self) -> PolyResult<()>;
    async fn stop(&self)  -> PolyResult<()>;
    async fn phase(&self) -> Phase;
    async fn health(&self) -> Health;
    async fn metrics(&self) -> Vec<Metric>;
}
```

### ServiceInfo

```rust
pub struct ServiceInfo {
    pub id:          &'static str,   // "drive"
    pub name:        &'static str,   // "Polygone Drive"
    pub tagline:     &'static str,   // one-line description
    pub category:    &'static str,   // "storage", "network", "ai", "messaging"
    pub version:     &'static str,   // semver of the service itself
    pub default:     bool,           // should Computer::boot start it?
}
```

### Phase

```
Stopped ──start()──▶ Starting ──ok──▶ Running ──stop()──▶ Stopping ──▶ Stopped
                          │                                  │
                          └─ error ──▶ Crashed ──start()──▶ Starting …
```

### Health

```
Ok < Degraded < Failing < Down
```

The Computer rolls up the worst across all services.

### Metric

```rust
pub struct Metric {
    pub name:  String,    // "drive.fragments.stored"
    pub value: f64,
    pub kind:  MetricKind,    // Counter | Gauge
    pub unit:  &'static str, // "fragments", "bytes", "ms", "°C"
}
```

---

## 7. The IPC protocol — wire format

A single line of UTF-8 JSON, terminated by `\n`. No length prefix, no
framing, no binary. The socket is `SOCK_STREAM` so the OS handles
framing for us; we just read until `\n`.

### Why JSON and not bincode?

- Trivial to debug with `socat` or `nc`
- Trivial to consume from any language
- The payloads are tiny (status snapshots are < 1 KB)
- Versioning is forward-compatible: extra fields are ignored

### Operations

| `op`      | `service` | Effect                                            |
| --------- | --------- | ------------------------------------------------- |
| `status`  | —         | full snapshot of Computer + all services          |
| `list`    | —         | just the list of services with phase + health     |
| `start`   | required  | start one service (idempotent if already running)  |
| `stop`    | required  | stop one service (idempotent if already stopped)   |

### Response shape

```json
{
  "id": "abc",
  "ok": true,
  "data": { /* any JSON */ }
}
```

or

```json
{
  "id": "abc",
  "ok": false,
  "error": "unknown service: x"
}
```

The `id` is **always** echoed back, even on parse errors. This is what
lets the client multiplex requests over a single connection if it
wants to.

---

## 8. The Server — protocol

Two HTTP endpoints, plain HTTP/1.1, no TLS (the server is zero-knowledge
anyway — TLS would just add CPU).

### `PUT /relay`

Request: opaque bytes in the body.
Response: `{"id": "<16 hex chars>"}`

The bytes are stored in an in-memory `HashMap<Token, Bytes>`. A
background sweep task removes anything older than `ttl_secs` (default
30, max 60).

### `GET /relay/:id`

Response: 200 + body if found, 404 otherwise. **After a successful GET
the fragment is deleted** — the relay is not a CDN.

### Constraints

- max body size: 32 KB (configurable)
- max in-memory: bounded by `ttl * put_rate`
- the server holds **no** state to disk
- restart = full amnesia, by design

---

## 9. The TUI — layout

```
╔════════════════════════════════════════════════════════════╗
║  ⬡ polygone     v0.1.0   Computer: lvs0   ↑0  ↓0  ♥ 4/7   ║
╠════════════════════════════════════════════════════════════╣
║                                                            ║
║   ┌── Dashboard ── Favoris ── Services ── Settings ──┐      ║
║   │                                                  │      ║
║   │  (current tab content)                           │      ║
║   │                                                  │      ║
║   └──────────────────────────────────────────────────┘      ║
║                                                            ║
║   ↑↓ navigate    ⏎ select    q quit    r refresh          ║
╚════════════════════════════════════════════════════════════╝
```

- `Dashboard` — your node, your services, your traffic
- `Favoris` — pinned peers / pinned fragments
- `Services` — start/stop the 8 services, see metrics live
- `Settings` — bind address, log level, identity export

Refresh: 250 ms. Colors: cyber-slate palette, `#22d3ee` (cyan) +
`#0f172a` (slate).

---

## 10. The web UI

Four HTML pages, no framework, no build step. The Rust binary embeds
them at compile time via `include_bytes!`.

| Page        | Path        | Talks to                                |
| ----------- | ----------- | --------------------------------------- |
| Landing     | `/`         | static                                  |
| Node        | `/node.html`| `/api/status` (poll every 3 s)          |
| Drive       | `/drive.html` | `/api/drive/*` (planned)              |
| Mesh        | `/mesh.html`  | `/api/peers` (planned)                |

The dashboard has a **simulation fallback**: if `/api/status` 404s, it
generates plausible local data so the page is never empty. This is
honest — the data is clearly labelled "simulated".

---

## 11. The build matrix

| Target                 | Subcommand              | Output                |
| ---------------------- | ----------------------- | --------------------- |
| `cargo build --lib`    | —                       | `libpolygone.rlib`    |
| `cargo build --bin polygone`          | CLI       | `polygone`            |
| `cargo build --bin polygone-computer` | daemon    | `polygone-computer`   |
| `cargo build --bin polygone-server`   | relay     | `polygone-server`     |
| `cargo build --bin polygone-ctl`      | IPC       | `polygone-ctl`        |
| `cargo build --release`               | all       | `target/release/*`    |
| `cargo test --lib`                    | tests     | 43/43 passing         |

Release profile:
```toml
[profile.release]
opt-level = 3
lto       = "thin"
strip     = true
panic     = "abort"
codegen-units = 1
```

A full release build of all four binaries: **~1.3 MB** stripped.

---

## 12. What is not in this document

- The exact `Cargo.toml` features — read it.
- The wire format of `msh` (mesh gossip) — see `src/network/mesh.rs`.
- The Drive chunking strategy — see `src/network/drive.rs`.
- The ML-KEM-1024 / ML-DSA-87 key schedule — see `src/crypto/`.

If something is missing here, it is because the code is the source of
truth, not the docs. **Always trust the tests.**
