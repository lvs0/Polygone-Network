# Polygone-Network — Resource Allocation Daemon

## Purpose

Polygone needs nodes to run. Most people's machines have idle resources 80% of the time. `polygoned` runs in the background, watches what's free, and allocates it to the Polygone P2P network — with a hard safety margin so the machine never feels it.

## Philosophy

- **Wozniak**: one engineer, one job, no bloat
- **Jobs**: if the machine is using resources, the daemon shrinks. If resources are free, it grows. It never touches what isn't free.
- **Palantir**: infrastructure that disappears into the background, invisible to the operator

## Design Principles

1. **Never starve the host** — daemon never allocates more than N% of free RAM/CPU, where N is configurable (default 70%)
2. **Graceful shrinking** — if a spike happens, daemon shrinks allocation over 5s, not instantly
3. **Zero config by default** — works out of the box, config file only to override
4. **Single binary** — no daemon of daemons, no systemd dependency, runs standalone
5. **Hard telemetry** — the daemon shows exactly what it's doing, no hidden state

## Architecture

```
polygoned (this crate)
├── system/     — reads /proc, sysinfo, CPU stats
├── allocator/  — decides how much to give/take
├── network/    — tells Polygone node via socket/HTTP
└── main.rs     — ties it together, runs the loop
```

## Resource Model

```
AVAILABLE = total_free - SAFETY_MARGIN
ALLOWED   = AVAILABLE * config.max_alloc_ratio (default 0.7)
```

Example on 16GB machine with 12GB free:
```
AVAILABLE = 12GB - 1GB = 11GB
ALLOWED   = 11GB * 0.7 = 7.7GB → round down to 7GB
```

## Interface to Polygone Node

The daemon writes to a Unix socket at `~/.polygone/daemon.sock`:

```json
{"cmd":"set_alloc","ram_mb":7168,"bandwidth_mbps":50,"timestamp":1752700000}
{"cmd":"shrink","reason":"user_activity","timestamp":1752700010}
{"cmd":"grow","headroom_mb":2048,"timestamp":1752700050}
```

The Polygone node reads this socket and adjusts its libp2p connection pool, DHT bucket size, and bandwidth allocation accordingly.

## Polygone Network Integration

`polygoned` is the **resource broker**. Polygone-Network (the P2P node) is the **client**. The daemon does not run the P2P stack itself — it tells an *existing* Polygone node how much it can consume.

## CLI Commands

```
polygoned              — start daemon (background)
polygoned status       — show current allocation + system stats
polygoned stop         — shrink to zero and exit cleanly
polygoned --dry-run    — print what would happen without acting
polygoned --config     — generate default config
```

## Dependencies

Only `sysinfo` (system info) + `tokio` (async runtime) + `serde` (JSON). No framework, no bloat.

## Output Format

```
[polygoned] 14:23:01 | RAM: 7.2GB/16GB | CPU: 12% | Alloc: 7GB | +2GB headroom | P2P: active
```

Clean, scannable, grep-able.