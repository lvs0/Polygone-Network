# archive/2026-07-src — the v1-era monolith (archived 2026-08-07)

**Do not build, do not import, do not trust.** This tree is the *previous*
architecture: a single-crate monolith with libp2p, an HTTP relay, a 4-tab
ratatui dashboard and a POLY economy ledger. It was replaced by the v2
workspace (4 crates: `core` / `client` / `relay` / `daemon`) and archived
**without being deleted** — the git history and this directory preserve the
ideas, so nothing is lost.

## What is in here (and why it is not in the product)

| Directory | Content | Status |
| --------- | ------- | ------ |
| `network/` | libp2p behaviour, drive storage, mesh, mDNS/BLE, topology | Replaced by TCP NDJSON + UDP broadcast in `crates/client` |
| `server/` | in-memory HTTP relay (PUT/GET /relay, 32 KB, TTL 30 s) | Replaced by `crates/relay` (TCP, stateless) |
| `tui/` | ratatui 4-tab dashboard | Replaced by the vim-style TUI in `crates/client` |
| `economy/` | POLY token ledger (0.1 POLY/min, 10 POLY/core-h, …) | **Never decided.** Parked here pending an explicit product decision |
| `ipc/` | line-JSON over Unix socket | Superseded by the daemon command socket |
| `web/` assets | 4 HTML pages | Superseded by `site/` + `index.html` landing |
| `crypto/`, `compute/`, `computer/`, `protocol/`, `services/` | early service architecture | Ideas only — see `ECOSYSTEM.md` / `STAGING.md` |

## Why it is kept

- The **economy ledger** (`economy/`) is the only copy of the POLY revenue
  model — a product decision that was written, hidden, then denied. It stays
  available so the decision can finally be made consciously.
- The libp2p research (`network/`) may inform a future mesh topology.

## Rules

- If a file is genuinely wanted back in the product, move it out of this
  directory into a live crate and make it compile — do not reference it
  from the workspace.
- `cargo build` / `cargo test` at the repo root must never see this tree
  (it is outside the workspace members).
