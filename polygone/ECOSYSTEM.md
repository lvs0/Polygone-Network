# ECOSYSTEM.md — the mother file

> *Si tu ne sais pas où regarder, c'est ici.*

This document is the single source of truth for **what Polygone is, what
services it ships with, and what each one does**. Every other document
references back to this one.

---

## 1. The three planes

Polygone is a **3-plane system**. Anything you do with Polygone lives in
exactly one of these:

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│   PLANE 1            PLANE 2             PLANE 3                │
│   ────────           ────────            ────────               │
│                                                                 │
│   YOUR               THE                 THE                    │
│   COMPUTER           MESH                RELAY                  │
│                                                                 │
│   polygone-          polygone-           polygone-              │
│   computer           computer            server                 │
│   (daemon)           (peers)             (stateless)            │
│                                                                 │
│   • owns your        • discovers         • bridges NATs         │
│     services           peers             • zero-knowledge        │
│   • IPC socket       • encrypted         • TTL 30s, 32KB        │
│   • restart loop       gossip             fragments             │
│   • status file      • mDNS/BLE          • no plaintext ever    │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

- **Plane 1 — Your Computer.** The local daemon. Owns every service on
  your machine. Exposes IPC (Unix socket) and HTTP (web UI). State is
  local: keys, fragments, config, logs.
- **Plane 2 — The Mesh.** Peers you can reach directly. They run
  `polygone-computer` too. Discovery is mDNS / BLE. Communication is
  end-to-end encrypted. The mesh is symmetric — there is no master.
- **Plane 3 — The Relay.** A `polygone-server` you can fall back to
  when you cannot reach a peer directly. The relay **never** sees
  plaintext, **never** sees keys, **never** persists anything beyond a
  30-second in-memory cache.

---

## 2. The service registry

Every service implements the `Service` trait (see `src/services/mod.rs`).
The Computer daemon is the only thing allowed to start or stop a
service. Clients (CLI, TUI, web, IPC) only ask.

| ID         | Name        | Role                                                    | Default |
| ---------- | ----------- | ------------------------------------------------------- | ------- |
| `compute`  | Polygone Compute | Lend / borrow compute for distributed work         | off     |
| `drive`    | Polygone Drive   | Encrypted sharded file storage across 7 peers      | on      |
| `hide`     | Polygone Hide    | SOCKS5 + HTTPS proxy through the mesh              | on      |
| `mesh`     | Polygone Mesh    | mDNS + BLE discovery, peer orchestration          | on      |
| `brain`    | Polygone Brain   | Local LLM (Notch SLM / Ollama / llama.cpp)         | on      |
| `msg`      | Polygone Msg     | Ephemeral E2E messaging — no server, no logs       | on      |
| `petals`   | Polygone Petals  | Distributed LLM — peers hold shards, run in parallel | off  |
| `shell`    | Polygone Shell   | Secure shell over the mesh, peer-to-peer           | off     |

`on` means the service is started by `Computer::boot()`. `off` means
it is registered but must be started explicitly by the user.

---

## 3. The contract — what every service must do

```rust
#[async_trait]
pub trait Service: Send + Sync {
    fn info(&self) -> ServiceInfo;     // static metadata
    async fn start(&self) -> PolyResult<()>;
    async fn stop(&self)  -> PolyResult<()>;
    async fn phase(&self) -> Phase;     // Stopped | Starting | Running | Stopping | Crashed
    async fn health(&self) -> Health;   // Ok | Degraded | Failing | Down
    async fn metrics(&self) -> Vec<Metric>;
}
```

Default implementations are provided for `is_active()`, `is_healthy()`,
`uptime()`. A service is expected to:

- be **idempotent** on `start()` and `stop()` (calling twice is a no-op)
- never panic
- expose at least one `Metric` once it is `Running`
- never block longer than 100 ms in a `start()` / `stop()` (spawn
  long-lived work as a background task instead)
- be observable through `metrics()` (counters and gauges only, no
  labels, no histograms yet)

---

## 4. The Computer daemon — lifecycle of a tick

```
boot()
  │
  ├── register_default_services()       # on: drive, hide, mesh, brain, msg
  │                                       off: compute, petals, shell
  │
  ├── start_all()
  │     for each service:
  │       if phase in {Stopped, Crashed}: spawn start task
  │
  └── run()  ← 1 Hz loop
        │
        ├── for each service:
        │     if phase == Crashed: eprintln!(); svc.start()
        │
        ├── write /tmp/polygone-status.json  (atomic rename)
        │
        └── accept IPC client on $POLYGONE_SOCKET
              dispatch one of: status | list | start | stop
```

The Computer **never** holds a service's data. It is a lifecycle
manager. State lives in the service itself.

---

## 5. The IPC — line-delimited JSON over a Unix socket

Path: `$POLYGONE_SOCKET` (default `/tmp/polygone-computer.sock`)

### Request

```json
{"id": "abc", "op": "status"}
{"id": "abc", "op": "list"}
{"id": "abc", "op": "start", "service": "drive"}
{"id": "abc", "op": "stop",  "service": "drive"}
```

### Response

```json
{"id": "abc", "ok": true,  "data": {...}}
{"id": "abc", "ok": false, "error": "unknown service: x"}
```

One line in, one line out. No streaming, no binary frames, no
subscriptions. If you need pub/sub, hit the web UI on `/api/status`
instead — it polls.

### Clients

- `polygone` TUI — connects on launch, redraws every 250 ms
- `polygone-ctl` — one-shot scriptable client
- any shell — `socat - UNIX-CONNECT:/tmp/polygone-computer.sock`

