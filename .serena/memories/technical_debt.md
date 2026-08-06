# Polygone — Technical Debt & Architectural Issues

## Critical issues (must fix before v2)

### 1. Duplicate `src/` and `polygone/` crates
The `polygone/` directory is a near-complete mirror of `src/` (the root crate).
This means:
- Two copies of every source file
- Workspace members in `/crates/` import from `polygone::` but the source lives in `src/`
- Build confusion: `cargo build --workspace` vs `cargo build` (root only)
- `polygone/src/lib.rs` re-exports from `polygone::` namespace while `src/lib.rs` re-exports from root

**Action**: Decide which is canonical. Likely should be `src/` (root) = canonical, `polygone/` = the published crate, and the workspace members import from `polygone`. But currently `polygone/` is NOT published — it's just a duplicate. Should delete it or make it properly owned.

### 2. `crates/` workspace members are stubs
The 9 workspace crates (brain, drive, hide, mesh, msg, compute, nodeos, search, common, app) 
are mostly empty or near-empty. The real implementation lives in `src/`. 
The workspace is aspirational, not functional.

**Action**: Either delete the empty crates, or fully migrate the actual code into them.
Don't keep aspirational scaffolding.

### 3. `src/computer/` is orphaned from P2P
`Computer` is a beautiful orchestrator (plan + event + services + SSE). 
It connects to `serve_live()` (web dashboard). But it has NO reference to `P2pNode`.
In the full vision, Computer should orchestrate distributed compute over the P2P network.
Currently Computer is a local-only orchestrator disconnected from the P2P layer.

**Action**: Define the `Computer → P2pNode` interface. Does Computer own a P2pNode?
Or are they separate entities that communicate via IPC/UDP?

### 4. Test directory doesn't exist
`src/computer/tests/` and `src/network/topology.rs` have `#[cfg(test)]` blocks, 
but `cargo test` times out (network tests need bootstrap/mock).

**Action**: Mock network for tests. Use `libp2p::swarm::NetworkBehaviour` with a mock node.

---

## Medium issues

### 5. `.git/` is duplicated: `Polygone-Fresh/` and `polygone/` subdirectory
The `polygone/` subdirectory has its own `.git/`. The root `Polygone-Fresh/.git/` 
covers the whole repo. The `polygone/.git/` is likely an artifact of a copy operation.

**Action**: Remove `polygone/.git/` (keep only the root .git/).

### 6. Two web servers: `serve()` and `serve_live()`
`serve()` uses `NodeState`, `serve_live()` uses `Computer`. They're separate binaries.
User runs one or the other. These should be ONE server with two modes.

**Action**: Unify into a single `serve()` that takes both NodeState and Computer.

### 7. No CI/CD
No GitHub Actions, no badge, no automated test on push.

**Action**: Add `rust.yml` CI with: cargo test, cargo clippy, cargo fmt --check.

### 8. `src/network/node.rs` and `src/network/p2p.rs` overlap
`node.rs` has a `P2pNode` struct. `p2p.rs` has `P2pNode` struct AND `PolygoneBehaviour`.
There are TWO definitions of P2pNode. This is a naming collision.

**Action**: Consolidate. Keep `p2p.rs` as the canonical source, remove from `node.rs` or merge.

### 9. No error taxonomy
`src/crypto/error.rs` defines `PolygoneError`. But many modules use `Box<dyn std::error::Error>`.
No centralized error handling strategy.

**Action**: Implement `thiserror` or `anyhow` consistently. Define error variants per module.

---

## Minor issues

### 10. `NIGHT-REPORT.md` in git
Generated nightly reports shouldn't be committed.

### 11. `polygone-tui-preview.svg` — generated asset, not source
Should be in .gitignore or generated at build time.

### 12. `docs/` directory empty
Documentation exists in `ARCHITECTURE.md`, `POLYGONE-SPEC-*.txt`, `ECOSYSTEM.md`, 
`IMPROVEMENT_PLAN.md` at root level. No `docs/` subdirectory needed, or consolidate to it.

### 13. `polygone-install.rs` binary
Looks like a Cargo install helper. Verify it's still needed or if `cargo install` suffices.

### 14. No `examples/` directory
Standard practice: add `examples/` for demo scripts (e.g., `examples/local_mesh.rs`).

---

## What's genuinely excellent (don't touch)

- ML-KEM-1024 + ML-DSA-87 + AES-256-GCM + Shamir 4/7 + BLAKE3 — correct, clean, real
- libp2p stacked behaviour (9 protocols) — textbook quality
- `TransitState` FSM in `protocol/session.rs` — elegant, correct
- `serve_live()` + SSE event stream — production-ready design
- `Service` trait + Phase/Health system — extensible, clean
- TUI dashboard (5 tabs, sparklines, gauges) — visually impressive
- `PolygoneRequest/Response` enum with discriminant types — well-designed
- `Computer` orchestrator (plan + event + services) — powerful concept
- `IdleDetector` for compute lending — smart
- Property-based testing in `crypto/kem.rs` — rigorous

---

## Roadmap priorities (my suggested order)

1. **Fix Git** — push, tag v1.0.0, add CI
2. **Unify web server** — serve() + serve_live() = one binary
3. **Connect Computer ↔ P2pNode** — distributed orchestrator
4. **Clean duplicate `polygone/` crate** — decide canonical
5. **Add examples/** — local loop demo, compute lending mock
6. **Property-based fuzzing** — proptest for crypto + protocol
7. **v2 brand** — README rewrite, landing page, manifest