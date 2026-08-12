# ═══════════════════════════════════════════════════════════════
# Polygone Ghost Node — Dockerfile
# ═══════════════════════════════════════════════════════════════
# Basé sur r-labs/polygone/Dockerfile (ZAB QUARTZ pattern)
# Build : docker build -f docs/deployment/Dockerfile -t polygone-ghost .
# ═══════════════════════════════════════════════════════════════

# Étape 1 : Build Rust (statique musl)
FROM rust:1.97-alpine AS build
RUN apk add --no-cache musl-dev

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY daemon ./daemon

# Build release
RUN cargo build --release --workspace --locked

# Étape 2 : Runtime ultra-léger (Alpine)
FROM alpine:3.21
RUN apk add --no-cache bash curl ca-certificates

# Copier binaires
COPY --from=build /src/target/release/polygone /usr/local/bin/
COPY --from=build /src/target/release/polygone-relay /usr/local/bin/ 2>/dev/null || true
COPY --from=build /src/target/release/polygoned /usr/local/bin/ 2>/dev/null || true

# Script ghost node
COPY docs/deployment/ghost-node.sh /usr/local/bin/ghost-node.sh
RUN chmod +x /usr/local/bin/ghost-node.sh

# Variables d'environnement
ENV ZAB_HOME=/data
ENV ZAB_PORT=4242
ENV ZAB_ALIAS=ghost
ENV ZAB_PUB_ADDR=127.0.0.1:4242
ENV BOOTSTRAP=127.0.0.1:4243
ENV HEARTBEAT_SECS=120

WORKDIR /data

# Healthcheck
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:$ZAB_PORT/health || exit 1

# Le heartbeat tourne en boucle (anti-veille réelle + présence réseau)
CMD ["/usr/local/bin/ghost-node.sh"]
