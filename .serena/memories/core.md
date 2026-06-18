# Polygone — Core Map

## What it is
Post-quantum ephemeral transit network. No server sees the message. No observer can prove a message existed.

## Stack
- Rust, 2021 edition, workspace (10 crates + 1 root)
- Tokio async runtime, clap for CLI, ratatui for TUI
- libp2p 0.55 for P2P (Kad, GossipSub, Req-Resp, mDNS, Relay, AutoNAT, DCUtR, Ping)
- pqc_kyber 0.7, aes-gcm 0.10, sharks (Shamir SSS), blake3

## Architecture (two layers)

### Root `src/` — the main binary
```
main.rs            — CLI: Menu(TUI), Status, Test, Serve
lib.rs             — re-exports: compute, computer, crypto, ipc, network, 
                     protocol, server, services, tui, web, error
crypto/            — ML-KEM-1024 (kem.rs), Shamir 4/7 (shamir.rs), AES-256-GCM
                     BLAKE3 KDF, sign.rs, karma.rs, error.rs
network/           — p2p.rs (PolygoneBehaviour = libp2p stacked),
                     node.rs (P2pNode), discovery, topology
protocol/          — session.rs (TransitState machine, SessionId)
services/          — Service trait (interface), Phase, Health, Metric,
                     ServiceEvent, ServicePolicy
tui/               — ratatui v2: app, views (5 tabs), favorites, widgets
web/               — web/mod.rs: serve() + serve_live(), NodeState/WebConfig,
                     handle_conn/route/sse, assets from embedded bytes
server/            — RelayStore, RelayStats, HTTP relay server (port 8080 fallback)
ipc/               — Unix socket client/daemon, Op enum, Request/Response
computer/          — Computer (plan orchestrator), Plan/PlanStep/PlanAction,
                     SSE event bus, snapshot API
compute/           — IdleDetector, idle daemon, SystemMetrics
bin/               — polygone-ctl, polygone-server, polygone-serve-live,
                     polygone-install, polygone-computer
```

### `/polygone/` crate — the internal library (duplicated from `src/`)
Same module structure, serves as a proper Rust crate for workspace members.

### `/crates/` workspace — future refactors (mostly empty stubs)
common/, app/, polygone-msg/, polygone-drive/, polygone-hide/, polygone-mesh/,
polygone-brain/, polygone-search/, polyygone-compute/, polygone-nodeos/

## Entry points

| Command | Description |
|---|---|
| `polygone` (default) | TUI dashboard, 5 tabs (Dashboard/Favorites/Services/Composer/Settings) |
| `polygone status` | Non-interactive system status print |
| `polygone test` | Self-test suite (crypto primitives) |
| `polygone serve --bind` | Web dashboard on :8080 |
| `polygone-ctl` | Scriptable IPC client over Unix socket |
| `polygone-server` | Relay HTTP server |
| `polygone-serve-live` | Live web dashboard with Computer/Plan SSE |

## Web API surfaces

**serve()** (NodeState-based):
- `GET /` → index.html
- `GET /health` → plain text OK
- `GET /api/status` → NodeState JSON
- `GET /api/peers` → peer list
- `POST /api/share` → file share

**serve_live()** (Computer-based):
- `GET /` → index.html
- `GET /health` → "polygone-live"
- `GET /api/status` → Computer snapshot
- `GET /api/plan` → current plan
- `POST /api/plan/propose` → propose boot plan
- `POST /api/plan/approve` → approve plan
- `POST /api/plan/reject` → reject plan
- `GET /api/events` → SSE event stream

## Critical design insight
`Computer` (src/computer/) is the orchestrator that binds services, plans, and events.
It connects to the web layer via `serve_live()`. It is the **intended successor** of the
NodeState/web approach. The two web servers coexist.

## Key enums / types

`TransitState` (protocol/): `Created → FragmentsDistributed → PartialReception → Reassembling → Delivered → Expired`

`PolygoneRequest` (network/): `DriveChunk | DriveStore | PetalsInfer | HideTunnel | HideData`

`PolygoneResponse` (network/): mirrors request types with success + data

`GossipMessage` (network/): `TopologyAnnounce | CapabilitiesAnnounce | Heartbeat`

`Capability` (network/): `DriveStorage | PetalsCompute | HideExit | Relay`

`Service` trait (services/): `phase(), health(), metrics(), on_event()`

`Phase` (services/): `Pending | Starting | Running | Stopping | Stopped | Failed`

## Files that matter for navigation
- `src/network/p2p.rs:346-710` — P2pNode event loop
- `src/computer/mod.rs` — Computer orchestrator (SSE + Plan + EventBus)
- `src/web/mod.rs:83-302` — serve + serve_live + routing
- `src/protocol/session.rs` — TransitState FSM
- `src/services/mod.rs` — Service trait + Phase + Health
- `crates/polygone-brain/src/lib.rs` — Brain (inference) module

## References
`mem:tech_stack` — Rust edition, tokio, libp2p, pqc_kyber versions
`mem:suggested_commands` — cargo build, test, clippy, fmt
`mem:conventions` — Rust conventions + Polygone patterns
`mem:task_completion` — what "done" means for this project
`mem:crypto` — ML-KEM-1024, Shamir, AES-GCM details
`mem:network` — libp2p stack, P2pNode lifecycle
`mem:web_api` — serve() vs serve_live() distinction
`mem:services` — Service trait, Phase/Health lifecycle
`mem:computer` — Computer orchestrator, Plan, SSE
`mem:protocol` — TransitState FSM, SessionId