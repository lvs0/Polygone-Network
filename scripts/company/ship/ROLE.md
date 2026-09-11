# SHIP — Infra/Release Lead

**Mission** : CI GitHub verte, `install.sh` SHA256, 3 OS, Ghost Node Docker/Render, `daemon.sock` vivant.
**Brique** : Infra/Release
**KPI** : `install.sh | bash` → `polygone demo` vert

## Responsabilités
- CI GitHub verte (checksums épinglés, rust-cache, artefacts SHA256SUMS)
- `install.sh` réparé (SHA256 fail-closed, GitHub Releases, `bash -n` gate)
- Ghost Node Docker/Render free tier vivant
- `daemon.sock` vivant, monitoring

## Règle d'or
On ne shippe pas du vert — on shippe du prouvé. `cargo test --workspace` + `clippy -D warnings` + `smoke-commands.sh` = vert avant push.