---

## 6. The Server — what it is, what it is not

`polygone-server` is **a dumb pipe**. It does not know:

- your identity
- your public keys
- your message contents
- who you are talking to

It knows only:

- a 16-hex-char token (opaque, random)
- a TTL (default 30 s, max 60 s)
- a size cap (32 KB per fragment)

If a fragment is not consumed within its TTL, it is purged by
`RelayStore::sweep()`. The server's RAM is bounded by `ttl * put_rate`.

The server is **not** required for Polygone to work. Two nodes on the
same LAN can communicate without it. The relay exists to bridge NATs.

---

## 7. The use cases

### 7.1 — Encrypted file share

```
alice@macbook>  polygone drive put report.pdf
                → /drive/abc123.../report.pdf
                → 4-of-7 Shamir split across 7 peers
                → each fragment AES-256-GCM encrypted with a per-fragment key

bob@thinkpad>   polygone drive get /drive/abc123.../report.pdf
                → 4 of 7 peers reachable: rebuild locally
                → 4 of 7 peers unreachable: error "need 4, have 3"
```

### 7.2 — Anonymous web

```
firefox → 127.0.0.1:9050 (SOCKS5) → polygone hide → mesh → peer → internet
                                                                ↑
                                              3 hops, each encrypted
                                              independently, no peer
                                              knows the full path
```

### 7.3 — Local LLM

```
polygone> brain ask "summarize /drive/abc/report.pdf"
        → if local model loaded: answer in 800 ms
        → if not: fallback to petals
            → split prompt into 8 shards
            → 8 peers each run 1/8 of the inference
            → assemble at home
        → answer in 4.2 s, 0 plaintext leaves the mesh
```

### 7.4 — One-shot message

```
alice → bob: encrypted, signed, ephemeral
        → bob's key signed by alice's ML-DSA-87 key
        → ciphertext is AES-256-GCM
        → TTL: 24h
        → stored nowhere
        → server sees only opaque bytes
```

### 7.5 — Mesh node dashboard

```
http://127.0.0.1:8080/node.html
        → live CPU, RAM, traffic in/out
        → 5 module cards (Drive / Hide / Mesh / Brain / Msg)
        → live log panel
        → mesh force-directed graph at /mesh.html
        → file drop zone at /drive.html
```

---

## 8. The naming

| Symbol       | What it is                                          |
| ------------ | --------------------------------------------------- |
| `Polygone`   | The ecosystem. The name on the box.                 |
| `Computer`   | The local daemon (`polygone-computer`).              |
| `Server`     | The relay (`polygone-server`). Stateless.            |
| `polygone`   | The binary you run. The dashboard.                   |
| `msh`        | Mesh gossip protocol. 3 letters, on purpose.         |
| `poly`       | The unit of account inside the mesh. 1 poly = 1 GB-h. |
| `lvs0`       | Example node ID (Lévy, single node).                 |

---

## 9. The non-goals

Polygone will **not**:

- replace your email
- store your photos in the cloud
- integrate with Slack / Discord / Twitter
- ask you for an account
- phone home
- be a token
- be a DAO
- be a "Web3" thing

Polygone **will**:

- run on your machine
- encrypt by default
- crash loudly
- refuse to start if integrity is broken
- be readable in one sitting
- have a TUI you can use over SSH on a 80x24 terminal
- **show you the plan before executing it** (Gemini/Perplexity pattern)
- **stream every system event live** (Operator/Devin pattern)

## 10. The thought stream and the plan gate

Two cross-cutting patterns borrowed from 2026 agentic products, adapted
to Polygone's local-first philosophy. See [`POLYGONE-PATTERNS.md`](./POLYGONE-PATTERNS.md)
for the full design rationale.

### 10.1 The plan gate

Any multi-step side effect (start service, stop service, restart
cluster) goes through a `Plan` that the user must approve.

```
    proposed                approved               done
   ─────────►  user ok   ──────────►  execute  ─────────►
       │                                                ▲
       └────── user reject ────────►  rejected          │
                                                        │
                                          failed ◄──────┘
```

A `Plan` is a list of `PlanStep { service, action, rationale }`. The
Computer refuses to execute a `Plan` in any state other than
`Approved`. The TUI shows the plan with `y/n`. The web page at
`/plan.html` shows the same plan with three buttons. The IPC accepts
`Request::ApprovePlan` and `Request::RejectPlan`.

### 10.2 The thought stream

Every service can emit `ServiceEvent` records. The Computer collects
them into an `mpsc` channel of capacity 1024 and exposes the stream
to the TUI and the web. The web dashboard's `/plan.html` page opens
an `EventSource` connection on `GET /api/events` and renders the last
100 events with timestamps and colour-coded severity.

```
Service ──emit──▶ Computer.event_sender ──broadcast──▶ TUI live tab
                                                    └▶ Web live feed
                                                    └▶ IPC subscriber
```

An event is `{ source, kind, severity, message, epoch_ms }`. Kinds
are `info` / `warn` / `error` / `done` / `metric`. Severities are
`info` / `success` / `warning` / `error`. The Computer itself emits
events for `boot`, `service_started`, `service_crashed`, `plan_proposed`,
`plan_approved`, `plan_rejected`, `plan_done`, `shutdown`.

**This is not** the LLM's inner monologue. It is *system state* —
what is happening on the wire, on the disk, on the relay. The user
sees the machine working, not a chatbot thinking.