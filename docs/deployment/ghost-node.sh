#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════
# ghost-node.sh — Nœud Ghost Polygone : présence réseau + anti-veille
# ═══════════════════════════════════════════════════════════════
# Ce n'est PAS du faux trafic : chaque tick est une véritable annonce
# (heartbeat) au bootstrap, qui maintient le nœud dans le registre des
# nœuds vivants. L'effet anti-veille (les plateformes gratuites coupent
# les instances inactives) découle naturellement de cette activité réelle.
# ═══════════════════════════════════════════════════════════════
set -euo pipefail

: "${ZAB_HOME:=/data}"
: "${ZAB_PORT:=4242}"
: "${ZAB_ALIAS:=ghost}"
: "${ZAB_PUB_ADDR:=127.0.0.1:4242}"
: "${BOOTSTRAP:=127.0.0.1:4243}"
: "${HEARTBEAT_SECS:=120}"

export ZAB_HOME
mkdir -p "$ZAB_HOME/inbox" "$ZAB_HOME/keys"

# Identité persistante (la clé ne change jamais entre redémarrages)
if [ ! -f "$ZAB_HOME/node.key" ]; then
  echo "› Génération de l'identité du nœud..."
  polygone keygen --out "$ZAB_HOME/node.key" --comment "ghost:$ZAB_ALIAS"
fi

# Écoute des messages entrants en arrière-plan
echo "› Démarrage de l'écoute sur le port $ZAB_PORT"
polygone ecouter --port "$ZAB_PORT" --inbox "$ZAB_HOME/inbox" &
LISTEN_PID=$!
trap 'kill $LISTEN_PID 2>/dev/null || true' EXIT

# Boucle de heartbeat
echo "› Annonce au bootstrap $BOOTSTRAP (alias=$ZAB_ALIAS, addr=$ZAB_PUB_ADDR)"
while true; do
  polygone annoncer --bootstrap "$BOOTSTRAP" \
       --alias "$ZAB_ALIAS" \
       --key "$ZAB_HOME/node.key" \
       --addr "$ZAB_PUB_ADDR" \
       || echo "✘ heartbeat échoué"
  sleep "$HEARTBEAT_SECS"
done
